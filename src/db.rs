use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Local};
use rusqlite::{params, Connection, Result};

use crate::model::{ClipboardItem, ClipboardItemType};

pub struct AppPaths {
    pub root: PathBuf,
    pub blobs: PathBuf,
    pub db: PathBuf,
}

impl AppPaths {
    pub fn get() -> Self {
        let local_app_data = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Users\Default\AppData\Local"));
        let root = local_app_data.join("WindowsPaste");
        let blobs = root.join("blobs");
        let db = root.join("history.db");

        let _ = fs::create_dir_all(&root);
        let _ = fs::create_dir_all(&blobs);

        Self { root, blobs, db }
    }
}

pub struct Database {
    conn: Connection,
    capacity: usize,
}

impl Database {
    pub fn open(db_path: &Path, capacity: usize) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Self { conn, capacity };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS ClipboardItems (
                Id        INTEGER PRIMARY KEY AUTOINCREMENT,
                Type      INTEGER NOT NULL,
                Content   TEXT    NOT NULL,
                BlobPath  TEXT    NULL,
                Hash      TEXT    NOT NULL,
                CreatedAt TEXT    NOT NULL,
                Pinned    INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS IX_ClipboardItems_Hash ON ClipboardItems(Hash);
            CREATE INDEX IF NOT EXISTS IX_ClipboardItems_CreatedAt ON ClipboardItems(CreatedAt DESC);",
        )?;
        Ok(())
    }

    pub fn get_most_recent_hash(&self) -> Option<String> {
        let mut stmt = self
            .conn
            .prepare("SELECT Hash FROM ClipboardItems ORDER BY CreatedAt DESC, Id DESC LIMIT 1")
            .ok()?;
        stmt.query_row([], |row| row.get(0)).ok()
    }

    pub fn add(&mut self, item: &mut ClipboardItem) -> Result<bool> {
        // De-duplication check: if newest entry has identical hash, skip
        if let Some(recent_hash) = self.get_most_recent_hash() {
            if recent_hash == item.hash {
                return Ok(false);
            }
        }

        self.conn.execute(
            "INSERT INTO ClipboardItems (Type, Content, BlobPath, Hash, CreatedAt, Pinned)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                item.item_type.to_i32(),
                item.content,
                item.blob_path,
                item.hash,
                item.created_at.to_rfc3339(),
                if item.pinned { 1 } else { 0 },
            ],
        )?;

        item.id = self.conn.last_insert_rowid();
        self.trim()?;
        Ok(true)
    }

    pub fn get_all(&self) -> Result<Vec<ClipboardItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT Id, Type, Content, BlobPath, Hash, CreatedAt, Pinned
             FROM ClipboardItems
             ORDER BY Pinned DESC, CreatedAt DESC, Id DESC",
        )?;
        let rows = stmt.query_map([], |row| Self::parse_row(row))?;
        let mut items = Vec::new();
        for item in rows.flatten() {
            items.push(item);
        }
        Ok(items)
    }

    pub fn search(&self, term: &str) -> Result<Vec<ClipboardItem>> {
        let trimmed = term.trim();
        if trimmed.is_empty() {
            return self.get_all();
        }

        let pattern = format!("%{}%", trimmed);
        let mut stmt = self.conn.prepare(
            "SELECT Id, Type, Content, BlobPath, Hash, CreatedAt, Pinned
             FROM ClipboardItems
             WHERE Content LIKE ?1
             ORDER BY Pinned DESC, CreatedAt DESC, Id DESC
             LIMIT 200",
        )?;
        let rows = stmt.query_map(params![pattern], |row| Self::parse_row(row))?;
        let mut items = Vec::new();
        for item in rows.flatten() {
            items.push(item);
        }
        Ok(items)
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE ClipboardItems SET Pinned = ?1 WHERE Id = ?2",
            params![if pinned { 1 } else { 0 }, id],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM ClipboardItems WHERE Id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn clear(&self) -> Result<Vec<String>> {
        let blobs = {
            let mut stmt = self.conn.prepare(
                "SELECT BlobPath FROM ClipboardItems WHERE BlobPath IS NOT NULL",
            )?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            rows.flatten().collect()
        };

        self.conn.execute("DELETE FROM ClipboardItems", [])?;
        Ok(blobs)
    }

    pub fn get_total_count(&self) -> Result<usize> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM ClipboardItems")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count as usize)
    }

    fn trim(&self) -> Result<()> {
        // Keep pinned rows intact, evict oldest unpinned exceeding capacity
        let mut stmt = self.conn.prepare(
            "SELECT Id, BlobPath FROM ClipboardItems
             WHERE Pinned = 0
             ORDER BY CreatedAt DESC, Id DESC
             LIMIT -1 OFFSET ?1",
        )?;

        let stale: Vec<(i64, Option<String>)> = stmt
            .query_map(params![self.capacity], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .flatten()
            .collect();

        for (id, blob) in stale {
            let _ = self.delete(id);
            if let Some(b) = blob {
                let paths = AppPaths::get();
                let blob_file = paths.blobs.join(b);
                let _ = fs::remove_file(blob_file);
            }
        }
        Ok(())
    }

    fn parse_row(row: &rusqlite::Row) -> rusqlite::Result<ClipboardItem> {
        let id: i64 = row.get(0)?;
        let item_type_num: i32 = row.get(1)?;
        let content: String = row.get(2)?;
        let blob_path: Option<String> = row.get(3)?;
        let hash: String = row.get(4)?;
        let date_str: String = row.get(5)?;
        let pinned_num: i32 = row.get(6)?;

        let created_at = DateTime::parse_from_rfc3339(&date_str)
            .map(|dt| dt.with_timezone(&Local))
            .unwrap_or_else(|_| Local::now());

        Ok(ClipboardItem {
            id,
            item_type: ClipboardItemType::from_i32(item_type_num),
            content,
            blob_path,
            hash,
            created_at,
            pinned: pinned_num != 0,
        })
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_crud_and_deduplication() {
        let temp_dir = std::env::temp_dir().join(format!("test_wp_db_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);
        let db_path = temp_dir.join("test.db");

        let mut db = Database::open(&db_path, 5).expect("open db");

        let mut item1 = ClipboardItem {
            id: 0,
            item_type: ClipboardItemType::Text,
            content: "Hello Rust".to_string(),
            blob_path: None,
            hash: "hash1".to_string(),
            created_at: Local::now(),
            pinned: false,
        };
        assert!(db.add(&mut item1).unwrap());
        assert_eq!(db.get_total_count().unwrap(), 1);

        // 重复哈希防抖：相同 hash 不重复入库
        let mut item1_dup = item1.clone();
        assert!(!db.add(&mut item1_dup).unwrap());
        assert_eq!(db.get_total_count().unwrap(), 1);

        // 搜索测试
        let search_res = db.search("Rust").unwrap();
        assert_eq!(search_res.len(), 1);
        assert_eq!(search_res[0].content, "Hello Rust");

        // 置顶与删除测试
        db.set_pinned(item1.id, true).unwrap();
        let all = db.get_all().unwrap();
        assert!(all[0].pinned);

        db.delete(item1.id).unwrap();
        assert_eq!(db.get_total_count().unwrap(), 0);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_db_trim_preserves_pinned() {
        let temp_dir = std::env::temp_dir().join(format!("test_wp_trim_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);
        let db_path = temp_dir.join("test.db");

        // 容量上限设为 3
        let mut db = Database::open(&db_path, 3).expect("open db");

        // 插入一条置顶记录
        let mut pinned_item = ClipboardItem {
            id: 0,
            item_type: ClipboardItemType::Text,
            content: "Pinned Note".to_string(),
            blob_path: None,
            hash: "pin_hash".to_string(),
            created_at: Local::now() - chrono::Duration::hours(5),
            pinned: true,
        };
        db.add(&mut pinned_item).unwrap();

        // 插入 4 条普通记录（超过上限 3 条）
        for i in 0..4 {
            let mut it = ClipboardItem {
                id: 0,
                item_type: ClipboardItemType::Text,
                content: format!("Note {}", i),
                blob_path: None,
                hash: format!("hash_{}", i),
                created_at: Local::now() + chrono::Duration::seconds(i as i64),
                pinned: false,
            };
            db.add(&mut it).unwrap();
        }

        let all = db.get_all().unwrap();
        // 置顶记录必须保留
        assert!(all.iter().any(|i| i.content == "Pinned Note" && i.pinned));
        // 未置顶记录被修剪至不超过容量 3 条
        let unpinned_count = all.iter().filter(|i| !i.pinned).count();
        assert!(unpinned_count <= 3);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
