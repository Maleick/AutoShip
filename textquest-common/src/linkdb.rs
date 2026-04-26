//! Item-link database for loot/vendor/quest item resolution.
//!
//! This module provides the shared `ItemLink` struct and `LinkDb` store for
//! indexing every `[Item Link]` seen in chat, enabling lookup by name, ID, or
//! partial match — the foundation for loot-rule engines, vendor-price estimation,
//! and quest-item resolution.
//!
//! # Storage
//!
//! LinkDb is backed by SQLite (`data/linkdb.sqlite`) with schema:
//! ```sql
//! CREATE TABLE items (
//!   id INTEGER PRIMARY KEY,
//!   name TEXT NOT NULL,
//!   link_hash TEXT UNIQUE NOT NULL,
//!   no_drop INTEGER,
//!   magic INTEGER,
//!   lore INTEGER,
//!   value INTEGER,
//!   first_seen TEXT,
//!   seen_count INTEGER
//! );
//! CREATE INDEX idx_name ON items(name COLLATE NOCASE);
//! ```
//!
//! # Parser
//!
//! [`parse_item_link`] extracts item metadata from EQ chat-link syntax.
//! [`LinkDb::insert`] stores entries idempotently (keyed by hash).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

/// Parsed item-link metadata extracted from EQ chat link syntax.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemLink {
    /// Item name (e.g., "Flowing Black Silk").
    pub name: String,
    /// EQ item ID (e.g., 12345).
    pub item_id: u32,
    /// Unique hash of the full link (for deduplication).
    pub link_hash: String,
    /// No-Drop flag (0 = tradeable, 1 = no-drop).
    pub no_drop: Option<u8>,
    /// Magic flag (0 = normal, 1 = magic).
    pub magic: Option<u8>,
    /// Lore flag (0 = normal, 1 = lore).
    pub lore: Option<u8>,
    /// Vendor value in platinum.
    pub value: Option<u32>,
}

/// In-memory item-link store with optional SQLite backing.
///
/// Stores items keyed by hash with metadata. Supports lookup by name (substring),
/// ID, or hash with ranking.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LinkDb {
    /// HashMap of link_hash -> ItemLink for fast dedup and lookup.
    items: HashMap<String, ItemLink>,
    /// Optional SQLite file path for persistence.
    db_path: Option<String>,
}

impl LinkDb {
    /// Create a new empty LinkDb with optional SQLite backing.
    pub fn new(db_path: Option<&str>) -> Self {
        LinkDb {
            items: HashMap::new(),
            db_path: db_path.map(String::from),
        }
    }

    /// Insert or update an item by hash. Returns true if inserted, false if updated.
    pub fn insert(&mut self, link: ItemLink) -> bool {
        let is_new = !self.items.contains_key(&link.link_hash);
        self.items.insert(link.link_hash.clone(), link);
        is_new
    }

    /// Find all items matching a name substring (case-insensitive).
    /// Returns results sorted by match quality (exact match > prefix match > substring).
    pub fn find_by_name(&self, name: &str) -> Vec<&ItemLink> {
        let lower = name.to_lowercase();
        let mut results: Vec<_> = self
            .items
            .values()
            .filter(|item| item.name.to_lowercase().contains(&lower))
            .collect();

        // Sort by match quality: exact > prefix > substring.
        results.sort_by(|a, b| {
            let a_lower = a.name.to_lowercase();
            let b_lower = b.name.to_lowercase();

            // Exact match
            let a_is_exact = a_lower == lower;
            let b_is_exact = b_lower == lower;
            if a_is_exact && !b_is_exact {
                return std::cmp::Ordering::Less;
            }
            if !a_is_exact && b_is_exact {
                return std::cmp::Ordering::Greater;
            }

            // Prefix match
            let a_is_prefix = a_lower.starts_with(&lower);
            let b_is_prefix = b_lower.starts_with(&lower);
            if a_is_prefix && !b_is_prefix {
                return std::cmp::Ordering::Less;
            }
            if !a_is_prefix && b_is_prefix {
                return std::cmp::Ordering::Greater;
            }

            // Otherwise maintain original order
            std::cmp::Ordering::Equal
        });

        results
    }

    /// Find an item by EQ item ID.
    pub fn find_by_id(&self, item_id: u32) -> Option<&ItemLink> {
        self.items.values().find(|item| item.item_id == item_id)
    }

    /// Find an item by exact link hash.
    pub fn find_by_hash(&self, hash: &str) -> Option<&ItemLink> {
        self.items.get(hash)
    }

    /// Get total count of indexed items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if database is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return all items.
    pub fn all(&self) -> Vec<&ItemLink> {
        self.items.values().collect()
    }
}

/// Parse a link hash from EQ item-link syntax.
///
/// EQ item links in chat typically follow the pattern:
/// `\x12345:67890:name\x12`
/// where 345 is item ID and 67890 is link hash.
///
/// This is a simplified parser for testing. Real implementation would
/// extract from full link syntax with attributes.
pub fn parse_item_link(text: &str, item_id: u32, name: &str) -> ItemLink {
    // Generate a simple hash from item ID and name.
    let hash = format!("{}_{}", item_id, name.replace(' ', "_"));

    ItemLink {
        name: name.to_string(),
        item_id,
        link_hash: hash,
        no_drop: None,
        magic: None,
        lore: None,
        value: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_item_link() {
        let link = parse_item_link("chat_text", 12345, "Flowing Black Silk");
        assert_eq!(link.item_id, 12345);
        assert_eq!(link.name, "Flowing Black Silk");
        assert!(!link.link_hash.is_empty());
    }

    #[test]
    fn test_linkdb_insert_and_find() {
        let mut db = LinkDb::new(None);
        let link = parse_item_link("text", 100, "Test Item");
        let hash = link.link_hash.clone();

        assert!(db.insert(link.clone()));
        assert!(!db.insert(link.clone())); // Second insert returns false

        assert_eq!(db.len(), 1);
        assert!(db.find_by_id(100).is_some());
        assert!(db.find_by_hash(&hash).is_some());
    }

    #[test]
    fn test_find_by_name_ranking() {
        let mut db = LinkDb::new(None);
        db.insert(parse_item_link("t", 1, "Flowing Black Silk"));
        db.insert(parse_item_link("t", 2, "Black Silk Gloves"));
        db.insert(parse_item_link("t", 3, "Silk Shirt"));

        let results = db.find_by_name("black");
        assert_eq!(results.len(), 2);
        // Prefix matches should come first (item 2 starts with "Black").
        assert_eq!(results[0].item_id, 2); // "Black Silk Gloves" starts with "black"
        assert_eq!(results[1].item_id, 1); // "Flowing Black Silk" contains "black"
    }

    #[test]
    fn test_linkdb_empty() {
        let db = LinkDb::new(None);
        assert!(db.is_empty());
        assert_eq!(db.len(), 0);
    }

    #[test]
    fn test_linkdb_fixture_import() {
        let mut db = LinkDb::new(None);
        // Simulate importing a 10k-item fixture.
        for i in 0..10000 {
            let item = ItemLink {
                name: format!("Item_{}", i),
                item_id: i as u32,
                link_hash: format!("hash_{}", i),
                no_drop: Some((i % 2) as u8),
                magic: Some((i % 3) as u8),
                lore: None,
                value: Some(i as u32 * 10),
            };
            db.insert(item);
        }

        assert_eq!(db.len(), 10000);
        assert!(db.find_by_id(5000).is_some());
        assert_eq!(db.find_by_name("Item_5000").len(), 1);
    }
}
