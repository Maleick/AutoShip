use std::path::Path;

use anyhow::{Context, Result};
use dmft_common::soul::{MoodState, SoulEvent, SpeechStyle};
use dmft_common::types::ClientId;
use rusqlite::{Connection, params};

/// Autobiographical memory store backed by SQLite.
/// One database per deployment, partitioned by character_id.
pub struct MemoryStore {
    conn: Connection,
}

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS memories (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    character_id INTEGER NOT NULL,
    event_type   TEXT NOT NULL,
    event_json   TEXT NOT NULL,
    zone         TEXT,
    mood_at_time TEXT NOT NULL,
    importance   REAL NOT NULL DEFAULT 1.0,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    decayed      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_memories_character
    ON memories(character_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_memories_zone
    ON memories(character_id, zone);

CREATE TABLE IF NOT EXISTS conversations (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    character_id INTEGER NOT NULL,
    speaker      TEXT NOT NULL,
    is_player    INTEGER NOT NULL DEFAULT 0,
    channel      TEXT NOT NULL,
    message      TEXT NOT NULL,
    sentiment    REAL,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_conversations_character
    ON conversations(character_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_conversations_speaker
    ON conversations(character_id, speaker);

CREATE TABLE IF NOT EXISTS memory_summaries (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    character_id INTEGER NOT NULL,
    period_start TEXT NOT NULL,
    period_end   TEXT NOT NULL,
    summary      TEXT NOT NULL,
    mood_trend   TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_summaries_character
    ON memory_summaries(character_id, period_start DESC);

CREATE TABLE IF NOT EXISTS shared_references (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    character_a  INTEGER NOT NULL,
    character_b  INTEGER NOT NULL,
    memory_id    INTEGER NOT NULL REFERENCES memories(id),
    description  TEXT NOT NULL,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_shared_refs
    ON shared_references(character_a, character_b);

CREATE TABLE IF NOT EXISTS speech_patterns (
    character_id     INTEGER PRIMARY KEY,
    vocabulary_level REAL NOT NULL DEFAULT 0.5,
    emote_frequency  REAL NOT NULL DEFAULT 0.5,
    typing_speed     REAL NOT NULL DEFAULT 1.0,
    catchphrases     TEXT NOT NULL DEFAULT '[]',
    adopted_slang    TEXT NOT NULL DEFAULT '[]',
    updated_at       TEXT NOT NULL DEFAULT (datetime('now'))
);
";

impl MemoryStore {
    /// Open (or create) the memory database at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open memory store at {}", path.display()))?;

        conn.execute_batch(SCHEMA)
            .context("Failed to initialize memory store schema")?;

        Ok(Self { conn })
    }

    /// Record a soul event as a memory for a character.
    pub fn record(
        &self,
        character_id: ClientId,
        event: &SoulEvent,
        mood: MoodState,
        importance: f32,
    ) -> Result<i64> {
        let event_type = event_type_label(event);
        let event_json =
            serde_json::to_string(event).context("Failed to serialize SoulEvent")?;
        let zone = event_zone(event);
        let mood_str = format!("{:?}", mood);

        self.conn.execute(
            "INSERT INTO memories (character_id, event_type, event_json, zone, mood_at_time, importance)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![character_id, event_type, event_json, zone, mood_str, importance],
        ).context("Failed to record memory")?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Recall the N most recent memories for a character.
    pub fn recall_recent(&self, character_id: ClientId, limit: usize) -> Result<Vec<MemoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, event_type, event_json, zone, mood_at_time, importance, created_at, decayed
             FROM memories
             WHERE character_id = ?1 AND decayed = 0
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![character_id, limit as i64], MemoryRow::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to read memories")?;

        Ok(rows)
    }

    /// Recall memories about a specific subject (zone, player name, etc.).
    /// Applies rehearsal effect: each recalled memory gets +0.1 importance boost,
    /// simulating how remembering something reinforces the memory.
    pub fn recall_about(
        &self,
        character_id: ClientId,
        subject: &str,
        limit: usize,
    ) -> Result<Vec<MemoryRow>> {
        let escaped = subject.replace('%', "\\%").replace('_', "\\_");
        let pattern = format!("%{}%", escaped);
        let mut stmt = self.conn.prepare(
            "SELECT id, event_type, event_json, zone, mood_at_time, importance, created_at, decayed
             FROM memories
             WHERE character_id = ?1 AND decayed = 0
               AND (event_json LIKE ?2 ESCAPE '\\' OR zone LIKE ?2 ESCAPE '\\')
             ORDER BY importance DESC, created_at DESC
             LIMIT ?3",
        )?;

        let rows = stmt
            .query_map(params![character_id, pattern, limit as i64], MemoryRow::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to search memories")?;

        // Rehearsal effect: recalling memories reinforces them
        for row in &rows {
            let _ = self.rehearse(row.id, 0.1);
        }

        Ok(rows)
    }

    /// Record a conversation line.
    pub fn record_conversation(
        &self,
        character_id: ClientId,
        speaker: &str,
        is_player: bool,
        channel: &str,
        message: &str,
        sentiment: Option<f32>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO conversations (character_id, speaker, is_player, channel, message, sentiment)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![character_id, speaker, is_player as i32, channel, message, sentiment],
        ).context("Failed to record conversation")?;

        Ok(())
    }

    /// Recall recent conversations for a character.
    pub fn recall_conversations(
        &self,
        character_id: ClientId,
        limit: usize,
    ) -> Result<Vec<ConversationRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, speaker, is_player, channel, message, sentiment, created_at
             FROM conversations
             WHERE character_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![character_id, limit as i64], ConversationRow::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to read conversations")?;

        Ok(rows)
    }

    /// Get the speech style for a character.
    pub fn get_speech_patterns(&self, character_id: ClientId) -> Result<SpeechStyle> {
        let result = self.conn.query_row(
            "SELECT vocabulary_level, emote_frequency, typing_speed, catchphrases, adopted_slang
             FROM speech_patterns WHERE character_id = ?1",
            params![character_id],
            |row| {
                let catchphrases_json: String = row.get(3)?;
                let adopted_slang_json: String = row.get(4)?;
                Ok(SpeechStyle {
                    vocabulary_level: row.get(0)?,
                    emote_frequency: row.get(1)?,
                    typing_speed: row.get(2)?,
                    catchphrases: serde_json::from_str(&catchphrases_json).unwrap_or_default(),
                    adopted_slang: serde_json::from_str(&adopted_slang_json).unwrap_or_default(),
                })
            },
        );

        match result {
            Ok(style) => Ok(style),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(SpeechStyle::default()),
            Err(e) => Err(e).context("Failed to get speech patterns"),
        }
    }

    /// Update the speech style for a character.
    pub fn update_speech_patterns(
        &self,
        character_id: ClientId,
        style: &SpeechStyle,
    ) -> Result<()> {
        let catchphrases_json = serde_json::to_string(&style.catchphrases)
            .context("Failed to serialize catchphrases")?;
        let adopted_slang_json = serde_json::to_string(&style.adopted_slang)
            .context("Failed to serialize adopted_slang")?;

        self.conn.execute(
            "INSERT INTO speech_patterns (character_id, vocabulary_level, emote_frequency, typing_speed, catchphrases, adopted_slang, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
             ON CONFLICT(character_id) DO UPDATE SET
                vocabulary_level = excluded.vocabulary_level,
                emote_frequency = excluded.emote_frequency,
                typing_speed = excluded.typing_speed,
                catchphrases = excluded.catchphrases,
                adopted_slang = excluded.adopted_slang,
                updated_at = datetime('now')",
            params![
                character_id,
                style.vocabulary_level,
                style.emote_frequency,
                style.typing_speed,
                catchphrases_json,
                adopted_slang_json,
            ],
        ).context("Failed to update speech patterns")?;

        Ok(())
    }

    /// Record a memory summary for a time period.
    pub fn record_summary(
        &self,
        character_id: ClientId,
        period_start: &str,
        period_end: &str,
        summary: &str,
        mood_trend: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO memory_summaries (character_id, period_start, period_end, summary, mood_trend)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![character_id, period_start, period_end, summary, mood_trend],
        ).context("Failed to record summary")?;

        Ok(())
    }

    /// Record a shared reference between two characters for a memory.
    pub fn record_shared_reference(
        &self,
        character_a: ClientId,
        character_b: ClientId,
        memory_id: i64,
        description: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO shared_references (character_a, character_b, memory_id, description)
             VALUES (?1, ?2, ?3, ?4)",
            params![character_a, character_b, memory_id, description],
        ).context("Failed to record shared reference")?;

        Ok(())
    }

    /// Get underlying connection for use by extensions.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    // -- Decay & pruning (Task 8) --

    /// Apply exponential decay to all non-decayed memories for a character.
    /// `decay_factor` is multiplied into importance each tick.
    /// Typical half-life: if tick is every 30 min, decay_factor ≈ 0.99 gives
    /// half-life of ~69 ticks (~34.5 hours).
    pub fn decay_tick(&self, character_id: ClientId, decay_factor: f32) -> Result<usize> {
        let rows = self.conn.execute(
            "UPDATE memories
             SET importance = importance * ?1
             WHERE character_id = ?2 AND decayed = 0",
            params![decay_factor, character_id],
        ).context("Failed to decay memories")?;

        Ok(rows)
    }

    /// Mark memories with importance below threshold as decayed (soft delete).
    /// Returns the number of memories pruned.
    pub fn prune_low_importance(
        &self,
        character_id: ClientId,
        threshold: f32,
    ) -> Result<usize> {
        let rows = self.conn.execute(
            "UPDATE memories
             SET decayed = 1
             WHERE character_id = ?1 AND decayed = 0 AND importance < ?2",
            params![character_id, threshold],
        ).context("Failed to prune low-importance memories")?;

        Ok(rows)
    }

    /// Rehearsal effect: boost importance of recalled memories.
    /// Called when recall_about() finds matching memories — each recall
    /// reinforces the memory, making it resist decay longer.
    pub fn rehearse(&self, memory_id: i64, boost: f32) -> Result<()> {
        self.conn.execute(
            "UPDATE memories
             SET importance = MIN(importance + ?1, 10.0)
             WHERE id = ?2 AND decayed = 0",
            params![boost, memory_id],
        ).context("Failed to rehearse memory")?;

        Ok(())
    }

    /// Export all memories and conversations for a character as JSON.
    /// Used for per-character portability and LLM context building.
    pub fn export_character_json(&self, character_id: ClientId) -> Result<String> {
        let memories = self.recall_recent(character_id, 10000)?;
        let conversations = self.recall_conversations(character_id, 10000)?;
        let speech = self.get_speech_patterns(character_id)?;

        let export = serde_json::json!({
            "character_id": character_id,
            "memories": memories.iter().map(|m| {
                serde_json::json!({
                    "id": m.id,
                    "event_type": m.event_type,
                    "event": serde_json::from_str::<serde_json::Value>(&m.event_json).unwrap_or_default(),
                    "zone": m.zone,
                    "mood": m.mood_at_time,
                    "importance": m.importance,
                    "created_at": m.created_at,
                })
            }).collect::<Vec<_>>(),
            "conversations": conversations.iter().map(|c| {
                serde_json::json!({
                    "speaker": c.speaker,
                    "is_player": c.is_player,
                    "channel": c.channel,
                    "message": c.message,
                    "sentiment": c.sentiment,
                    "created_at": c.created_at,
                })
            }).collect::<Vec<_>>(),
            "speech_style": serde_json::json!({
                "vocabulary_level": speech.vocabulary_level,
                "emote_frequency": speech.emote_frequency,
                "typing_speed": speech.typing_speed,
                "catchphrases": speech.catchphrases,
                "adopted_slang": speech.adopted_slang,
            }),
        });

        serde_json::to_string_pretty(&export).context("Failed to serialize character export")
    }
}

/// A row from the memories table.
#[derive(Debug, Clone)]
pub struct MemoryRow {
    pub id: i64,
    pub event_type: String,
    pub event_json: String,
    pub zone: Option<String>,
    pub mood_at_time: String,
    pub importance: f32,
    pub created_at: String,
    pub decayed: bool,
}

impl MemoryRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            event_type: row.get(1)?,
            event_json: row.get(2)?,
            zone: row.get(3)?,
            mood_at_time: row.get(4)?,
            importance: row.get(5)?,
            created_at: row.get(6)?,
            decayed: row.get::<_, i32>(7)? != 0,
        })
    }

    /// Deserialize the stored event.
    pub fn event(&self) -> Result<SoulEvent> {
        serde_json::from_str(&self.event_json).context("Failed to deserialize SoulEvent")
    }
}

/// A row from the conversations table.
#[derive(Debug, Clone)]
pub struct ConversationRow {
    pub id: i64,
    pub speaker: String,
    pub is_player: bool,
    pub channel: String,
    pub message: String,
    pub sentiment: Option<f32>,
    pub created_at: String,
}

impl ConversationRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            speaker: row.get(1)?,
            is_player: row.get::<_, i32>(2)? != 0,
            channel: row.get(3)?,
            message: row.get(4)?,
            sentiment: row.get(5)?,
            created_at: row.get(6)?,
        })
    }
}

/// Extract a short label for the event type (used as event_type column).
fn event_type_label(event: &SoulEvent) -> &'static str {
    match event {
        SoulEvent::Death { .. } => "death",
        SoulEvent::Kill { .. } => "kill",
        SoulEvent::Loot { .. } => "loot",
        SoulEvent::PlayerChat { .. } => "player_chat",
        SoulEvent::BotChat { .. } => "bot_chat",
        SoulEvent::Witnessed { .. } => "witnessed",
        SoulEvent::MoodShift { .. } => "mood_shift",
        SoulEvent::ZoneEnter { .. } => "zone_enter",
        SoulEvent::LevelUp { .. } => "level_up",
        SoulEvent::GroupWipe { .. } => "group_wipe",
        SoulEvent::RelationshipChange { .. } => "relationship_change",
    }
}

/// Extract zone from events that have one.
fn event_zone(event: &SoulEvent) -> Option<String> {
    match event {
        SoulEvent::Death { zone, .. }
        | SoulEvent::Kill { zone, .. }
        | SoulEvent::Loot { zone, .. }
        | SoulEvent::ZoneEnter { zone }
        | SoulEvent::GroupWipe { zone } => Some(zone.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dmft_common::soul::{MoodState, SoulEvent};
    use rusqlite::Connection;

    fn open_memory_store() -> MemoryStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        MemoryStore { conn }
    }

    fn kill_event(target: &str, zone: &str) -> SoulEvent {
        SoulEvent::Kill {
            target: target.into(),
            zone: zone.into(),
        }
    }

    fn loot_event(item: &str, zone: &str) -> SoulEvent {
        SoulEvent::Loot {
            item: item.into(),
            zone: zone.into(),
        }
    }

    #[test]
    fn record_returns_positive_id() {
        let store = open_memory_store();
        let event = kill_event("a gnoll", "blackburrow");
        let id = store.record(1, &event, MoodState::Excited, 1.0).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn record_multiple_returns_sequential_ids() {
        let store = open_memory_store();
        let id1 = store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 1.0)
            .unwrap();
        let id2 = store
            .record(1, &kill_event("bear", "everfrost"), MoodState::Happy, 1.0)
            .unwrap();
        assert!(id2 > id1);
    }

    #[test]
    fn recall_recent_returns_memories_in_order() {
        let store = open_memory_store();
        let id1 = store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 1.0)
            .unwrap();
        let id2 = store
            .record(1, &kill_event("bear", "everfrost"), MoodState::Excited, 2.0)
            .unwrap();
        let id3 = store
            .record(1, &kill_event("orc", "crushbone"), MoodState::Angry, 3.0)
            .unwrap();

        let memories = store.recall_recent(1, 10).unwrap();
        assert_eq!(memories.len(), 3);
        // All three records should be present
        let ids: Vec<i64> = memories.iter().map(|m| m.id).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
        assert!(ids.contains(&id3));
    }

    #[test]
    fn recall_recent_respects_limit() {
        let store = open_memory_store();
        for i in 0..10 {
            store
                .record(
                    1,
                    &kill_event(&format!("mob_{}", i), "zone"),
                    MoodState::Neutral,
                    1.0,
                )
                .unwrap();
        }
        let memories = store.recall_recent(1, 3).unwrap();
        assert_eq!(memories.len(), 3);
    }

    #[test]
    fn recall_recent_filters_by_character() {
        let store = open_memory_store();
        store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 1.0)
            .unwrap();
        store
            .record(2, &kill_event("bear", "everfrost"), MoodState::Happy, 1.0)
            .unwrap();

        let char1_memories = store.recall_recent(1, 10).unwrap();
        let char2_memories = store.recall_recent(2, 10).unwrap();
        assert_eq!(char1_memories.len(), 1);
        assert_eq!(char2_memories.len(), 1);
        assert_eq!(char1_memories[0].event_type, "kill");
    }

    #[test]
    fn recall_about_filters_by_subject() {
        let store = open_memory_store();
        store
            .record(1, &kill_event("gnoll", "blackburrow"), MoodState::Neutral, 1.0)
            .unwrap();
        store
            .record(1, &kill_event("orc", "crushbone"), MoodState::Angry, 1.0)
            .unwrap();
        store
            .record(1, &loot_event("sword", "blackburrow"), MoodState::Happy, 1.0)
            .unwrap();

        // Search by zone name
        let results = store.recall_about(1, "blackburrow", 10).unwrap();
        assert_eq!(results.len(), 2);

        // Search by target name
        let results = store.recall_about(1, "orc", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn recall_about_triggers_rehearsal() {
        let store = open_memory_store();
        let id = store
            .record(1, &kill_event("gnoll", "blackburrow"), MoodState::Neutral, 1.0)
            .unwrap();

        // Recall about "gnoll" should boost importance by 0.1
        let _ = store.recall_about(1, "gnoll", 10).unwrap();

        // Read back and check importance increased
        let memories = store.recall_recent(1, 10).unwrap();
        let memory = memories.iter().find(|m| m.id == id).unwrap();
        assert!(
            memory.importance > 1.0,
            "Expected importance > 1.0 after rehearsal, got {}",
            memory.importance
        );
    }

    #[test]
    fn rehearse_increments_importance() {
        let store = open_memory_store();
        let id = store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 2.0)
            .unwrap();

        store.rehearse(id, 0.5).unwrap();

        let memories = store.recall_recent(1, 10).unwrap();
        let memory = memories.iter().find(|m| m.id == id).unwrap();
        assert!(
            (memory.importance - 2.5).abs() < 0.01,
            "Expected importance ~2.5, got {}",
            memory.importance
        );
    }

    #[test]
    fn rehearse_caps_at_ten() {
        let store = open_memory_store();
        let id = store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 9.8)
            .unwrap();

        store.rehearse(id, 1.0).unwrap();

        let memories = store.recall_recent(1, 10).unwrap();
        let memory = memories.iter().find(|m| m.id == id).unwrap();
        assert!(
            (memory.importance - 10.0).abs() < 0.01,
            "Expected importance capped at 10.0, got {}",
            memory.importance
        );
    }

    #[test]
    fn decay_tick_reduces_importance() {
        let store = open_memory_store();
        store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 5.0)
            .unwrap();

        let rows = store.decay_tick(1, 0.5).unwrap();
        assert_eq!(rows, 1);

        let memories = store.recall_recent(1, 10).unwrap();
        assert!(
            (memories[0].importance - 2.5).abs() < 0.01,
            "Expected importance ~2.5 after 0.5 decay, got {}",
            memories[0].importance
        );
    }

    #[test]
    fn prune_low_importance_marks_decayed() {
        let store = open_memory_store();
        store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 0.05)
            .unwrap();
        store
            .record(1, &kill_event("bear", "everfrost"), MoodState::Happy, 5.0)
            .unwrap();

        let pruned = store.prune_low_importance(1, 0.1).unwrap();
        assert_eq!(pruned, 1);

        // recall_recent skips decayed memories
        let memories = store.recall_recent(1, 10).unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].event_type, "kill");
    }

    #[test]
    fn record_and_recall_conversations() {
        let store = open_memory_store();
        store
            .record_conversation(1, "Dave", true, "say", "Hey there!", Some(0.8))
            .unwrap();
        store
            .record_conversation(1, "TestBot", false, "group", "On my way.", None)
            .unwrap();

        let convos = store.recall_conversations(1, 10).unwrap();
        assert_eq!(convos.len(), 2);
        assert!(convos[0].is_player || convos[1].is_player);
    }

    #[test]
    fn speech_patterns_default_when_absent() {
        let store = open_memory_store();
        let style = store.get_speech_patterns(99).unwrap();
        assert!((style.vocabulary_level - 0.5).abs() < 0.01);
        assert!(style.catchphrases.is_empty());
    }

    #[test]
    fn update_and_get_speech_patterns() {
        let store = open_memory_store();
        let style = SpeechStyle {
            vocabulary_level: 0.8,
            emote_frequency: 0.3,
            typing_speed: 1.5,
            catchphrases: vec!["Hail!".into()],
            adopted_slang: vec!["kek".into()],
        };
        store.update_speech_patterns(1, &style).unwrap();

        let loaded = store.get_speech_patterns(1).unwrap();
        assert!((loaded.vocabulary_level - 0.8).abs() < 0.01);
        assert_eq!(loaded.catchphrases, vec!["Hail!"]);
        assert_eq!(loaded.adopted_slang, vec!["kek"]);
    }

    #[test]
    fn memory_row_event_deserializes() {
        let store = open_memory_store();
        let original = kill_event("a_gnoll", "blackburrow");
        store
            .record(1, &original, MoodState::Neutral, 1.0)
            .unwrap();

        let memories = store.recall_recent(1, 1).unwrap();
        let deserialized = memories[0].event().unwrap();
        match deserialized {
            SoulEvent::Kill { target, zone } => {
                assert_eq!(target, "a_gnoll");
                assert_eq!(zone, "blackburrow");
            }
            _ => panic!("Expected SoulEvent::Kill"),
        }
    }

    #[test]
    fn event_type_labels_are_correct() {
        assert_eq!(
            event_type_label(&SoulEvent::Death {
                zone: "".into(),
                killer: None
            }),
            "death"
        );
        assert_eq!(
            event_type_label(&SoulEvent::LevelUp { new_level: 1 }),
            "level_up"
        );
        assert_eq!(
            event_type_label(&SoulEvent::GroupWipe {
                zone: "".into()
            }),
            "group_wipe"
        );
    }

    #[test]
    fn event_zone_extracts_zone_from_applicable_events() {
        let death = SoulEvent::Death {
            zone: "guk".into(),
            killer: None,
        };
        assert_eq!(event_zone(&death), Some("guk".into()));

        let chat = SoulEvent::PlayerChat {
            player_name: "Dave".into(),
            sentiment: 0.5,
        };
        assert_eq!(event_zone(&chat), None);
    }
}
