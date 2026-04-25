use crate::Result;
use rusqlite::Connection;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Bookmark {
    pub session_id: String,
    pub start_flag_idx: u64,
    pub end_flag_idx: u64,
    pub label: String,
}

pub struct BookmarkStore {
    bookmarks: Vec<Bookmark>,
}

impl BookmarkStore {
    pub fn load_from_sqlite(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path).map_err(|e| {
            crate::BehaviorCloningError::Dataset(format!("Failed to open bookmarks DB: {}", e))
        })?;

        let mut bookmarks = Vec::new();
        let mut stmt = conn
            .prepare("SELECT session_id, start_flag_idx, end_flag_idx, label FROM bookmarks")
            .map_err(|e| {
                crate::BehaviorCloningError::Dataset(format!(
                    "Failed to prepare SQL statement: {}",
                    e
                ))
            })?;

        let bookmark_iter = stmt
            .query_map([], |row| {
                Ok(Bookmark {
                    session_id: row.get(0)?,
                    start_flag_idx: row.get::<_, i64>(1)? as u64,
                    end_flag_idx: row.get::<_, i64>(2)? as u64,
                    label: row.get(3)?,
                })
            })
            .map_err(|e| {
                crate::BehaviorCloningError::Dataset(format!("Failed to query bookmarks: {}", e))
            })?;

        for result in bookmark_iter {
            let bookmark =
                result.map_err(|e| {
                    crate::BehaviorCloningError::Dataset(format!("Failed to read bookmark: {}", e))
                })?;
            bookmarks.push(bookmark);
        }

        Ok(BookmarkStore { bookmarks })
    }

    pub fn get_bookmarks(&self) -> &[Bookmark] {
        &self.bookmarks
    }

    pub fn filter_by_label(&self, label: &str) -> Vec<&Bookmark> {
        self.bookmarks
            .iter()
            .filter(|b| b.label == label || b.label.is_empty())
            .collect()
    }
}
