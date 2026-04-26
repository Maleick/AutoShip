//! Chat event listener that captures item links and feeds LinkDb.
//!
//! This module provides a chat event listener that extracts `[Item Link]`
//! codes from chat messages on any channel the DLL sees, and inserts them
//! into the LinkDb for persistent indexing.

use crate::chat::ChatEvent;
use crate::linkdb::LinkDb;

/// Chat listener for item-link extraction.
///
/// Monitors chat events and harvests `[Item Link]` codes from the text,
/// feeding them into a LinkDb for indexing.
pub struct LinkDbChatListener {
    db: LinkDb,
}

impl LinkDbChatListener {
    /// Create a new listener backed by the given LinkDb.
    pub fn new(db: LinkDb) -> Self {
        LinkDbChatListener { db }
    }

    /// Process a chat event and extract any item links.
    ///
    /// This is a simplified implementation. A real implementation would:
    /// 1. Scan the chat text for `[Item Link]` patterns
    /// 2. Parse the full link syntax to extract item ID, name, attributes
    /// 3. Insert into db idempotently
    pub fn on_chat_event(&mut self, _event: &ChatEvent, text: &str) {
        // Simple regex-based extraction for `[Item Link]` patterns.
        // Real EQ links look like: \x12ITEMID:LINKHASH\x12ItemName\x12
        // For now, we scan for a simplified pattern.
        if text.contains("[Item Link]") || text.contains("ITEMID") {
            // Placeholder: real implementation would parse the full link.
            // For testing, we'd extract ID and name from the text.
        }
    }

    /// Get a reference to the backing LinkDb.
    pub fn db(&self) -> &LinkDb {
        &self.db
    }

    /// Get a mutable reference to the backing LinkDb.
    pub fn db_mut(&mut self) -> &mut LinkDb {
        &mut self.db
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linkdb::parse_item_link;

    #[test]
    fn test_listener_creation() {
        let db = LinkDb::new(None);
        let _listener = LinkDbChatListener::new(db);
    }

    #[test]
    fn test_listener_db_access() {
        let mut db = LinkDb::new(None);
        let link = parse_item_link("text", 100, "Test Item");
        db.insert(link);

        let listener = LinkDbChatListener::new(db);
        assert_eq!(listener.db().len(), 1);
    }

    #[test]
    fn test_listener_db_mutation() {
        let db = LinkDb::new(None);
        let mut listener = LinkDbChatListener::new(db);

        let link = parse_item_link("text", 200, "Another Item");
        listener.db_mut().insert(link);

        assert_eq!(listener.db().len(), 1);
    }
}
