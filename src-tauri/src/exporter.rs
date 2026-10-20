//! 导入 / 导出 (Import / Export)
//!
//! 数据形态决定了一件麻烦事：历史分散在 SQLite 和按 sha256 命名的 blob 目录里，
//! 靠 `BlobPath` 一列关联。只拷 `history.db` 不拷 `blobs/`，所有图片都会变成裂图。
//!
//! 所以导出格式设计成**单文件**：JSON 里直接内嵌图片的 base64。
//! 代价是文件变大（base64 约 +33%），换来的是"这一个文件就是全部数据"——
//! 可以发邮件、可以放进任何网盘、换机器时拷过去就能导回去。
//!
//! 导入以 `Hash` 为幂等键：同一个包导入十次，结果与导入一次完全相同。

use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::db::{Database, ImportOutcome};
use crate::model::{clamp_files, clamp_text, ClipboardItem, ClipboardItemType};

/// 导出文件的格式版本。将来若字段变化，靠它做兼容分支。
const FORMAT_VERSION: u32 = 1;

const FORMAT_NAME: &str = "windows-paste-export";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBundle {
    /// 固定为 `windows-paste-export`，导入时用于识别文件是否属于本应用
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub app_version: String,
    pub items: Vec<ExportItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportItem {
    /// "Text" | "Image" | "Files"
    pub item_type: String,
    pub content: String,
    pub hash: String,
    /// RFC3339，保留原始时间
    pub created_at: String,
    pub pinned: bool,
    pub tag: Option<String>,
    /// 图片条目的 PNG 字节（base64）。其他类型为 null。
    pub image_base64: Option<String>,
}

/// 导入预览：让界面先告诉用户"将导入 N 条，其中 M 条已存在"，别让人盲点确认。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub total: usize,
    /// 已存在且不会变化（真正会被跳过）
    pub existing: usize,
    /// 新条目数量
    pub fresh: usize,
    pub format_version: u32,
    pub exported_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub inserted: usize,
    pub merged: usize,
    pub skipped: usize,
    /// 因格式不完整而被丢弃的条目
    pub invalid: usize,
    pub total_after: usize,
}

/// 把全部历史写成单文件 JSON（图片内嵌 base64）。
pub fn export_to_file(
    db: &Database,
    blobs_dir: &Path,
    target: &Path,
) -> Result<usize, String> {
    let items = db.get_all().map_err(|e| format!("读取历史失败: {e}"))?;

    let exported: Vec<ExportItem> = items
        .iter()
        .map(|item| {
            let image_base64 = match item.item_type {
                ClipboardItemType::Image => item
                    .blob_path
                    .as_ref()
                    .and_then(|name| fs::read(blobs_dir.join(name)).ok())
                    .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes)),
                _ => None,
            };

            ExportItem {
                item_type: item.item_type.as_str().to_string(),
                content: item.content.clone(),
                hash: item.hash.clone(),
                created_at: item.created_at.to_rfc3339(),
                pinned: item.pinned,
                tag: item.tag.clone(),
                image_base64,
            }
        })
        .collect();

    let count = exported.len();
    let bundle = ExportBundle {
        format: FORMAT_NAME.to_string(),
        version: FORMAT_VERSION,
        exported_at: Local::now().to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        items: exported,
    };

    let json = serde_json::to_string_pretty(&bundle)
        .map_err(|e| format!("序列化失败: {e}"))?;

    if let Some(parent) = target.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(target, json).map_err(|e| format!("写入文件失败: {e}"))?;

    Ok(count)
}

fn read_bundle(path: &Path) -> Result<ExportBundle, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("读取文件失败: {e}"))?;
    let bundle: ExportBundle =
        serde_json::from_str(&raw).map_err(|e| format!("不是有效的导出文件: {e}"))?;

    if bundle.format != FORMAT_NAME {
        return Err("这个文件不是 Windows Paste 的导出包".to_string());
    }
    if bundle.version > FORMAT_VERSION {
        return Err(format!(
            "导出包版本 {} 高于当前应用支持的版本 {}，请先升级",
            bundle.version, FORMAT_VERSION
        ));
    }
    Ok(bundle)
}

/// 统计导入会发生什么，不写库。
pub fn preview_import(db: &Database, path: &Path) -> Result<ImportPreview, String> {
    let bundle = read_bundle(path)?;

    let mut existing = 0;
    for item in &bundle.items {
        if let Ok(Some(_)) = db.find_id_by_hash(&item.hash) {
            existing += 1;
        }
    }

    Ok(ImportPreview {
        total: bundle.items.len(),
        existing,
        fresh: bundle.items.len().saturating_sub(existing),
        format_version: bundle.version,
        exported_at: bundle.exported_at,
    })
}

/// 真正执行导入。以 hash 为幂等键，已在库中的条目只合并元数据。
pub fn import_from_file(
    db: &mut Database,
    blobs_dir: &Path,
    path: &Path,
) -> Result<ImportResult, String> {
    let bundle = read_bundle(path)?;

    let mut inserted = 0;
    let mut merged = 0;
    let mut skipped = 0;
    let mut invalid = 0;

    for entry in &bundle.items {
        let Some(item_type) = ClipboardItemType::from_label(&entry.item_type) else {
            invalid += 1;
            continue;
        };

        let created_at = DateTime::parse_from_rfc3339(&entry.created_at)
            .map(|dt| dt.with_timezone(&Local))
            .unwrap_or_else(|_| Local::now());

        // 图片条目要把 PNG 落回 blob 目录，文件名沿用内容 hash 命名规则（幂等）
        let blob_path = match &entry.image_base64 {
            Some(encoded) if item_type == ClipboardItemType::Image => {
                match base64::engine::general_purpose::STANDARD.decode(encoded) {
                    Ok(bytes) if !bytes.is_empty() => {
                        let file_name = format!("{}.png", entry.hash);
                        let full = blobs_dir.join(&file_name);
                        if !full.exists() {
                            if fs::write(&full, &bytes).is_err() {
                                invalid += 1;
                                continue;
                            }
                        }
                        Some(file_name)
                    }
                    _ => {
                        invalid += 1;
                        continue;
                    }
                }
            }
            _ => None,
        };

        let content = match item_type {
            ClipboardItemType::Files => clamp_files(&entry.content),
            _ => clamp_text(&entry.content),
        };

        let mut item = ClipboardItem {
            id: 0,
            item_type,
            content,
            blob_path,
            hash: entry.hash.clone(),
            created_at,
            pinned: entry.pinned,
            tag: entry.tag.clone(),
        };

        match db.upsert_imported(&mut item) {
            Ok(ImportOutcome::Inserted) => inserted += 1,
            Ok(ImportOutcome::Merged) => merged += 1,
            Ok(ImportOutcome::Skipped) => skipped += 1,
            Err(_) => invalid += 1,
        }
    }

    Ok(ImportResult {
        inserted,
        merged,
        skipped,
        invalid,
        total_after: db.get_total_count().unwrap_or(0),
    })
}

/// 给导出文件起一个带时间戳的默认名字。
pub fn default_export_name() -> String {
    format!(
        "windows-paste-{}.json",
        Local::now().format("%Y%m%d-%H%M%S")
    )
}

/// 用户主目录下的 `Documents`，作为文件对话框的默认落点。
pub fn default_export_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .map(|home| home.join("Documents"))
        .filter(|p| p.exists())
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::AppPaths;

    fn sample_text(hash: &str, content: &str, tag: Option<&str>, pinned: bool) -> ClipboardItem {
        ClipboardItem {
            id: 0,
            item_type: ClipboardItemType::Text,
            content: content.to_string(),
            blob_path: None,
            hash: hash.to_string(),
            created_at: Local::now(),
            pinned,
            tag: tag.map(|t| t.to_string()),
        }
    }

    #[test]
    fn test_export_import_roundtrip_is_idempotent() {
        let root = std::env::temp_dir().join(format!("test_wp_export_{}", std::process::id()));
        let paths = AppPaths::at(root.clone());
        let export_path = root.join("bundle.json");

        let mut source = Database::open(&paths.db, 100).expect("open db");
        let mut a = sample_text("h1", "一段代码", Some("代码"), true);
        let mut b = sample_text("h2", "https://tauri.app", None, false);
        source.add(&mut a).unwrap();
        source.add(&mut b).unwrap();

        let count = export_to_file(&source, &paths.blobs, &export_path).unwrap();
        assert_eq!(count, 2);

        // 导入到一个全新的库
        let other_root = root.join("other");
        let other = AppPaths::at(other_root.clone());
        let mut target = Database::open(&other.db, 100).expect("open db");

        let preview = preview_import(&target, &export_path).unwrap();
        assert_eq!(preview.total, 2);
        assert_eq!(preview.fresh, 2);
        assert_eq!(preview.existing, 0);

        let result = import_from_file(&mut target, &other.blobs, &export_path).unwrap();
        assert_eq!(result.inserted, 2);
        assert_eq!(result.total_after, 2);

        // 再导一次：全部跳过，总数不变 —— 这就是幂等
        let result2 = import_from_file(&mut target, &other.blobs, &export_path).unwrap();
        assert_eq!(result2.inserted, 0);
        assert_eq!(result2.skipped, 2);
        assert_eq!(result2.total_after, 2);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_import_restores_pinned_and_tag() {
        let root = std::env::temp_dir().join(format!("test_wp_export_meta_{}", std::process::id()));
        let paths = AppPaths::at(root.clone());
        let export_path = root.join("bundle.json");

        let mut source = Database::open(&paths.db, 100).expect("open db");
        let mut a = sample_text("meta1", "重要内容", Some("工作"), true);
        source.add(&mut a).unwrap();
        export_to_file(&source, &paths.blobs, &export_path).unwrap();

        let other_root = root.join("other");
        let other = AppPaths::at(other_root.clone());
        let mut target = Database::open(&other.db, 100).expect("open db");
        import_from_file(&mut target, &other.blobs, &export_path).unwrap();

        let restored = &target.get_all().unwrap()[0];
        assert_eq!(restored.tag.as_deref(), Some("工作"));
        assert!(restored.pinned, "置顶状态必须恢复");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_image_bytes_survive_the_roundtrip() {
        let root = std::env::temp_dir().join(format!("test_wp_export_img_{}", std::process::id()));
        let paths = AppPaths::at(root.clone());
        let export_path = root.join("bundle.json");

        let mut source = Database::open(&paths.db, 100).expect("open db");

        // 一个最小但合法的 PNG（1x1 透明像素）
        let png: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        fs::write(paths.blobs.join("pic.png"), &png).unwrap();

        let mut image_item = ClipboardItem {
            id: 0,
            item_type: ClipboardItemType::Image,
            content: "1 x 1 image".to_string(),
            blob_path: Some("pic.png".to_string()),
            hash: "imagehash".to_string(),
            created_at: Local::now(),
            pinned: false,
            tag: None,
        };
        source.add(&mut image_item).unwrap();

        export_to_file(&source, &paths.blobs, &export_path).unwrap();

        // 导入到一个全新的库，图片必须被重新落盘
        let other_root = root.join("other");
        let other = AppPaths::at(other_root.clone());
        let mut target = Database::open(&other.db, 100).expect("open db");
        let result = import_from_file(&mut target, &other.blobs, &export_path).unwrap();

        assert_eq!(result.inserted, 1);
        let restored = &target.get_all().unwrap()[0];
        assert_eq!(restored.item_type, ClipboardItemType::Image);

        let blob_name = restored.blob_path.as_ref().expect("图片必须有 blob 路径");
        let restored_bytes = fs::read(other.blobs.join(blob_name)).expect("图片文件应已还原");
        assert_eq!(restored_bytes, png, "PNG 字节必须逐字节一致");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_rejects_foreign_file() {
        let root = std::env::temp_dir().join(format!("test_wp_export_bad_{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("not-ours.json");
        fs::write(&path, r#"{"hello":"world"}"#).unwrap();

        let paths = AppPaths::at(root.join("db"));
        let db = Database::open(&paths.db, 10).expect("open db");

        assert!(preview_import(&db, &path).is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
