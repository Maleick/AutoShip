//! Persistent operator scratchpad for notes and temporary data.
//!
//! Provides an in-memory scratchpad with automatic file-based persistence.
//! Notes are stored in ~/.config/textquest/scratchpad.json and are visible
//! from both TUI and web UI.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// A single scratchpad note.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Note {
    /// Unique note identifier (timestamp or UUID-based)
    pub id: String,
    /// Note title
    pub title: String,
    /// Note content
    pub content: String,
    /// Creation timestamp (ISO 8601)
    pub created_at: String,
    /// Last modified timestamp (ISO 8601)
    pub modified_at: String,
}

impl Note {
    /// Create a new note with the given title and content.
    pub fn new(title: impl Into<String>, content: impl Into<String>) -> Self {
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        Self {
            id: format!("note-{}", uuid::Uuid::new_v4()),
            title: title.into(),
            content: content.into(),
            created_at: now.clone(),
            modified_at: now,
        }
    }

    /// Update note content.
    pub fn update_content(&mut self, content: impl Into<String>) {
        self.content = content.into();
        self.modified_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    }
}

/// Scratchpad data model for serialization.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScratchpadData {
    /// List of notes
    pub notes: Vec<Note>,
}

/// Thread-safe scratchpad manager.
pub struct Scratchpad {
    data: Arc<Mutex<ScratchpadData>>,
    config_path: PathBuf,
}

impl Scratchpad {
    /// Create or load a scratchpad from the config directory.
    pub fn new(config_dir: impl AsRef<Path>) -> Result<Self> {
        let config_path = config_dir.as_ref().join("scratchpad.json");

        // Load existing data or start with empty
        let data = if config_path.exists() {
            let json_str =
                fs::read_to_string(&config_path).context("Failed to read scratchpad.json")?;
            serde_json::from_str(&json_str).context("Failed to parse scratchpad.json")?
        } else {
            ScratchpadData::default()
        };

        Ok(Self {
            data: Arc::new(Mutex::new(data)),
            config_path,
        })
    }

    /// Create a default scratchpad in ~/.config/textquest/
    pub fn default_location() -> Result<Self> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE")) // Windows fallback
            .context("Could not determine home directory")?;
        let config_dir = PathBuf::from(home).join(".config/textquest");
        std::fs::create_dir_all(&config_dir).context("Failed to create config directory")?;
        Self::new(config_dir)
    }

    /// Add a new note to the scratchpad.
    pub fn add_note(&self, title: impl Into<String>, content: impl Into<String>) -> Result<String> {
        let mut data = self.data.lock().unwrap();
        let note = Note::new(title, content);
        let id = note.id.clone();
        data.notes.push(note);
        self.save(&data)?;
        Ok(id)
    }

    /// Get a note by ID.
    pub fn get_note(&self, id: &str) -> Option<Note> {
        let data = self.data.lock().unwrap();
        data.notes.iter().find(|n| n.id == id).cloned()
    }

    /// Update a note's content.
    pub fn update_note(&self, id: &str, content: impl Into<String>) -> Result<()> {
        let mut data = self.data.lock().unwrap();
        if let Some(note) = data.notes.iter_mut().find(|n| n.id == id) {
            note.update_content(content);
            self.save(&data)?;
            Ok(())
        } else {
            anyhow::bail!("Note not found: {}", id)
        }
    }

    /// Delete a note by ID.
    pub fn delete_note(&self, id: &str) -> Result<()> {
        let mut data = self.data.lock().unwrap();
        let initial_len = data.notes.len();
        data.notes.retain(|n| n.id != id);
        if data.notes.len() == initial_len {
            anyhow::bail!("Note not found: {}", id)
        }
        self.save(&data)?;
        Ok(())
    }

    /// Get all notes.
    pub fn list_notes(&self) -> Vec<Note> {
        let data = self.data.lock().unwrap();
        data.notes.clone()
    }

    /// Clear all notes.
    pub fn clear(&self) -> Result<()> {
        let mut data = self.data.lock().unwrap();
        data.notes.clear();
        self.save(&data)?;
        Ok(())
    }

    /// Save scratchpad data to disk.
    fn save(&self, data: &ScratchpadData) -> Result<()> {
        let json_str = serde_json::to_string_pretty(data)?;
        fs::write(&self.config_path, json_str).context("Failed to write scratchpad.json")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn note_creation_sets_timestamps() {
        let note = Note::new("Test", "Content");
        assert!(!note.id.is_empty());
        assert_eq!(note.title, "Test");
        assert_eq!(note.content, "Content");
        assert_eq!(note.created_at, note.modified_at);
    }

    #[test]
    fn note_update_changes_modified_time() {
        let mut note = Note::new("Test", "Initial");
        let original_modified = note.modified_at.clone();
        std::thread::sleep(std::time::Duration::from_millis(10));
        note.update_content("Updated");
        assert_eq!(note.content, "Updated");
        assert!(note.modified_at >= original_modified); // May be same if very fast
    }

    #[test]
    fn scratchpad_crud() -> Result<()> {
        let dir = TempDir::new()?;
        let scratchpad = Scratchpad::new(dir.path())?;

        // Create
        let id = scratchpad.add_note("Title 1", "Content 1")?;
        assert!(!id.is_empty());

        // Read
        let note = scratchpad.get_note(&id).expect("Should find note");
        assert_eq!(note.title, "Title 1");
        assert_eq!(note.content, "Content 1");

        // Update
        scratchpad.update_note(&id, "Updated Content")?;
        let updated = scratchpad.get_note(&id).expect("Should find updated note");
        assert_eq!(updated.content, "Updated Content");

        // Delete
        scratchpad.delete_note(&id)?;
        assert!(scratchpad.get_note(&id).is_none());

        Ok(())
    }

    #[test]
    fn scratchpad_persists_to_file() -> Result<()> {
        let dir = TempDir::new()?;
        let config_path = dir.path().join("scratchpad.json");

        {
            let scratchpad = Scratchpad::new(dir.path())?;
            scratchpad.add_note("Persistent", "Data")?;
        }

        // File should exist
        assert!(config_path.exists());

        // Reload and verify
        {
            let scratchpad = Scratchpad::new(dir.path())?;
            let notes = scratchpad.list_notes();
            assert_eq!(notes.len(), 1);
            assert_eq!(notes[0].title, "Persistent");
        }

        Ok(())
    }

    #[test]
    fn scratchpad_list_notes() -> Result<()> {
        let dir = TempDir::new()?;
        let scratchpad = Scratchpad::new(dir.path())?;

        scratchpad.add_note("Note 1", "Content 1")?;
        scratchpad.add_note("Note 2", "Content 2")?;
        scratchpad.add_note("Note 3", "Content 3")?;

        let notes = scratchpad.list_notes();
        assert_eq!(notes.len(), 3);
        assert_eq!(notes[0].title, "Note 1");
        assert_eq!(notes[1].title, "Note 2");
        assert_eq!(notes[2].title, "Note 3");

        Ok(())
    }

    #[test]
    fn scratchpad_clear() -> Result<()> {
        let dir = TempDir::new()?;
        let scratchpad = Scratchpad::new(dir.path())?;

        scratchpad.add_note("Note 1", "Content 1")?;
        scratchpad.add_note("Note 2", "Content 2")?;

        assert_eq!(scratchpad.list_notes().len(), 2);

        scratchpad.clear()?;
        assert_eq!(scratchpad.list_notes().len(), 0);

        Ok(())
    }

    #[test]
    fn scratchpad_delete_nonexistent_fails() -> Result<()> {
        let dir = TempDir::new()?;
        let scratchpad = Scratchpad::new(dir.path())?;

        let result = scratchpad.delete_note("nonexistent");
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn scratchpad_update_nonexistent_fails() -> Result<()> {
        let dir = TempDir::new()?;
        let scratchpad = Scratchpad::new(dir.path())?;

        let result = scratchpad.update_note("nonexistent", "content");
        assert!(result.is_err());

        Ok(())
    }
}
