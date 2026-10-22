//! 应用状态：**由单一线程独占**，不再放在 Mutex 里。
//!
//! 为什么去掉锁：
//! 原来的设计是 `Mutex<AppState>`，剪贴板线程持锁做"写库 → 全表重查 → 序列化
//! → emit"，而 emit 需要主线程参与；主线程此时正等同一把锁去唤出面板。
//! 两边互等，就是永久卡死。
//!
//! 现在 `AppState` 只属于 `state_worker` 线程（见 `worker.rs`）。任何想操作
//! 它的地方都往 channel 里投一条消息，由那个线程串行处理。没有锁，
//! 因此没有死锁、没有锁中毒、也没有锁竞争。

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
    /// 每个分类下的条目数，供删除确认框展示影响范围。
    pub tag_counts: std::collections::BTreeMap<String, usize>,
    pub columns: usize,
    /// Foreground window captured right before the panel was summoned, so the
    /// simulated Ctrl+V lands in the app the user was actually using.
    pub last_target_hwnd: isize,
}

impl AppState {
    pub fn new(paths: AppPaths) -> Self {
        // 打不开数据库时把原因写进日志再退出。
        // 直接 expect 的话，release 构建里进程会静默消失，
        // 日志里只有"启动"一行，看不出发生了什么。
        let db = match Database::open_with_blobs(&paths.db, HISTORY_CAPACITY, paths.blobs.clone()) {
            Ok(db) => db,
            Err(err) => {
                crate::logging::log(&format!(
                    "[致命] 无法打开数据库 {}：{}",
                    paths.db.display(),
                    err
                ));
                panic!("open sqlite database: {err}");
            }
        };
        let total_count = db.get_total_count().unwrap_or(0);
        let all_tags = db.get_all_tags().unwrap_or_default();
        let tag_counts = db.get_tag_counts().unwrap_or_default();
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
            tag_counts,
            columns: 4,
            last_target_hwnd: 0,
        }
    }

    pub fn reload(&mut self) {
        self.total_count = self.db.get_total_count().unwrap_or(0);
        self.all_tags = self.db.get_all_tags().unwrap_or_default();
        // 每个分类下有多少条：删除确认框要用它告诉用户影响范围
        self.tag_counts = self.db.get_tag_counts().unwrap_or_default();
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

    /// 按 id 取一条记录的副本。
    ///
    /// 需要把数据带到 owner 线程之外时用它——比如粘贴要先把条目读出来，
    /// 之后的写剪贴板、注入按键都不该占着 owner 线程。
    pub fn item_by_id(&self, id: i64) -> Option<ClipboardItem> {
        self.items.iter().find(|i| i.id == id).cloned()
    }

    /// 选中项被删除后，把选择停在原先的槽位。
    ///
    /// `reload()` 的兜底是"选中第一项"：这对搜索过滤是对的（旧下标在新结果里
    /// 没有意义），但对删除不是。用户刚删掉第 8 张，视线还在那里，选中环却
    /// 跳回第 1 张，列表也随之滚回顶部。删除路径在 `reload()` 之后调用这个
    /// 函数覆盖兜底结果；槽位越界（删的是最后一项）时落到列表末尾。
    pub fn select_at_index_or_last(&mut self, index: usize) {
        self.selected_id = self
            .items
            .get(index)
            .or_else(|| self.items.last())
            .map(|item| item.id);
    }
}

/// 卡片缩略图的目标边长（物理像素）。
///
/// 卡片本身约 236px 宽，按 2x DPI 留到 480，再大就是浪费 IPC。
const THUMBNAIL_EDGE: u32 = 480;

/// 解码 + 缩放 + base64 的结果缓存，key 是 blob 文件名。
///
/// 没有这个缓存的话，每一次推送（搜索、置顶、新增）都会把列表里所有图片
/// 重新解码一遍——这正是之前 base64 方案卡顿的根源。缓存之后同一张图只算一次，
/// 从「每次推送 N 张」降到「整个进程生命周期每张一次」。
///
/// 缩略图缓存是独立的，不受 AppState 归属影响，所以仍然用一把小锁——
/// 它只保护纯内存哈希表，临界区极短，且调用方都在 owner 线程上，不存在竞争。
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

/// 按 blob 文件名取缩略图 data URL，供 `HostRequest::LoadThumbnail` 使用。
pub fn thumbnail_for_blob(blobs_dir: &std::path::Path, file_name: &str) -> Option<String> {
    thumbnail_data_url(blobs_dir, file_name)
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
    /// RFC3339 原文。前端据此算「N 分钟前」，`created_at_label` 留给 tooltip。
    pub created_at: String,
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
    pub fn from_item(item: &ClipboardItem, _blobs_dir: &std::path::Path) -> Self {
        let has_image = matches!(item.item_type, ClipboardItemType::Image)
            && item.blob_path.is_some();

        Self {
            id: item.id,
            r#type: item.item_type.as_str().to_string(),
            content: item.content.clone(),
            preview: item.preview(),
            created_at_label: item.created_at_label(),
            created_at: item.created_at.to_rfc3339(),
            pinned: item.pinned,
            tag: item.tag.clone(),
            // 截断阈值是入库时的字符上限，到顶即认为被裁剪过
            truncated: item.content.chars().count() >= MAX_CONTENT_CHARS,
            has_image,
        }
    }
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

    #[test]
    fn test_selection_stays_at_deleted_slot() {
        let root = std::env::temp_dir().join(format!("test_wp_sel_del_{}", std::process::id()));
        let paths = AppPaths::at(root.clone());
        let mut state = AppState::new(paths);

        let item = |id: i64| ClipboardItem {
            id,
            item_type: ClipboardItemType::Text,
            content: format!("item {id}"),
            blob_path: None,
            hash: format!("h{id}"),
            created_at: Local::now(),
            pinned: false,
            tag: None,
        };

        // 中间项被删：原下标现在是下一张卡
        state.items = vec![item(1), item(2), item(3)];
        state.selected_id = Some(2);
        state.items.remove(1);
        state.select_at_index_or_last(1);
        assert_eq!(state.selected_id, Some(3), "应停在原槽位，而不是第一项");

        // 末尾项被删：下标越界，落到新的最后一项
        state.items = vec![item(1), item(2), item(3)];
        state.selected_id = Some(3);
        state.items.remove(2);
        state.select_at_index_or_last(2);
        assert_eq!(state.selected_id, Some(2), "越界时应落到列表末尾");

        // 唯一一项被删：空列表不选中任何东西
        state.items = vec![item(1)];
        state.selected_id = Some(1);
        state.items.clear();
        state.select_at_index_or_last(0);
        assert_eq!(state.selected_id, None);

        drop(state);
        let _ = std::fs::remove_dir_all(&root);
    }
}