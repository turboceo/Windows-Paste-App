use chrono::{DateTime, Local};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardItemType {
    Text = 0,
    Image = 1,
    Files = 2,
}

impl ClipboardItemType {
    pub fn from_i32(val: i32) -> Self {
        match val {
            1 => Self::Image,
            2 => Self::Files,
            _ => Self::Text,
        }
    }

    pub fn to_i32(&self) -> i32 {
        *self as i32
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Image => "Image",
            Self::Files => "Files",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClipboardItem {
    pub id: i64,
    pub item_type: ClipboardItemType,
    pub content: String,
    pub blob_path: Option<String>,
    pub hash: String,
    pub created_at: DateTime<Local>,
    pub pinned: bool,
}

impl ClipboardItem {
    pub fn preview(&self) -> String {
        match self.item_type {
            ClipboardItemType::Image => {
                if self.content.is_empty() {
                    "Image".to_string()
                } else {
                    self.content.clone()
                }
            }
            _ => {
                let flat = self.content.replace(['\r', '\n'], " ");
                let trimmed = flat.trim();
                if trimmed.chars().count() <= 200 {
                    trimmed.to_string()
                } else {
                    trimmed.chars().take(200).collect()
                }
            }
        }
    }

    pub fn created_at_label(&self) -> String {
        self.created_at.format("%Y-%m-%d %H:%M").to_string()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_item_type_conversion() {
        assert_eq!(ClipboardItemType::from_i32(0), ClipboardItemType::Text);
        assert_eq!(ClipboardItemType::from_i32(1), ClipboardItemType::Image);
        assert_eq!(ClipboardItemType::from_i32(2), ClipboardItemType::Files);
        assert_eq!(ClipboardItemType::from_i32(99), ClipboardItemType::Text);

        assert_eq!(ClipboardItemType::Text.to_i32(), 0);
        assert_eq!(ClipboardItemType::Image.to_i32(), 1);
        assert_eq!(ClipboardItemType::Files.to_i32(), 2);
    }

    #[test]
    fn test_preview_and_date_label() {
        let item = ClipboardItem {
            id: 1,
            item_type: ClipboardItemType::Text,
            content: "Hello\r\nWorld   Test".to_string(),
            blob_path: None,
            hash: "h".to_string(),
            created_at: Local::now(),
            pinned: false,
        };
        assert_eq!(item.preview(), "Hello  World   Test");
        assert!(!item.created_at_label().is_empty());
    }
}
