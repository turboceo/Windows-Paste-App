use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Local};
use rusqlite::{params, Connection, OptionalExtension, Result};

use crate::model::{ClipboardItem, ClipboardItemType};

#[derive(Clone)]
pub struct AppPaths {
    pub root: PathBuf,
    pub blobs: PathBuf,
    pub db: PathBuf,
}

impl AppPaths {
    /// 用户数据目录（`%LOCALAPPDATA%\WindowsPaste`）。
    pub fn get() -> Self {
        let local_app_data = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Users\Default\AppData\Local"));
        Self::at(local_app_data.join("WindowsPaste"))
    }

    /// 在指定根目录下组织数据库与 blob 目录。
    ///
    /// 自检 / 种子数据必须用 `temporary_root()` 拿到临时路径再走这里，
    /// 绝不能碰用户的真实历史。
    pub fn at(root: PathBuf) -> Self {
        let blobs = root.join("blobs");
        let db = root.join("history.db");

        let _ = fs::create_dir_all(&root);
        let _ = fs::create_dir_all(&blobs);

        Self { root, blobs, db }
    }

    /// 一次性临时根目录（只给路径，不创建 AppPaths），供 `--selftest` / `--seed` 使用。
    ///
    /// 带上 pid 与时间戳：同一台机器上并行跑多个自检也不会互相踩到同一个库。
    pub fn temporary_root(tag: &str) -> PathBuf {
        let unique = format!(
            "WindowsPaste-{}-{}-{}",
            tag,
            std::process::id(),
            chrono::Local::now().timestamp_millis()
        );
        std::env::temp_dir().join(unique)
    }
}

/// `add()` 的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddOutcome {
    /// 新条目。
    Inserted,
    /// 同一内容已存在：已把它的时间戳刷新到当下，列表顺序随之改变。
    Touched,
}

/// `upsert_imported()` 的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportOutcome {
    Inserted,
    /// 已存在，但置顶 / 标签被合并更新了。
    Merged,
    /// 已存在且无变化。
    Skipped,
}

/// 删除 `blobs/` 目录里没有任何记录引用的文件。
///
/// 用于自愈历史遗留的泄漏（`clear()` 曾经把待删列表丢掉）。
/// 返回被清理的文件数。
pub fn sweep_orphan_blobs(db: &Database, blobs_dir: &Path) -> usize {
    let referenced = match db.referenced_blobs() {
        Ok(set) => set,
        Err(_) => return 0,
    };

    let entries = match fs::read_dir(blobs_dir) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };

    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("png") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !referenced.contains(name) && fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

pub struct Database {
    conn: Connection,
    capacity: usize,
    /// 该库对应的 blob 目录。
    ///
    /// 之前 `trim()` 里用 `AppPaths::get()` 现拼路径，一旦库被指向别处
    /// （自检、`--data-dir`、导入到临时目录），删文件就会删错目录。
    blobs_dir: PathBuf,
}

impl Database {
    /// 打开数据库，blob 目录取该库所在目录下的 `blobs/`。
    pub fn open(db_path: &Path, capacity: usize) -> Result<Self> {
        let blobs_dir = db_path
            .parent()
            .map(|dir| dir.join("blobs"))
            .unwrap_or_else(|| PathBuf::from("blobs"));
        Self::open_with_blobs(db_path, capacity, blobs_dir)
    }

    /// 显式指定 blob 目录，便于测试与 `--data-dir`。
    pub fn open_with_blobs(db_path: &Path, capacity: usize, blobs_dir: PathBuf) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        // WAL：读写不互相阻塞。万一同一个库被第二个进程打开（例如用户从别处
        // 拷了一份正在用的库进来），也不至于立刻报 database is locked。
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "NORMAL");
        let db = Self {
            conn,
            capacity,
            blobs_dir,
        };
        db.init()?;
        Ok(db)
    }

    pub fn blobs_dir(&self) -> &Path {
        &self.blobs_dir
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
                Pinned    INTEGER NOT NULL DEFAULT 0,
                Tag       TEXT    NULL,
                SortOrder REAL    NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS IX_ClipboardItems_Hash ON ClipboardItems(Hash);
            CREATE INDEX IF NOT EXISTS IX_ClipboardItems_CreatedAt ON ClipboardItems(CreatedAt DESC);",
        )?;
        // 平滑升级兼容已有库
        let _ = self.conn.execute("ALTER TABLE ClipboardItems ADD COLUMN Tag TEXT NULL", []);
        let _ = self.conn.execute(
            "ALTER TABLE ClipboardItems ADD COLUMN SortOrder REAL NOT NULL DEFAULT 0",
            [],
        );
        let _ = self.conn.execute("CREATE INDEX IF NOT EXISTS IX_ClipboardItems_Tag ON ClipboardItems(Tag);", []);
        Ok(())
    }

    /// 按 hash 查已有条目的 id。导入与去重都靠它。
    ///
    /// 这也是 `IX_ClipboardItems_Hash` 这个索引存在的意义——
    /// 之前建了却没被任何查询用上。
    pub fn find_id_by_hash(&self, hash: &str) -> Result<Option<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT Id FROM ClipboardItems WHERE Hash = ?1 LIMIT 1")?;
        stmt.query_row(params![hash], |row| row.get(0))
            .optional()
    }

    /// 写入一条新记录。
    ///
    /// 返回 `AddOutcome`：
    /// - `Inserted`：新条目
    /// - `Touched`：同一内容已经在历史里，把它的时间戳刷新到当下（"提到最前"）
    ///
    /// 为什么重复项要"提到最前"而不是忽略：用户复制 A、B、再复制 A 时，
    /// 若只是忽略，列表顶部仍然是 B —— 与他刚才的操作对不上。
    /// 同时这也让"同一次复制触发两三次 WM_CLIPBOARDUPDATE"天然幂等。
    pub fn add(&mut self, item: &mut ClipboardItem) -> Result<AddOutcome> {
        if let Some(existing_id) = self.find_id_by_hash(&item.hash)? {
            // "提到最前"要同时重置手动排序：否则一条被拖到后面的记录，
            // 即使重新复制也不会回到顶部，和用户的直觉相反。
            self.conn.execute(
                "UPDATE ClipboardItems SET CreatedAt = ?1, SortOrder = 0 WHERE Id = ?2",
                params![item.created_at.to_rfc3339(), existing_id],
            )?;
            item.id = existing_id;
            return Ok(AddOutcome::Touched);
        }

        self.conn.execute(
            "INSERT INTO ClipboardItems (Type, Content, BlobPath, Hash, CreatedAt, Pinned, Tag)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                item.item_type.to_i32(),
                item.content,
                item.blob_path,
                item.hash,
                item.created_at.to_rfc3339(),
                if item.pinned { 1 } else { 0 },
                item.tag,
            ],
        )?;

        item.id = self.conn.last_insert_rowid();
        self.trim()?;
        Ok(AddOutcome::Inserted)
    }

    /// 导入专用：按 hash 幂等写入，已存在时只更新用户产出的元数据。
    ///
    /// 不碰 `CreatedAt`（保留原始时间），也不碰 `Content`——同一个 hash
    /// 意味着内容相同，覆盖没有意义却可能引入差异。
    pub fn upsert_imported(&mut self, item: &mut ClipboardItem) -> Result<ImportOutcome> {
        if let Some(existing_id) = self.find_id_by_hash(&item.hash)? {
            let (current_pinned, current_tag): (i32, Option<String>) = self
                .conn
                .query_row(
                    "SELECT Pinned, Tag FROM ClipboardItems WHERE Id = ?1",
                    params![existing_id],
                    |row| Ok((row.get(0)?, row.get(1).ok())),
                )?;

            // 合并规则：置顶取并集，标签只在本地为空时补上——都不覆盖用户已有的选择
            let next_pinned = current_pinned != 0 || item.pinned;
            let next_tag: Option<String> = match current_tag.as_deref() {
                Some(existing) if !existing.is_empty() => Some(existing.to_string()),
                _ => item.tag.clone(),
            };

            item.id = existing_id;

            if next_pinned == (current_pinned != 0) && next_tag == current_tag {
                return Ok(ImportOutcome::Skipped);
            }

            self.conn.execute(
                "UPDATE ClipboardItems SET Pinned = ?1, Tag = ?2 WHERE Id = ?3",
                params![if next_pinned { 1 } else { 0 }, next_tag, existing_id],
            )?;
            return Ok(ImportOutcome::Merged);
        }

        self.conn.execute(
            "INSERT INTO ClipboardItems (Type, Content, BlobPath, Hash, CreatedAt, Pinned, Tag)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                item.item_type.to_i32(),
                item.content,
                item.blob_path,
                item.hash,
                item.created_at.to_rfc3339(),
                if item.pinned { 1 } else { 0 },
                item.tag,
            ],
        )?;
        item.id = self.conn.last_insert_rowid();
        self.trim()?;
        Ok(ImportOutcome::Inserted)
    }

    pub fn get_all(&self) -> Result<Vec<ClipboardItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT Id, Type, Content, BlobPath, Hash, CreatedAt, Pinned, Tag
             FROM ClipboardItems
             ORDER BY Pinned DESC, SortOrder ASC, CreatedAt DESC, Id DESC",
        )?;
        let rows = stmt.query_map([], |row| Self::parse_row(row))?;
        let mut items = Vec::new();
        for item in rows.flatten() {
            items.push(item);
        }
        Ok(items)
    }

    pub fn search(&self, term: &str) -> Result<Vec<ClipboardItem>> {
        self.get_filtered(term, None)
    }

    /// 综合检索：支持同时按关键字与分类 Tag 筛选
    pub fn get_filtered(&self, search: &str, tag_filter: Option<&str>) -> Result<Vec<ClipboardItem>> {
        let search_trimmed = search.trim();
        let tag_trimmed = tag_filter.map(|t| t.trim()).filter(|t| !t.is_empty());

        match (search_trimmed.is_empty(), tag_trimmed) {
            (true, None) => {
                return self.get_all();
            }
            (false, None) => {
                let pattern = format!("%{}%", search_trimmed);
                let mut stmt = self.conn.prepare(
                    "SELECT Id, Type, Content, BlobPath, Hash, CreatedAt, Pinned, Tag
                     FROM ClipboardItems
                     WHERE Content LIKE ?1
                     ORDER BY Pinned DESC, SortOrder ASC, CreatedAt DESC, Id DESC
                     LIMIT 200",
                )?;
                let rows = stmt.query_map(params![pattern], |row| Self::parse_row(row))?;
                return Ok(rows.flatten().collect());
            }
            (true, Some(tag)) => {
                let mut stmt = self.conn.prepare(
                    "SELECT Id, Type, Content, BlobPath, Hash, CreatedAt, Pinned, Tag
                     FROM ClipboardItems
                     WHERE Tag = ?1
                     ORDER BY Pinned DESC, SortOrder ASC, CreatedAt DESC, Id DESC
                     LIMIT 200",
                )?;
                let rows = stmt.query_map(params![tag], |row| Self::parse_row(row))?;
                return Ok(rows.flatten().collect());
            }
            (false, Some(tag)) => {
                let pattern = format!("%{}%", search_trimmed);
                let mut stmt = self.conn.prepare(
                    "SELECT Id, Type, Content, BlobPath, Hash, CreatedAt, Pinned, Tag
                     FROM ClipboardItems
                     WHERE Tag = ?1 AND Content LIKE ?2
                     ORDER BY Pinned DESC, SortOrder ASC, CreatedAt DESC, Id DESC
                     LIMIT 200",
                )?;
                let rows = stmt.query_map(params![tag, pattern], |row| Self::parse_row(row))?;
                return Ok(rows.flatten().collect());
            }
        };
    }

    pub fn set_tag(&self, id: i64, tag: Option<&str>) -> Result<()> {
        let clean_tag = tag.map(|t| t.trim()).filter(|t| !t.is_empty());
        self.conn.execute(
            "UPDATE ClipboardItems SET Tag = ?1 WHERE Id = ?2",
            params![clean_tag, id],
        )?;
        Ok(())
    }

    /// 更新条目的内容描述（例如图片 OCR 识别文本提取后追加到 Content 中，供搜索与提取）
    pub fn update_content(&self, id: i64, content: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE ClipboardItems SET Content = ?1 WHERE Id = ?2",
            params![content, id],
        )?;
        Ok(())
    }

    pub fn get_all_tags(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT Tag FROM ClipboardItems WHERE Tag IS NOT NULL AND Tag != '' ORDER BY Tag ASC",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.flatten().collect())
    }

    /// 统计某个分类下有多少条记录。确认框要用它告诉用户影响范围。
    pub fn count_by_tag(&self, tag: &str) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM ClipboardItems WHERE Tag = ?1",
            params![tag],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// 一次查出所有分类及各自的条目数。
    ///
    /// 比逐个 `count_by_tag` 少一轮 N+1 查询；`reload()` 每次都要用它。
    pub fn get_tag_counts(&self) -> Result<std::collections::BTreeMap<String, usize>> {
        let mut stmt = self.conn.prepare(
            "SELECT Tag, COUNT(*) FROM ClipboardItems
             WHERE Tag IS NOT NULL AND Tag != ''
             GROUP BY Tag",
        )?;
        let rows = stmt.query_map([], |row| {
            let tag: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((tag, count as usize))
        })?;
        Ok(rows.flatten().collect())
    }

    /// 删除一个分类：把该标签从所有条目上摘掉，条目本身保留。
    ///
    /// 这里没有独立的 Tags 表——标签是条目上的一个字符串字段，
    /// 由 `get_all_tags()` 汇总出来的。所以"删除分类"的语义就是
    /// "让这个字符串不再出现在任何条目上"，最后一个引用消失时标签自然消失。
    ///
    /// 返回受影响的条目数。
    pub fn delete_tag(&self, tag: &str) -> Result<usize> {
        let affected = self.conn.execute(
            "UPDATE ClipboardItems SET Tag = NULL WHERE Tag = ?1",
            params![tag],
        )?;
        Ok(affected)
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE ClipboardItems SET Pinned = ?1 WHERE Id = ?2",
            params![if pinned { 1 } else { 0 }, id],
        )?;
        Ok(())
    }

    /// 把一条记录移动到 `before_id` 之前；`None` 表示移到所在分组的末尾。
    ///
    /// 只重排被拖拽项所属的分组（置顶 / 未置顶）——列表里这两段永远前后分开，
    /// 跨组拖动没有意义。顺序每次按 0..N 重新编号写回，而不是取两个邻居的
    /// 浮点中点：中点会被反复二分，最终耗尽精度。
    pub fn reorder(&mut self, item_id: i64, before_id: Option<i64>) -> Result<()> {
        if before_id == Some(item_id) {
            return Ok(());
        }

        // 先确认条目存在，同时拿到它属于哪一段
        let pinned: i32 = self.conn.query_row(
            "SELECT Pinned FROM ClipboardItems WHERE Id = ?1",
            params![item_id],
            |row| row.get(0),
        )?;

        let mut ids: Vec<i64> = {
            let mut stmt = self.conn.prepare(
                "SELECT Id FROM ClipboardItems
                 WHERE Pinned = ?1
                 ORDER BY SortOrder ASC, CreatedAt DESC, Id DESC",
            )?;
            let rows = stmt.query_map(params![pinned], |row| row.get(0))?;
            rows.flatten().collect()
        };
        ids.retain(|id| *id != item_id);

        // before_id 不在本组（例如拖到了另一段）时退化为"放到本组末尾"，
        // 而不是把记录插到分组外面去。
        let insert_at = before_id
            .and_then(|target| ids.iter().position(|id| *id == target))
            .unwrap_or(ids.len());
        ids.insert(insert_at, item_id);

        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare("UPDATE ClipboardItems SET SortOrder = ?1 WHERE Id = ?2")?;
            for (index, id) in ids.iter().enumerate() {
                stmt.execute(params![index as f64, id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// 删除一行，返回它的 blob 文件名（如果有），由调用方负责删文件。
    pub fn delete(&self, id: i64) -> Result<Option<String>> {
        // 行可能不存在：用 optional() 把 "no rows" 转成 None 而不是错误
        let blob: Option<Option<String>> = self
            .conn
            .query_row(
                "SELECT BlobPath FROM ClipboardItems WHERE Id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        let blob = blob.flatten();

        self.conn.execute(
            "DELETE FROM ClipboardItems WHERE Id = ?1",
            params![id],
        )?;
        Ok(blob)
    }

    /// 清空全部行，返回所有待删的 blob 文件名。
    ///
    /// 调用方<b>必须</b>用返回值去删文件，否则 `blobs/` 会变成永不回收的垃圾桶。
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

    /// 列出数据库里仍在引用的所有 blob 文件名。
    pub fn referenced_blobs(&self) -> Result<HashSet<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT BlobPath FROM ClipboardItems WHERE BlobPath IS NOT NULL")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.flatten().collect())
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
                // 用本库自己的 blob 目录，而不是全局 AppPaths
                let _ = fs::remove_file(self.blobs_dir.join(b));
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
        let tag: Option<String> = row.get(7).ok();

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
            tag,
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
            tag: None,
        };
        assert_eq!(db.add(&mut item1).unwrap(), AddOutcome::Inserted);
        assert_eq!(db.get_total_count().unwrap(), 1);

        // 重复哈希防抖：相同 hash 不重复入库，而是刷新最新一条的时间戳
        let mut item1_dup = item1.clone();
        assert_eq!(db.add(&mut item1_dup).unwrap(), AddOutcome::Touched);
        assert_eq!(db.get_total_count().unwrap(), 1);

        // 复制 A → B → A：A 不应产生副本，而是被"提到最前"
        let mut other = item1.clone();
        other.hash = "hash_other".to_string();
        other.content = "Another".to_string();
        db.add(&mut other).unwrap();
        let mut item1_again = item1.clone();
        item1_again.created_at = Local::now() + chrono::Duration::seconds(5);
        assert_eq!(db.add(&mut item1_again).unwrap(), AddOutcome::Touched);
        assert_eq!(db.get_total_count().unwrap(), 2, "不应产生幽灵副本");
        assert_eq!(
            db.get_all().unwrap()[0].id,
            item1.id,
            "刚复制的内容必须回到列表首位"
        );

        // 搜索测试
        let search_res = db.search("Rust").unwrap();
        assert_eq!(search_res.len(), 1);
        assert_eq!(search_res[0].content, "Hello Rust");

        // 置顶与删除测试
        db.set_pinned(item1.id, true).unwrap();
        let all = db.get_all().unwrap();
        assert!(all[0].pinned);

        // 此时库里还有 "Another" 一条，删掉 item1 后应剩 1
        db.delete(item1.id).unwrap();
        assert_eq!(db.get_total_count().unwrap(), 1);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_db_tag_operations() {
        let temp_dir = std::env::temp_dir().join(format!("test_wp_tag_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);
        let db_path = temp_dir.join("test.db");

        let mut db = Database::open(&db_path, 10).expect("open db");

        let mut item1 = ClipboardItem {
            id: 0,
            item_type: ClipboardItemType::Text,
            content: "fn main() {}".to_string(),
            blob_path: None,
            hash: "hash_code".to_string(),
            created_at: Local::now(),
            pinned: false,
            tag: None,
        };
        db.add(&mut item1).unwrap();

        let mut item2 = ClipboardItem {
            id: 0,
            item_type: ClipboardItemType::Text,
            content: "https://tauri.app".to_string(),
            blob_path: None,
            hash: "hash_link".to_string(),
            created_at: Local::now(),
            pinned: false,
            tag: None,
        };
        db.add(&mut item2).unwrap();

        // 为 item1 设置 Tag
        db.set_tag(item1.id, Some("代码")).unwrap();
        let all_tags = db.get_all_tags().unwrap();
        assert_eq!(all_tags, vec!["代码".to_string()]);

        // 按 Tag 过滤
        let code_items = db.get_filtered("", Some("代码")).unwrap();
        assert_eq!(code_items.len(), 1);
        assert_eq!(code_items[0].id, item1.id);
        assert_eq!(code_items[0].tag, Some("代码".to_string()));

        // 清理 Tag
        db.set_tag(item1.id, None).unwrap();
        assert!(db.get_all_tags().unwrap().is_empty());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_delete_tag_removes_tag_but_keeps_items() {
        let temp_dir = std::env::temp_dir().join(format!("test_wp_deltag_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);
        let mut db = Database::open(&temp_dir.join("test.db"), 100).expect("open db");

        let mut a = sample("dt1", "first", false, Some("代码"));
        let mut b = sample("dt2", "second", false, Some("代码"));
        let mut c = sample("dt3", "third", false, Some("工作"));
        db.add(&mut a).unwrap();
        db.add(&mut b).unwrap();
        db.add(&mut c).unwrap();

        assert_eq!(db.count_by_tag("代码").unwrap(), 2);
        assert_eq!(db.get_all_tags().unwrap(), vec!["代码".to_string(), "工作".to_string()]);

        let affected = db.delete_tag("代码").unwrap();
        assert_eq!(affected, 2, "应报告受影响的条目数");

        // 条目本身必须保留，只是失去分类
        assert_eq!(db.get_total_count().unwrap(), 3, "删分类不能删记录");
        assert_eq!(db.get_all_tags().unwrap(), vec!["工作".to_string()]);
        assert_eq!(db.count_by_tag("代码").unwrap(), 0);

        // 记录内容不受影响
        let all = db.get_all().unwrap();
        assert!(all.iter().any(|i| i.content == "first" && i.tag.is_none()));
        assert!(all.iter().any(|i| i.content == "third" && i.tag.as_deref() == Some("工作")));

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
            tag: None,
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
                tag: None,
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

    fn sample(hash: &str, content: &str, pinned: bool, tag: Option<&str>) -> ClipboardItem {
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
    fn test_import_is_idempotent() {
        let temp_dir = std::env::temp_dir().join(format!("test_wp_import_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);
        let mut db = Database::open(&temp_dir.join("test.db"), 100).expect("open db");

        let mut first = sample("imp1", "imported text", false, Some("代码"));
        assert_eq!(db.upsert_imported(&mut first).unwrap(), ImportOutcome::Inserted);

        // 同一个包再导一次：内容一致、置顶一致、标签一致 → 跳过
        let mut again = sample("imp1", "imported text", false, Some("代码"));
        assert_eq!(db.upsert_imported(&mut again).unwrap(), ImportOutcome::Skipped);
        assert_eq!(db.get_total_count().unwrap(), 1);

        // 新包里这条被置顶了 → 合并置顶，但不重复插入
        let mut pinned_version = sample("imp1", "imported text", true, Some("代码"));
        assert_eq!(
            db.upsert_imported(&mut pinned_version).unwrap(),
            ImportOutcome::Merged
        );
        assert_eq!(db.get_total_count().unwrap(), 1);
        assert!(db.get_all().unwrap()[0].pinned);
    }

    #[test]
    fn test_import_does_not_overwrite_local_tag() {
        let temp_dir = std::env::temp_dir().join(format!("test_wp_imp_tag_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);
        let mut db = Database::open(&temp_dir.join("test.db"), 100).expect("open db");

        let mut local = sample("imp2", "shared", false, Some("工作"));
        db.upsert_imported(&mut local).unwrap();

        // 导入文件里带的是另一个标签——本地已有分类时不应该被覆盖
        let mut incoming = sample("imp2", "shared", false, Some("个人"));
        assert_eq!(db.upsert_imported(&mut incoming).unwrap(), ImportOutcome::Skipped);
        assert_eq!(
            db.get_all().unwrap()[0].tag.as_deref(),
            Some("工作"),
            "本地已有的分类必须保留"
        );
    }

    /// 列表当前顺序（按 content 取），用于断言拖拽排序结果。
    fn contents(db: &Database) -> Vec<String> {
        db.get_all()
            .unwrap()
            .into_iter()
            .map(|item| item.content)
            .collect()
    }

    #[test]
    fn test_reorder_persists_and_keeps_new_items_on_top() {
        let root = std::env::temp_dir().join(format!("test_wp_reorder_{}", std::process::id()));
        let _ = fs::create_dir_all(&root);
        let mut db = Database::open(&root.join("test.db"), 100).expect("open db");

        let mut a = sample("reorder-a", "A", false, None);
        a.created_at = Local::now() - chrono::Duration::seconds(30);
        let mut b = sample("reorder-b", "B", false, None);
        b.created_at = Local::now() - chrono::Duration::seconds(20);
        let mut c = sample("reorder-c", "C", false, None);
        c.created_at = Local::now() - chrono::Duration::seconds(10);
        db.add(&mut a).unwrap();
        db.add(&mut b).unwrap();
        db.add(&mut c).unwrap();
        assert_eq!(contents(&db), vec!["C", "B", "A"], "默认按时间倒序");

        // 把最早的 A 拖到 C 前面
        db.reorder(a.id, Some(c.id)).unwrap();
        assert_eq!(contents(&db), vec!["A", "C", "B"]);

        // 手动排序后，新内容仍然出现在最前（SortOrder 默认 0，时间更新）
        let mut d = sample("reorder-d", "D", false, None);
        db.add(&mut d).unwrap();
        assert_eq!(contents(&db), vec!["D", "A", "C", "B"]);

        // 重新复制 B：touch 语义要同时重置手动排序，否则回不到顶部
        let mut b_again = b.clone();
        b_again.created_at = Local::now() + chrono::Duration::seconds(5);
        assert_eq!(db.add(&mut b_again).unwrap(), AddOutcome::Touched);
        assert_eq!(contents(&db), vec!["B", "D", "A", "C"]);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_reorder_respects_pinned_group() {
        let root = std::env::temp_dir().join(format!("test_wp_reorder_pin_{}", std::process::id()));
        let _ = fs::create_dir_all(&root);
        let mut db = Database::open(&root.join("test.db"), 100).expect("open db");

        let mut a = sample("rp-a", "A", false, None);
        a.created_at = Local::now() - chrono::Duration::seconds(30);
        let mut b = sample("rp-b", "B", false, None);
        b.created_at = Local::now() - chrono::Duration::seconds(20);
        let mut c = sample("rp-c", "C", false, None);
        c.created_at = Local::now() - chrono::Duration::seconds(10);
        let mut p = sample("rp-p", "P", true, None);
        p.created_at = Local::now() - chrono::Duration::seconds(40);
        for item in [&mut a, &mut b, &mut c, &mut p] {
            db.add(item).unwrap();
        }
        assert_eq!(contents(&db), vec!["P", "C", "B", "A"]);

        // 未置顶段内移动：C 拖到 A 前面 → B, C, A
        db.reorder(c.id, Some(a.id)).unwrap();
        assert_eq!(contents(&db), vec!["P", "B", "C", "A"]);

        // 拖拽项不会越出自己所在的分组：置顶段只有 P，移到末尾仍排第一
        db.reorder(p.id, None).unwrap();
        assert_eq!(contents(&db), vec!["P", "B", "C", "A"]);

        // 指向另一组的 before_id 退化为"本组末尾"，不会把置顶项挤下去
        db.reorder(p.id, Some(a.id)).unwrap();
        assert_eq!(contents(&db), vec!["P", "B", "C", "A"]);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_delete_and_clear_release_blobs() {
        let root = std::env::temp_dir().join(format!("test_wp_blobs_{}", std::process::id()));
        let paths = AppPaths::at(root.clone());
        let mut db = Database::open(&paths.db, 100).expect("open db");
        // open() 会把 blob 目录推导成 <db 所在目录>/blobs，正好就是 paths.blobs
        assert_eq!(db.blobs_dir(), paths.blobs);

        // 造两个 blob 文件，一条记录引用其中第一个
        fs::write(paths.blobs.join("a.png"), b"x").unwrap();
        fs::write(paths.blobs.join("orphan.png"), b"y").unwrap();

        let mut item = sample("img1", "1 x 1 image", false, None);
        item.blob_path = Some("a.png".to_string());
        db.add(&mut item).unwrap();

        // 删除应把文件名交还给调用方
        let released = db.delete(item.id).unwrap();
        assert_eq!(released.as_deref(), Some("a.png"));

        // 孤儿扫描：数据库已无引用，a.png 与 orphan.png 都应被清掉
        let removed = sweep_orphan_blobs(&db, &paths.blobs);
        assert_eq!(removed, 2);
        assert!(!paths.blobs.join("a.png").exists());
        assert!(!paths.blobs.join("orphan.png").exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_clear_returns_all_blob_paths() {
        let root = std::env::temp_dir().join(format!("test_wp_clear_{}", std::process::id()));
        let paths = AppPaths::at(root.clone());
        let mut db = Database::open(&paths.db, 100).expect("open db");
        // AppPaths::at 会建好 blobs 目录，open() 推导出的路径应与之一致
        assert_eq!(db.blobs_dir(), paths.blobs);

        for (i, name) in ["p.png", "q.png"].iter().enumerate() {
            let mut item = sample(&format!("img{}", i), "image", false, None);
            item.blob_path = Some((*name).to_string());
            db.add(&mut item).unwrap();
        }

        let released = db.clear().unwrap();
        assert_eq!(released.len(), 2, "clear() 必须把待删的文件名全部交出来");
        assert_eq!(db.get_total_count().unwrap(), 0);

        let _ = fs::remove_dir_all(&root);
    }
}
