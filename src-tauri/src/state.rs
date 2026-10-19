//! Shared application state and DTO conversion for the Tauri command layer.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::db::{AppPaths, Database};
use crate::model::{ClipboardItem, ClipboardItemType};

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
        let db = Database::open(&paths.db, HISTORY_CAPACITY).expect("open sqlite database");
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
    pub image_url: Option<String>,
    pub created_at_label: String,
    pub pinned: bool,
    pub tag: Option<String>,
}

impl ClipItemDto {
    pub fn from_item(item: &ClipboardItem, blobs_dir: &PathBuf) -> Self {
        let image_url = match item.item_type {
            ClipboardItemType::Image => item
                .blob_path
                .as_ref()
                .and_then(|name| std::fs::read(blobs_dir.join(name)).ok())
                .map(|bytes| {
                    use base64::Engine;
                    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                    format!("data:image/png;base64,{}", encoded)
                }),
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
