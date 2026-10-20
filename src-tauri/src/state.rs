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

/// One history entry as delivered to the React UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipItemDto {
    pub id: i64,
    /// "Text" | "Image" | "Files"
    pub r#type: String,
    pub content: String,
    pub preview: String,
    /// 图片条目的 blob 文件名（不是完整路径）。前端用 `convertFileSrc`
    /// 走 asset 协议直接加载，避免把图片塞进 IPC payload。
    pub image_url: Option<String>,
    pub created_at_label: String,
    pub pinned: bool,
    pub tag: Option<String>,
    /// 原文长度超过上限、落库时被截断过。
    pub truncated: bool,
}

impl ClipItemDto {
    /// 构造推给前端的 DTO。
    ///
    /// `image_url` 只给**文件名**，由前端用 `convertFileSrc` 拼成 asset URL。
    /// 之前这里每次推送都读盘 + base64，一张几 MB 的截图会在每次搜索、
    /// 每次置顶时被重新编码一遍——IPC 体积和内存峰值都被放大几十倍。
    pub fn from_item(item: &ClipboardItem) -> Self {
        let image_url = match item.item_type {
            ClipboardItemType::Image => item.blob_path.clone(),
            _ => None,
        };

        Self {
            id: item.id,
            r#type: item.item_type.as_str().to_string(),
            content: item.content.clone(),
            preview: item.preview(),
            image_url,
            created_at_label: item.created_at_label(),
            pinned: item.pinned,
            tag: item.tag.clone(),
            // 截断阈值是入库时的字符上限，到顶即认为被裁剪过
            truncated: item.content.chars().count() >= MAX_CONTENT_CHARS,
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
