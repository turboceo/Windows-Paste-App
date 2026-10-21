//! Shared application state and DTO conversion for the Tauri command layer.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::db::{AppPaths, Database};
use crate::model::{ClipboardItem, ClipboardItemType, MAX_CONTENT_CHARS};

/// Maximum number of unpinned entries kept in history.
pub const HISTORY_CAPACITY: usize = 500;

pub struct AppState {
    pub db: Database,
    pub paths: AppPaths,
    pub items: Vec<ClipboardItem>,
    pub total_count: usize,
    pub selected_id: Option<i64>,
    pub search_text: String,
    pub active_tag: Option<String>,
    pub all_tags: Vec<String>,
    pub columns: usize,
    /// Foreground window captured right before the panel was summoned, so the
    /// simulated Ctrl+V lands in the app the user was actually using.
    pub last_target_hwnd: isize,
}

impl AppState {
    pub fn new(paths: AppPaths) -> Self {
        let db = Database::open_with_blobs(&paths.db, HISTORY_CAPACITY, paths.blobs.clone())
            .expect("open sqlite database");
        let total_count = db.get_total_count().unwrap_or(0);
        let all_tags = db.get_all_tags().unwrap_or_default();
        let items = db.get_all().unwrap_or_default();
        let selected_id = items.first().map(|i| i.id);

        Self {
            db,
            paths,
            items,
            total_count,
            selected_id,
            search_text: String::new(),
            active_tag: None,
            all_tags,
            columns: 4,
            last_target_hwnd: 0,
        }
    }

    pub fn reload(&mut self) {
        self.total_count = self.db.get_total_count().unwrap_or(0);
        self.all_tags = self.db.get_all_tags().unwrap_or_default();
        self.items = self
            .db
            .get_filtered(&self.search_text, self.active_tag.as_deref())
            .unwrap_or_default();

        let keep = self
            .selected_id
            .filter(|id| self.items.iter().any(|i| i.id == *id));
        self.selected_id = keep.or_else(|| self.items.first().map(|i| i.id));
    }

    pub fn selected(&self) -> Option<ClipboardItem> {
        let id = self.selected_id?;
        self.items.iter().find(|i| i.id == id).cloned()
    }
}

pub type SharedState = Mutex<AppState>;

/// 卡片缩略图的目标边长（物理像素）。
///
/// 卡片本身约 236px 宽，按 2x DPI 留到 480，再大就是浪费 IPC。
const THUMBNAIL_EDGE: u32 = 480;

/// 解码 + 缩放 + base64 的结果缓存，key 是 blob 文件名。
///
/// 没有这个缓存的话，每一次推送（搜索、置顶、新增）都会把列表里所有图片
/// 重新解码一遍——这正是之前 base64 方案卡顿的根源。缓存之后同一张图只算一次，
/// 从「每次推送 N 张」降到「整个进程生命周期每张一次」。
fn thumbnail_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, Option<String>>> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Option<String>>>,
    > = std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// 把 blob 里的 PNG 缩放成卡片缩略图，返回 data URL。
///
/// 失败（文件缺失 / 解码失败）返回 `None`，卡片会退化成文字占位——
/// 但不会让整次推送失败。
fn thumbnail_data_url(blobs_dir: &std::path::Path, file_name: &str) -> Option<String> {
    // 先查缓存
    if let Ok(cache) = thumbnail_cache().lock() {
        if let Some(hit) = cache.get(file_name) {
            return hit.clone();
        }
    }

    let computed = build_thumbnail(blobs_dir, file_name);

    if let Ok(mut cache) = thumbnail_cache().lock() {
        cache.insert(file_name.to_string(), computed.clone());
    }
    computed
}

/// 删除记录后清掉对应的缩略图缓存，避免缓存无上限增长。
pub fn forget_thumbnails(file_names: &[String]) {
    if file_names.is_empty() {
        return;
    }
    if let Ok(mut cache) = thumbnail_cache().lock() {
        for name in file_names {
            cache.remove(name);
        }
    }
}

fn build_thumbnail(blobs_dir: &std::path::Path, file_name: &str) -> Option<String> {
    use base64::Engine;

    let bytes = std::fs::read(blobs_dir.join(file_name)).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;

    // 只缩不放：小图保持原尺寸，避免放大后糊成一片
    let thumb = if img.width() > THUMBNAIL_EDGE || img.height() > THUMBNAIL_EDGE {
        img.resize(
            THUMBNAIL_EDGE,
            THUMBNAIL_EDGE,
            image::imageops::FilterType::Triangle,
        )
    } else {
        img
    };

    let mut png = Vec::new();
    thumb
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;

    let encoded = base64::engine::general_purpose::STANDARD.encode(&png);
    Some(format!("data:image/png;base64,{}", encoded))
}

/// One history entry as delivered to the React UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipItemDto {
    pub id: i64,
    /// "Text" | "Image" | "Files"
    pub r#type: String,
    pub content: String,
    pub preview: String,
    pub created_at_label: String,
    pub pinned: bool,
    pub tag: Option<String>,
    /// 原文长度超过上限、落库时被截断过。
    pub truncated: bool,
    /// 这是一个图片条目，可以让前端按需来取缩略图。
    pub has_image: bool,
}

impl ClipItemDto {
    /// 构造推给前端的 DTO。
    ///
    /// 列表里**不带图片数据**，只给一个 `has_image` 标记；缩略图由前端对
    /// 可见的卡片单独调用 `LoadThumbnail` 拉取。
    ///
    /// 为什么这么做：列表推送每次都是全表。如果每条图片都内嵌一张几十 KB 的
    /// data URL，500 条里几十张图就是几 MB 的 JSON，每次搜索、每次新增、每次
    /// 唤出面板都要重新序列化并跨 IPC 传一遍。表越大越慢——这正是"用久了
    /// 就唤不出来"的放大器。现在推送负载与图片数量无关。
    pub fn from_item(item: &ClipboardItem, _blobs_dir: &std::path::Path) -> Self {
        let has_image = matches!(item.item_type, ClipboardItemType::Image)
            && item.blob_path.is_some();

        Self {
            id: item.id,
            r#type: item.item_type.as_str().to_string(),
            content: item.content.clone(),
            preview: item.preview(),
            created_at_label: item.created_at_label(),
            pinned: item.pinned,
            tag: item.tag.clone(),
            // 截断阈值是入库时的字符上限，到顶即认为被裁剪过
            truncated: item.content.chars().count() >= MAX_CONTENT_CHARS,
            has_image,
        }
    }

}

/// 按 blob 文件名取缩略图 data URL，供 `HostRequest::LoadThumbnail` 使用。
pub fn thumbnail_for_blob(blobs_dir: &std::path::Path, file_name: &str) -> Option<String> {
    thumbnail_data_url(blobs_dir, file_name)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionDto {
    pub item_id: Option<i64>,
    pub can_prev: bool,
    pub can_next: bool,
}

impl SelectionDto {
    pub fn from_state(state: &AppState) -> Self {
        let index = state
            .selected_id
            .and_then(|id| state.items.iter().position(|i| i.id == id));

        match index {
            Some(i) => Self {
                item_id: Some(state.items[i].id),
                can_prev: i > 0,
                can_next: i + 1 < state.items.len(),
            },
            None => Self { item_id: None, can_prev: false, can_next: false },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    /// 1x1 的合法 PNG。
    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    fn image_item(blob: Option<&str>) -> ClipboardItem {
        ClipboardItem {
            id: 1,
            item_type: ClipboardItemType::Image,
            content: "1 x 1 image".to_string(),
            blob_path: blob.map(|s| s.to_string()),
            hash: "img-hash".to_string(),
            created_at: Local::now(),
            pinned: false,
            tag: None,
        }
    }

    #[test]
    fn test_list_push_does_not_embed_thumbnails() {
        let dir = std::env::temp_dir().join(format!("test_wp_thumb_a_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("pic.png"), TINY_PNG).unwrap();

        let item = image_item(Some("pic.png"));
        let dto = ClipItemDto::from_item(&item, &dir);

        // 列表 DTO 只有标记，不含图片数据：
        // 这是"用久了推送越来越大"的根源，也是唤出变慢的放大器。
        assert!(dto.has_image, "图片条目必须带 has_image 标记");
        // 序列化后的 JSON 里不应出现 base64 大字段
        let json = serde_json::to_string(&dto).expect("serialize dto");
        assert!(
            !json.contains("base64"),
            "列表推送里不应内嵌缩略图数据，实际 payload: {json}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_thumbnail_is_a_data_url() {
        let dir = std::env::temp_dir().join(format!("test_wp_thumb_b_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("pic.png"), TINY_PNG).unwrap();

        let url = thumbnail_for_blob(&dir, "pic.png").expect("应当能生成缩略图");
        assert!(
            url.starts_with("data:image/png;base64,"),
            "缩略图应是可直接用于 <img src> 的 data URL，实际是 {}",
            &url[..url.len().min(40)]
        );

        use base64::Engine;
        let raw = base64::engine::general_purpose::STANDARD
            .decode(url.trim_start_matches("data:image/png;base64,"))
            .expect("base64 必须合法");
        assert!(raw.starts_with(&[0x89, 0x50, 0x4E, 0x47]), "应是 PNG 字节");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_missing_blob_yields_no_thumbnail() {
        let dir = std::env::temp_dir().join(format!("test_wp_thumb_c_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);

        // 文件不存在时不能 panic，只是取不到缩略图，界面退回文字占位
        assert!(thumbnail_for_blob(&dir, "nope.png").is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_text_item_has_no_image() {
        let dir = std::env::temp_dir();
        let item = ClipboardItem {
            id: 2,
            item_type: ClipboardItemType::Text,
            content: "hello".to_string(),
            blob_path: None,
            hash: "t".to_string(),
            created_at: Local::now(),
            pinned: false,
            tag: None,
        };
        let dto = ClipItemDto::from_item(&item, &dir);
        assert!(!dto.has_image);
    }
}
