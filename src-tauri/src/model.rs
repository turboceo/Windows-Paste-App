use chrono::{DateTime, Local};

/// 单条记录 `content` 允许保留的最大字符数。
///
/// 超出部分在入库前截断：一条几 MB 的日志如果不截，会跟着每一次全量推送
/// 反复穿过 IPC，把面板拖垮。`hash` 仍按原始内容计算，因此去重语义不受影响。
pub const MAX_CONTENT_CHARS: usize = 8192;

/// 文件列表条目允许保留的最大路径数（按整行截断，避免截出半个路径）。
pub const MAX_FILE_PATHS: usize = 500;

/// 按字符边界截断普通文本，保证不会切坏 UTF-8。
pub fn clamp_text(s: &str) -> String {
    if s.chars().count() <= MAX_CONTENT_CHARS {
        return s.to_string();
    }
    s.chars().take(MAX_CONTENT_CHARS).collect()
}

/// 按行截断文件路径列表：宁可少几条，也不留下无法粘贴的半个路径。
pub fn clamp_files(s: &str) -> String {
    let mut out = String::new();
    for (index, line) in s.lines().enumerate() {
        if index >= MAX_FILE_PATHS || out.len() + line.len() > MAX_CONTENT_CHARS {
            break;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }
    if out.is_empty() {
        // 极端情况：单行就超出上限，退回按字符截断
        return clamp_text(s);
    }
    out
}

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

    /// 从导入文件里的类型字符串还原；未知类型返回 `None`。
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "Text" | "text" => Some(Self::Text),
            "Image" | "image" => Some(Self::Image),
            "Files" | "files" => Some(Self::Files),
            _ => None,
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
    pub tag: Option<String>,
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
    fn test_from_label_roundtrip() {
        for kind in [
            ClipboardItemType::Text,
            ClipboardItemType::Image,
            ClipboardItemType::Files,
        ] {
            assert_eq!(ClipboardItemType::from_label(kind.as_str()), Some(kind));
        }
        assert_eq!(ClipboardItemType::from_label("Html"), None);
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
            tag: None,
        };
        assert_eq!(item.preview(), "Hello  World   Test");
        assert!(!item.created_at_label().is_empty());
    }

    #[test]
    fn test_clamp_text_respects_char_boundary() {
        let short = "短文本";
        assert_eq!(clamp_text(short), short);

        let long: String = "中".repeat(MAX_CONTENT_CHARS + 50);
        let clamped = clamp_text(&long);
        assert_eq!(clamped.chars().count(), MAX_CONTENT_CHARS);
        // 必须仍是合法 UTF-8（能正常构造 String 即已验证）
        assert!(clamped.chars().all(|c| c == '中'));
    }

    #[test]
    fn test_clamp_files_keeps_whole_paths() {
        let paths: Vec<String> = (0..MAX_FILE_PATHS + 20)
            .map(|i| format!("C:\\Users\\Pyzl\\Documents\\file-{}.txt", i))
            .collect();
        let joined = paths.join("\n");
        let clamped = clamp_files(&joined);
        let lines: Vec<&str> = clamped.lines().collect();

        assert!(lines.len() <= MAX_FILE_PATHS);
        // 每一行都必须是完整路径，不能出现半截
        for line in lines {
            assert!(line.starts_with("C:\\Users\\Pyzl\\Documents\\"));
            assert!(line.ends_with(".txt"));
        }
    }
}
