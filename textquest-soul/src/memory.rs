use std::{
    cell::RefCell,
    collections::VecDeque,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result};
use chrono::{NaiveDateTime, Utc};
use rusqlite::{Connection, params};
use textquest_common::{
    soul::{MoodState, SoulEvent, SpeechStyle},
    types::ClientId,
};

/// Exponential backoff delays for database retry logic (milliseconds).
const RETRY_DELAYS_MS: [u64; 5] = [100, 500, 1_000, 5_000, 30_000];

/// Maximum consecutive failures before the circuit breaker opens.
const CIRCUIT_BREAKER_THRESHOLD: u32 = 5;

/// Maximum number of entries held in the in-memory fallback cache.
const FALLBACK_CACHE_MAX: usize = 256;
/// Maximum number of memory rows summarized in one hourly pass.
const SUMMARY_MEMORY_ROWS_MAX: usize = 100;

/// Health state shared across retried operations.
#[derive(Debug, Default)]
struct DbHealth {
    /// Count of consecutive write failures.
    consecutive_failures: AtomicU32,
    /// Circuit breaker: true means DB writes are disabled.
    circuit_open: AtomicBool,
}

impl DbHealth {
    fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        if self.circuit_open.swap(false, Ordering::Relaxed) {
            tracing::info!("memory_store: circuit breaker closed — DB writes re-enabled");
        }
    }

    fn record_failure(&self, op: &str, err: &anyhow::Error) {
        let prev = self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
        let count = prev + 1;
        tracing::warn!(op, consecutive_failures = count, error = %err, "memory_store: DB write failed");
        if count >= CIRCUIT_BREAKER_THRESHOLD && !self.circuit_open.load(Ordering::Relaxed) {
            self.circuit_open.store(true, Ordering::Relaxed);
            tracing::error!(
                "memory_store: circuit breaker OPEN after {} consecutive failures — DB writes \
                 disabled",
                count
            );
        }
    }

    fn is_open(&self) -> bool {
        self.circuit_open.load(Ordering::Relaxed)
    }
}

/// A cached record held in the fallback in-memory store.
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct CachedMemory {
    character_id: ClientId,
    event_type: String,
    event_json: String,
    zone: Option<String>,
    mood_at_time: String,
    importance: f32,
}

/// Autobiographical memory store backed by `SQLite`.
/// One database per deployment, partitioned by `character_id`.
///
/// Includes:
/// - Exponential backoff retry (up to 5 attempts) on transient DB errors
/// - Circuit breaker that disables writes after 5 consecutive failures
/// - In-memory fallback cache (up to 256 entries) when the circuit is open
pub struct MemoryStore {
    conn: Connection,
    health: Arc<DbHealth>,
    /// In-memory fallback cache used when the circuit breaker is open.
    /// Uses `RefCell` for interior mutability so write methods retain `&self`.
    fallback_cache: RefCell<VecDeque<CachedMemory>>,
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

CREATE TABLE IF NOT EXISTS soul_audit_log (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    character_id INTEGER NOT NULL,
    action_type  TEXT NOT NULL,
    action_json  TEXT NOT NULL,
    reason       TEXT,
    operator_id  TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_audit_character
    ON soul_audit_log(character_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_action_type
    ON soul_audit_log(action_type, created_at DESC);

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

/// Retry a fallible DB operation with exponential backoff.
///
/// Attempts the operation up to `RETRY_DELAYS_MS.len() + 1` times (6 total).
/// Sleeps between attempts using the delays in `RETRY_DELAYS_MS`.
/// Returns the last error if all attempts fail.
fn retry_db_op<T, F>(op_name: &'static str, mut f: F) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    let mut last_err = None;
    for (attempt, &delay_ms) in std::iter::once(&0u64)
        .chain(RETRY_DELAYS_MS.iter())
        .enumerate()
    {
        if delay_ms > 0 {
            tracing::debug!(
                op = op_name,
                attempt,
                delay_ms,
                "memory_store: retrying DB op"
            );
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                tracing::warn!(op = op_name, attempt, error = %e, "memory_store: DB op failed");
                last_err = Some(e);
            }
        }
    }
    Err(last_err.expect("retry loop must set last_err"))
}

impl MemoryStore {
    /// Open (or create) the memory database at the given path.
    ///
    /// On transient failures, retries with exponential backoff (up to 5
    /// attempts).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails after all retries.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = retry_db_op("open", || {
            Connection::open(path)
                .with_context(|| format!("Failed to open memory store at {}", path.display()))
        })?;

        retry_db_op("schema_init", || {
            conn.execute_batch(SCHEMA)
                .context("Failed to initialize memory store schema")
        })?;

        Ok(Self {
            conn,
            health: Arc::new(DbHealth::default()),
            fallback_cache: RefCell::new(VecDeque::new()),
        })
    }

    /// Open an in-memory database — available in `#[cfg(test)]` only so that
    /// tests outside this module (e.g. `soul::perf_tests`) can construct a
    /// `MemoryStore` without touching the filesystem.
    ///
    /// # Panics
    ///
    /// Panics if the in-memory connection cannot be opened or the schema fails
    /// to apply — both are programmer errors in a test context.
    #[cfg(test)]
    pub fn open_in_memory() -> Self {
        let conn = Connection::open_in_memory().expect("Failed to open in-memory SQLite");
        conn.execute_batch(SCHEMA)
            .expect("Failed to apply schema to in-memory store");
        Self {
            conn,
            health: Arc::new(DbHealth::default()),
            fallback_cache: RefCell::new(VecDeque::new()),
        }
    }

    /// Record a soul event as a memory for a character.
    ///
    /// Uses exponential backoff retry on transient failures. If the circuit
    /// breaker is open, the memory is written to an in-memory fallback cache
    /// instead of the database so no data is permanently lost for recent
    /// events.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails. DB errors are handled
    /// internally (logged + fallback cache) and do not propagate unless the
    /// caller needs the inserted row ID for further operations.
    pub fn record(
        &self,
        character_id: ClientId,
        event: &SoulEvent,
        mood: MoodState,
        importance: f32,
    ) -> Result<i64> {
        let event_type = event_type_label(event);
        let event_json = serde_json::to_string(event).context("Failed to serialize SoulEvent")?;
        let zone = event_zone(event);
        let mood_str = format!("{mood:?}");

        // If the circuit is open, write to fallback cache and return a synthetic ID.
        if self.health.is_open() {
            tracing::warn!(
                character_id,
                event_type,
                "memory_store: circuit open — buffering memory in fallback cache"
            );
            self.push_fallback(CachedMemory {
                character_id,
                event_type: event_type.to_string(),
                event_json,
                zone,
                mood_at_time: mood_str,
                importance,
            });
            return Ok(-1);
        }

        let result = retry_db_op("record_memory", || {
            self.conn
                .execute(
                    "INSERT INTO memories (character_id, event_type, event_json, zone, \
                     mood_at_time, importance)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        character_id,
                        event_type,
                        &event_json,
                        &zone,
                        &mood_str,
                        importance
                    ],
                )
                .context("Failed to record memory")
        });

        match result {
            Ok(_) => {
                self.health.record_success();
                Ok(self.conn.last_insert_rowid())
            }
            Err(e) => {
                self.health.record_failure("record_memory", &e);
                tracing::warn!(
                    character_id,
                    event_type,
                    error = %e,
                    "memory_store: falling back to in-memory cache after retry exhaustion"
                );
                self.push_fallback(CachedMemory {
                    character_id,
                    event_type: event_type.to_string(),
                    event_json,
                    zone,
                    mood_at_time: mood_str,
                    importance,
                });
                Ok(-1)
            }
        }
    }

    /// Push a memory entry into the in-memory fallback cache, evicting the
    /// oldest entry when the cache is full.
    fn push_fallback(&self, entry: CachedMemory) {
        let mut cache = self.fallback_cache.borrow_mut();
        if cache.len() >= FALLBACK_CACHE_MAX {
            cache.pop_front();
        }
        cache.push_back(entry);
    }

    /// Returns the number of memories currently held in the fallback cache.
    pub fn fallback_cache_len(&self) -> usize {
        self.fallback_cache.borrow().len()
    }

    /// Returns true if the circuit breaker is open (DB writes disabled).
    pub fn is_circuit_open(&self) -> bool {
        self.health.is_open()
    }

    /// Recall the N most recent memories for a character.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn recall_recent(&self, character_id: ClientId, limit: usize) -> Result<Vec<MemoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, event_type, event_json, zone, mood_at_time, importance, created_at, \
             decayed
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
    /// Applies rehearsal effect: each recalled memory gets +0.1 importance
    /// boost, simulating how remembering something reinforces the memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn recall_about(
        &self,
        character_id: ClientId,
        subject: &str,
        limit: usize,
    ) -> Result<Vec<MemoryRow>> {
        let escaped = subject.replace('%', "\\%").replace('_', "\\_");
        let pattern = format!("%{escaped}%");
        let mut stmt = self.conn.prepare(
            "SELECT id, event_type, event_json, zone, mood_at_time, importance, created_at, \
             decayed
             FROM memories
             WHERE character_id = ?1 AND decayed = 0
               AND (event_json LIKE ?2 ESCAPE '\\' OR zone LIKE ?2 ESCAPE '\\')
             ORDER BY importance DESC, created_at DESC
             LIMIT ?3",
        )?;

        let rows = stmt
            .query_map(
                params![character_id, pattern, limit as i64],
                MemoryRow::from_row,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to search memories")?;

        // Rehearsal effect: recalling memories reinforces them
        for row in &rows {
            let _ = self.rehearse(row.id, 0.1);
        }

        Ok(rows)
    }

    /// Record a conversation line.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn record_conversation(
        &self,
        character_id: ClientId,
        speaker: &str,
        is_player: bool,
        channel: &str,
        message: &str,
        sentiment: f32,
    ) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO conversations (character_id, speaker, is_player, channel, message, \
                 sentiment)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    character_id,
                    speaker,
                    i32::from(is_player),
                    channel,
                    message,
                    sentiment
                ],
            )
            .context("Failed to record conversation")?;

        Ok(())
    }

    /// Prune old conversation lines, keeping only the most recent `max_rows`
    /// for a character.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn prune_conversations(&self, character_id: ClientId, max_rows: usize) -> Result<usize> {
        let deleted = self
            .conn
            .execute(
                "DELETE FROM conversations
                 WHERE character_id = ?1
                   AND id IN (
                       SELECT id
                       FROM conversations
                       WHERE character_id = ?1
                       ORDER BY created_at DESC, id DESC
                       LIMIT -1 OFFSET ?2
                   )",
                params![character_id, max_rows as i64],
            )
            .context("Failed to prune conversations")?;

        Ok(deleted)
    }

    /// Recall recent conversations for a character.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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
            .query_map(
                params![character_id, limit as i64],
                ConversationRow::from_row,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to read conversations")?;

        Ok(rows)
    }

    /// Summarize notable positive and negative player chat counts for one
    /// speaker.
    ///
    /// Counts only messages whose stored sentiment crosses the same strong
    /// thresholds used for relationship changes.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn summarize_player_chat_sentiment(
        &self,
        character_id: ClientId,
        speaker: &str,
    ) -> Result<Option<String>> {
        let (positive_count, negative_count): (i64, i64) = self
            .conn
            .query_row(
                "SELECT
                    SUM(CASE WHEN sentiment > 0.5 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN sentiment < -0.5 THEN 1 ELSE 0 END)
                 FROM conversations
                 WHERE character_id = ?1 AND speaker = ?2 AND is_player = 1",
                params![character_id, speaker],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                        row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    ))
                },
            )
            .context("Failed to summarize player chat sentiment")?;

        if positive_count == 0 && negative_count == 0 {
            return Ok(None);
        }

        let positive_label = if positive_count == 1 {
            "positive chat"
        } else {
            "positive chats"
        };
        let negative_label = if negative_count == 1 {
            "negative chat"
        } else {
            "negative chats"
        };

        Ok(Some(format!(
            "had {positive_count} {positive_label}, {negative_count} {negative_label}"
        )))
    }

    /// Get the speech style for a character.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn update_speech_patterns(
        &self,
        character_id: ClientId,
        style: &SpeechStyle,
    ) -> Result<()> {
        let catchphrases_json = serde_json::to_string(&style.catchphrases)
            .context("Failed to serialize catchphrases")?;
        let adopted_slang_json = serde_json::to_string(&style.adopted_slang)
            .context("Failed to serialize adopted_slang")?;

        self.conn
            .execute(
                "INSERT INTO speech_patterns (character_id, vocabulary_level, emote_frequency, \
                 typing_speed, catchphrases, adopted_slang, updated_at)
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
            )
            .context("Failed to update speech patterns")?;

        Ok(())
    }

    /// Record a memory summary for a time period.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn record_summary(
        &self,
        character_id: ClientId,
        period_start: &str,
        period_end: &str,
        summary: &str,
        mood_trend: Option<&str>,
    ) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO memory_summaries (character_id, period_start, period_end, summary, \
                 mood_trend)
             VALUES (?1, ?2, ?3, ?4, ?5)",
                params![character_id, period_start, period_end, summary, mood_trend],
            )
            .context("Failed to record summary")?;

        Ok(())
    }

    /// Record a shared reference between two characters for a memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn record_shared_reference(
        &self,
        character_a: ClientId,
        character_b: ClientId,
        memory_id: i64,
        description: &str,
    ) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO shared_references (character_a, character_b, memory_id, description)
             VALUES (?1, ?2, ?3, ?4)",
                params![character_a, character_b, memory_id, description],
            )
            .context("Failed to record shared reference")?;

        Ok(())
    }

    /// Get underlying connection for use by extensions.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Return the top `max_memories` memories ranked by combined importance ×
    /// recency score.
    ///
    /// Recency is computed as `max(0.0, 1.0 - (days_ago / 30.0))` so memories
    /// created today score 1.0 and memories older than 30 days score 0.0.
    /// Combined score is `importance * recency`.
    ///
    /// No DB writes are performed — this is a pure read + ordering operation.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn get_llm_context(
        &self,
        character_id: ClientId,
        max_memories: usize,
    ) -> Result<Vec<MemoryRow>> {
        // Pull all non-decayed memories for the character.
        let mut stmt = self.conn.prepare(
            "SELECT id, event_type, event_json, zone, mood_at_time, importance, created_at, \
             decayed
             FROM memories
             WHERE character_id = ?1 AND decayed = 0",
        )?;

        let mut rows: Vec<MemoryRow> = stmt
            .query_map(params![character_id], MemoryRow::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to read memories for LLM context")?;

        // Score each memory and sort descending.
        rows.sort_by(|a, b| {
            let score_a = memory_combined_score(a);
            let score_b = memory_combined_score(b);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        rows.truncate(max_memories);
        Ok(rows)
    }

    // -- Decay & pruning (Task 8) --

    /// Apply exponential decay to all non-decayed memories for a character.
    /// `decay_factor` is multiplied into importance each tick.
    /// Typical half-life: if tick is every 30 min, `decay_factor` ≈ 0.99 gives
    /// half-life of ~69 ticks (~34.5 hours).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn decay_tick(&self, character_id: ClientId, decay_factor: f32) -> Result<usize> {
        let rows = self
            .conn
            .execute(
                "UPDATE memories
             SET importance = importance * ?1
             WHERE character_id = ?2 AND decayed = 0",
                params![decay_factor, character_id],
            )
            .context("Failed to decay memories")?;

        Ok(rows)
    }

    /// Mark memories with importance below threshold as decayed (soft delete).
    /// Returns the number of memories pruned.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn prune_low_importance(&self, character_id: ClientId, threshold: f32) -> Result<usize> {
        let rows = self
            .conn
            .execute(
                "UPDATE memories
             SET decayed = 1
             WHERE character_id = ?1 AND decayed = 0 AND importance < ?2",
                params![character_id, threshold],
            )
            .context("Failed to prune low-importance memories")?;

        Ok(rows)
    }

    /// Rehearsal effect: boost importance of recalled memories.
    /// Called when recall_about() finds matching memories — each recall
    /// reinforces the memory, making it resist decay longer.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn rehearse(&self, memory_id: i64, boost: f32) -> Result<()> {
        self.conn
            .execute(
                "UPDATE memories
             SET importance = MIN(importance + ?1, 10.0)
             WHERE id = ?2 AND decayed = 0",
                params![boost, memory_id],
            )
            .context("Failed to rehearse memory")?;

        Ok(())
    }

    /// Generate a compact text summary of memories in a unix-timestamp time
    /// window.
    ///
    /// Queries all non-decayed memories for `character_id` with `created_at`
    /// between `period_start` and `period_end` (inclusive, unix seconds).
    /// Formats each event as "[zone] event_type: description" (one line per
    /// event). Appends the most common mood in the window at the end as
    /// "Mood trend: X". The returned string is capped at 500 bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn generate_summary(
        &self,
        character_id: ClientId,
        period_start: i64,
        period_end: i64,
    ) -> Result<String> {
        // Query memories in the time window by converting unix timestamp parameters
        // to SQLite datetimes, so the created_at index remains usable.
        let mut stmt = self.conn.prepare(
            "SELECT event_type, event_json, zone, mood_at_time
             FROM memories
             WHERE character_id = ?1
               AND decayed = 0
               AND created_at >= datetime(?2, 'unixepoch')
               AND created_at <= datetime(?3, 'unixepoch')
             ORDER BY created_at ASC
             LIMIT ?4",
        )?;

        struct MemSummaryRow {
            event_type: String,
            event_json: String,
            zone: Option<String>,
            mood: String,
        }

        let rows = stmt
            .query_map(
                params![
                    character_id,
                    period_start,
                    period_end,
                    SUMMARY_MEMORY_ROWS_MAX as i64,
                ],
                |row| {
                    Ok(MemSummaryRow {
                        event_type: row.get(0)?,
                        event_json: row.get(1)?,
                        zone: row.get(2)?,
                        mood: row.get(3)?,
                    })
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to query memories for summary")?;

        if rows.is_empty() {
            return Ok(String::new());
        }

        // Count moods to find the trend (most common).
        let mut mood_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for row in &rows {
            *mood_counts.entry(row.mood.clone()).or_insert(0) += 1;
        }
        let mood_trend = mood_counts
            .into_iter()
            .max_by(|(mood_a, count_a), (mood_b, count_b)| {
                count_a.cmp(count_b).then_with(|| mood_a.cmp(mood_b))
            })
            .map(|(mood, _)| mood)
            .unwrap_or_else(|| "Neutral".to_string());

        // Build compact lines.
        let mut lines: Vec<String> = Vec::with_capacity(rows.len() + 1);
        for row in &rows {
            let zone_prefix = row
                .zone
                .as_deref()
                .map(|z| format!("[{z}] "))
                .unwrap_or_default();

            // Extract a short human-readable description from the event JSON.
            let description = describe_event(&row.event_type, &row.event_json);
            lines.push(format!("{zone_prefix}{}: {description}", row.event_type));
        }

        // Append mood trend line.
        lines.push(format!("Mood trend: {mood_trend}"));

        // Join and cap at 500 chars.
        let full = lines.join("\n");
        if full.len() <= 500 {
            Ok(full)
        } else {
            // Truncate at a UTF-8 boundary.
            let mut end = 500;
            while !full.is_char_boundary(end) {
                end -= 1;
            }
            Ok(full[..end].to_string())
        }
    }

    /// Export all memories and conversations for a character as JSON.
    /// Used for per-character portability and LLM context building.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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

    // ── Audit log ────────────────────────────────────────────────────────────

    /// Append an entry to the immutable audit log.
    ///
    /// # Errors
    ///
    /// Returns an error if the insert fails.
    pub fn audit_log(
        &self,
        character_id: ClientId,
        action_type: &str,
        action_json: &str,
        reason: Option<&str>,
        operator_id: Option<&str>,
    ) -> Result<i64> {
        self.conn
            .execute(
                "INSERT INTO soul_audit_log (character_id, action_type, action_json, reason, \
                 operator_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![character_id, action_type, action_json, reason, operator_id],
            )
            .context("Failed to append audit log entry")?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Retrieve audit log entries for a character within an optional date
    /// range.
    ///
    /// `start_date` and `end_date` are ISO-8601 strings (`"YYYY-MM-DD"` or
    /// `"YYYY-MM-DD HH:MM:SS"`).  Pass `None` to omit the respective bound.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_audit_log(
        &self,
        character_id: ClientId,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> Result<Vec<AuditEntry>> {
        let mut sql = String::from(
            "SELECT id, character_id, action_type, action_json, reason, operator_id, created_at \
             FROM soul_audit_log WHERE character_id = ?1",
        );
        if start_date.is_some() {
            sql.push_str(" AND created_at >= ?2");
        }
        if end_date.is_some() {
            sql.push_str(if start_date.is_some() {
                " AND created_at <= ?3"
            } else {
                " AND created_at <= ?2"
            });
        }
        sql.push_str(" ORDER BY created_at DESC");

        let mut stmt = self.conn.prepare(&sql)?;

        let rows = match (start_date, end_date) {
            (Some(s), Some(e)) => stmt
                .query_map(params![character_id, s, e], AuditEntry::from_row)?
                .collect::<std::result::Result<Vec<_>, _>>(),
            (Some(s), None) => stmt
                .query_map(params![character_id, s], AuditEntry::from_row)?
                .collect::<std::result::Result<Vec<_>, _>>(),
            (None, Some(e)) => stmt
                .query_map(params![character_id, e], AuditEntry::from_row)?
                .collect::<std::result::Result<Vec<_>, _>>(),
            (None, None) => stmt
                .query_map(params![character_id], AuditEntry::from_row)?
                .collect::<std::result::Result<Vec<_>, _>>(),
        }
        .context("Failed to query audit log")?;

        Ok(rows)
    }

    /// Retrieve all audit log entries for a given action type (across all
    /// characters).
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_audit_log_by_action(&self, action_type: &str) -> Result<Vec<AuditEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, action_type, action_json, reason, operator_id, created_at \
             FROM soul_audit_log WHERE action_type = ?1 ORDER BY created_at DESC",
        )?;

        let rows = stmt
            .query_map(params![action_type], AuditEntry::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to query audit log by action")?;

        Ok(rows)
    }

    /// Return per-day action counts for a character (oldest day first).
    ///
    /// Each element is `(date_str, count)` where `date_str` is `"YYYY-MM-DD"`.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_audit_summary(&self, character_id: ClientId) -> Result<Vec<AuditDaySummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT date(created_at) AS day, COUNT(*) AS cnt FROM soul_audit_log WHERE \
             character_id = ?1 GROUP BY day ORDER BY day ASC",
        )?;

        let rows = stmt
            .query_map(params![character_id], |row| {
                Ok(AuditDaySummary {
                    date: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to query audit summary")?;

        Ok(rows)
    }

    /// Export the full audit log for a character as a CSV string.
    ///
    /// Columns: `id,character_id,action_type,action_json,reason,operator_id,
    /// created_at`
    ///
    /// # Errors
    ///
    /// Returns an error if the query or serialisation fails.
    pub fn export_audit_csv(&self, character_id: ClientId) -> Result<String> {
        let entries = self.get_audit_log(character_id, None, None)?;

        let mut out =
            String::from("id,character_id,action_type,action_json,reason,operator_id,created_at\n");
        for e in &entries {
            let reason = e.reason.as_deref().unwrap_or("");
            let operator = e.operator_id.as_deref().unwrap_or("");
            out.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                e.id,
                e.character_id,
                csv_field(&e.action_type),
                csv_field(&e.action_json),
                csv_field(reason),
                csv_field(operator),
                csv_field(&e.created_at),
            ));
        }
        Ok(out)
    }
}

/// A row from the `soul_audit_log` table.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    /// Database row ID.
    pub id: i64,
    /// Character this action belongs to.
    pub character_id: i64,
    /// Short label for the action (e.g., `"say"`, `"mood_change"`).
    pub action_type: String,
    /// Full action payload as a JSON string.
    pub action_json: String,
    /// Human-readable reason for the action, if provided.
    pub reason: Option<String>,
    /// Operator or system component that triggered the action.
    pub operator_id: Option<String>,
    /// ISO timestamp when the entry was created.
    pub created_at: String,
}

impl AuditEntry {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            character_id: row.get(1)?,
            action_type: row.get(2)?,
            action_json: row.get(3)?,
            reason: row.get(4)?,
            operator_id: row.get(5)?,
            created_at: row.get(6)?,
        })
    }
}

/// Per-day action count returned by [`MemoryStore::get_audit_summary`].
#[derive(Debug, Clone)]
pub struct AuditDaySummary {
    /// Calendar date (`"YYYY-MM-DD"`).
    pub date: String,
    /// Number of audit entries on that day.
    pub count: i64,
}

fn csv_field(s: &str) -> String {
    let formula_safe = if matches!(s.trim_start().chars().next(), Some('=' | '+' | '-' | '@')) {
        format!("'{s}")
    } else {
        s.to_owned()
    };

    if formula_safe.contains(',')
        || formula_safe.contains('"')
        || formula_safe.contains('\n')
        || formula_safe.contains('\r')
    {
        format!("\"{}\"", formula_safe.replace('"', "\"\""))
    } else {
        formula_safe
    }
}

#[cfg(test)]
fn format_context_for_llm(memories: &[MemoryRow]) -> String {
    memories
        .iter()
        .map(|row| {
            let zone = row.zone.as_deref().unwrap_or("-");
            format!(
                "[{}] zone={} mood={} importance={:.2} created_at={}",
                row.event_type, zone, row.mood_at_time, row.importance, row.created_at
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn memory_recency(created_at: &str) -> f32 {
    let created_at = match NaiveDateTime::parse_from_str(created_at, "%Y-%m-%d %H:%M:%S") {
        Ok(dt) => dt,
        Err(_) => return 0.0,
    };

    let age_seconds = Utc::now()
        .naive_utc()
        .signed_duration_since(created_at)
        .num_seconds()
        .max(0) as f32;
    (1.0 - (age_seconds / 86_400.0) / 30.0).clamp(0.0, 1.0)
}

fn memory_combined_score(row: &MemoryRow) -> f32 {
    row.importance * memory_recency(&row.created_at)
}

/// A row from the memories table.
#[derive(Debug, Clone)]
pub struct MemoryRow {
    /// Database row ID.
    pub id: i64,
    /// Event type label (e.g., "kill", "death", "loot").
    pub event_type: String,
    /// Serialized JSON of the `SoulEvent`.
    pub event_json: String,
    /// Zone where the event occurred, if applicable.
    pub zone: Option<String>,
    /// Mood state at the time of the event.
    pub mood_at_time: String,
    /// Memory importance score (decays over time, boosted by rehearsal).
    pub importance: f32,
    /// ISO timestamp when the memory was created.
    pub created_at: String,
    /// Whether this memory has been pruned via decay.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn event(&self) -> Result<SoulEvent> {
        serde_json::from_str(&self.event_json).context("Failed to deserialize SoulEvent")
    }
}

/// A row from the conversations table.
#[derive(Debug, Clone)]
pub struct ConversationRow {
    /// Database row ID.
    pub id: i64,
    /// Name of the speaker.
    pub speaker: String,
    /// True if the speaker is a real player (not a bot).
    pub is_player: bool,
    /// Chat channel (say, group, tell, etc.).
    pub channel: String,
    /// The message text.
    pub message: String,
    /// Sentiment score (-1.0 to 1.0), if analyzed.
    pub sentiment: Option<f32>,
    /// ISO timestamp when the conversation was recorded.
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

/// Extract a short label for the event type (used as `event_type` column).
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

/// Extract a short human-readable description from stored event JSON.
/// Falls back to the raw event type label if parsing fails.
fn describe_event(event_type: &str, event_json: &str) -> String {
    let Ok(event) = serde_json::from_str::<SoulEvent>(event_json) else {
        return event_type.to_string();
    };
    match event {
        SoulEvent::Death { killer, .. } => killer
            .map(|k| format!("killed by {k}"))
            .unwrap_or_else(|| "died".to_string()),
        SoulEvent::Kill { target, .. } => format!("killed {target}"),
        SoulEvent::Loot { item, .. } => format!("looted {item}"),
        SoulEvent::PlayerChat { player_name, .. } => format!("chat with {player_name}"),
        SoulEvent::BotChat { character_name } => format!("chat with {character_name}"),
        SoulEvent::Witnessed { description } => description,
        SoulEvent::MoodShift { from, to, .. } => format!("mood: {from:?} -> {to:?}"),
        SoulEvent::ZoneEnter { zone } => format!("entered {zone}"),
        SoulEvent::LevelUp { new_level } => format!("reached level {new_level}"),
        SoulEvent::GroupWipe { .. } => "group wipe".to_string(),
        SoulEvent::RelationshipChange { character, delta } => {
            format!("relationship with {character}: {delta:+.1}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use chrono;
    use rusqlite::Connection;
    use textquest_common::soul::{MoodState, SoulEvent};

    fn open_memory_store() -> MemoryStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        MemoryStore {
            conn,
            health: Arc::new(DbHealth::default()),
            fallback_cache: RefCell::new(VecDeque::new()),
        }
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
            .record(
                1,
                &kill_event("gnoll", "blackburrow"),
                MoodState::Neutral,
                1.0,
            )
            .unwrap();
        store
            .record(1, &kill_event("orc", "crushbone"), MoodState::Angry, 1.0)
            .unwrap();
        store
            .record(
                1,
                &loot_event("sword", "blackburrow"),
                MoodState::Happy,
                1.0,
            )
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
            .record(
                1,
                &kill_event("gnoll", "blackburrow"),
                MoodState::Neutral,
                1.0,
            )
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
            .record_conversation(1, "Dave", true, "say", "Hey there!", 0.8)
            .unwrap();
        store
            .record_conversation(1, "TestBot", false, "group", "On my way.", 0.0)
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
        store.record(1, &original, MoodState::Neutral, 1.0).unwrap();

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
            event_type_label(&SoulEvent::GroupWipe { zone: "".into() }),
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

    #[test]
    fn record_summary_inserts_successfully() {
        let store = open_memory_store();
        let result = store.record_summary(
            1,
            "2026-03-30 12:00",
            "2026-03-30 14:00",
            "Killed many gnolls in Blackburrow. Found a good camp spot.",
            Some("Excited"),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn record_summary_without_mood_trend() {
        let store = open_memory_store();
        let result = store.record_summary(1, "start", "end", "Did things.", None);
        assert!(result.is_ok());
    }

    #[test]
    fn generate_summary_limits_rows_per_hour() {
        let store = open_memory_store();

        for i in 0..150 {
            let player_name = format!("Player{i}");
            let event = SoulEvent::PlayerChat {
                player_name: player_name.clone(),
                sentiment: 0.0,
            };
            let event_json = serde_json::to_string(&event).unwrap();
            store
                .conn
                .execute(
                    "INSERT INTO memories (character_id, event_type, event_json, zone, \
                     mood_at_time, importance, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime(?7, 'unixepoch'))",
                    params![
                        1,
                        "PlayerChat",
                        event_json,
                        Option::<String>::None,
                        "Neutral",
                        1.0,
                        i as i64,
                    ],
                )
                .unwrap();
        }

        let summary = store.generate_summary(1, 0, 10_000).unwrap();
        assert!(
            summary.contains("chat with Player0"),
            "summary should include the earliest row in the capped window"
        );
        assert!(
            !summary.contains("chat with Player149"),
            "summary should not include rows beyond the summary cap"
        );
        assert!(
            summary.lines().count() <= SUMMARY_MEMORY_ROWS_MAX + 1,
            "summary should contain at most {} memory lines plus the mood trend",
            SUMMARY_MEMORY_ROWS_MAX
        );
    }

    #[test]
    fn record_shared_reference_inserts_successfully() {
        let store = open_memory_store();
        let id = store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 1.0)
            .unwrap();
        let result = store.record_shared_reference(1, 2, id, "Killed gnoll together");
        assert!(result.is_ok());
    }

    #[test]
    fn export_character_json_produces_valid_json() {
        let store = open_memory_store();
        store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Excited, 5.0)
            .unwrap();
        store
            .record(1, &loot_event("sword", "bb"), MoodState::Happy, 3.0)
            .unwrap();
        store
            .record_conversation(1, "Dave", true, "say", "Hello!", 0.8)
            .unwrap();

        let json = store.export_character_json(1).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["character_id"], 1);
        assert_eq!(parsed["memories"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["conversations"].as_array().unwrap().len(), 1);
        assert!(parsed["speech_style"]["vocabulary_level"].is_number());
    }

    #[test]
    fn export_character_json_empty_character() {
        let store = open_memory_store();
        let json = store.export_character_json(99).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["character_id"], 99);
        assert!(parsed["memories"].as_array().unwrap().is_empty());
        assert!(parsed["conversations"].as_array().unwrap().is_empty());
    }

    #[test]
    fn decay_tick_only_affects_specified_character() {
        let store = open_memory_store();
        store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 4.0)
            .unwrap();
        store
            .record(2, &kill_event("orc", "cb"), MoodState::Neutral, 4.0)
            .unwrap();

        store.decay_tick(1, 0.5).unwrap();

        let char1 = store.recall_recent(1, 10).unwrap();
        let char2 = store.recall_recent(2, 10).unwrap();
        assert!((char1[0].importance - 2.0).abs() < 0.01);
        assert!((char2[0].importance - 4.0).abs() < 0.01);
    }

    #[test]
    fn prune_skips_already_decayed() {
        let store = open_memory_store();
        store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 0.01)
            .unwrap();

        // First prune should mark it decayed
        let pruned = store.prune_low_importance(1, 0.1).unwrap();
        assert_eq!(pruned, 1);

        // Second prune should find nothing (already decayed)
        let pruned = store.prune_low_importance(1, 0.1).unwrap();
        assert_eq!(pruned, 0);
    }

    #[test]
    fn prune_does_not_touch_high_importance() {
        let store = open_memory_store();
        store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 5.0)
            .unwrap();

        let pruned = store.prune_low_importance(1, 0.1).unwrap();
        assert_eq!(pruned, 0);

        let memories = store.recall_recent(1, 10).unwrap();
        assert_eq!(memories.len(), 1);
    }

    #[test]
    fn connection_accessor() {
        let store = open_memory_store();
        let conn = store.connection();
        // Should be able to query via the raw connection
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn recall_conversations_filters_by_character() {
        let store = open_memory_store();
        store
            .record_conversation(1, "Alice", true, "say", "Hi", 0.5)
            .unwrap();
        store
            .record_conversation(2, "Bob", true, "tell", "Hey", 0.6)
            .unwrap();

        let c1 = store.recall_conversations(1, 10).unwrap();
        let c2 = store.recall_conversations(2, 10).unwrap();
        assert_eq!(c1.len(), 1);
        assert_eq!(c1[0].speaker, "Alice");
        assert_eq!(c2.len(), 1);
        assert_eq!(c2[0].speaker, "Bob");
    }

    #[test]
    fn recall_conversations_respects_limit() {
        let store = open_memory_store();
        for i in 0..10 {
            store
                .record_conversation(1, &format!("Player{}", i), true, "say", "msg", 0.5)
                .unwrap();
        }
        let convos = store.recall_conversations(1, 3).unwrap();
        assert_eq!(convos.len(), 3);
    }

    #[test]
    fn prune_conversations_keeps_only_recent_rows() {
        let store = open_memory_store();
        for i in 0..5 {
            store
                .record_conversation(1, "Alice", true, "say", &format!("msg-{i}"), 0.5)
                .unwrap();
        }

        let deleted = store.prune_conversations(1, 2).unwrap();
        assert_eq!(deleted, 3);

        let convos = store.recall_conversations(1, 10).unwrap();
        assert_eq!(convos.len(), 2);
    }

    #[test]
    fn summarize_player_chat_sentiment_counts_strong_messages() {
        let store = open_memory_store();
        store
            .record_conversation(1, "Alice", true, "say", "great pull", 0.8)
            .unwrap();
        store
            .record_conversation(1, "Alice", true, "say", "thanks", 0.9)
            .unwrap();
        store
            .record_conversation(1, "Alice", true, "say", "you are trash", -0.7)
            .unwrap();
        store
            .record_conversation(1, "Alice", true, "say", "neutral", 0.2)
            .unwrap();

        let summary = store.summarize_player_chat_sentiment(1, "Alice").unwrap();

        assert_eq!(
            summary.as_deref(),
            Some("had 2 positive chats, 1 negative chat")
        );
    }

    #[test]
    fn summarize_player_chat_sentiment_ignores_other_speakers_and_bots() {
        let store = open_memory_store();
        store
            .record_conversation(1, "Alice", false, "group", "bot line", 0.9)
            .unwrap();
        store
            .record_conversation(1, "Bob", true, "say", "bad", -0.9)
            .unwrap();
        store
            .record_conversation(1, "Alice", true, "say", "neutral", 0.2)
            .unwrap();

        let summary = store.summarize_player_chat_sentiment(1, "Alice").unwrap();

        assert!(summary.is_none());
    }

    #[test]
    fn event_type_label_all_variants() {
        assert_eq!(
            event_type_label(&SoulEvent::Kill {
                target: "".into(),
                zone: "".into()
            }),
            "kill"
        );
        assert_eq!(
            event_type_label(&SoulEvent::Loot {
                item: "".into(),
                zone: "".into()
            }),
            "loot"
        );
        assert_eq!(
            event_type_label(&SoulEvent::ZoneEnter { zone: "".into() }),
            "zone_enter"
        );
        assert_eq!(
            event_type_label(&SoulEvent::PlayerChat {
                player_name: "".into(),
                sentiment: 0.0
            }),
            "player_chat"
        );
        assert_eq!(
            event_type_label(&SoulEvent::MoodShift {
                from: MoodState::Neutral,
                to: MoodState::Happy,
                reason: "test".into(),
            }),
            "mood_shift"
        );
        assert_eq!(
            event_type_label(&SoulEvent::RelationshipChange {
                character: "".into(),
                delta: 0.0
            }),
            "relationship_change"
        );
    }

    #[test]
    fn event_zone_for_all_zone_events() {
        assert_eq!(
            event_zone(&SoulEvent::Kill {
                target: "orc".into(),
                zone: "cb".into()
            }),
            Some("cb".into())
        );
        assert_eq!(
            event_zone(&SoulEvent::Loot {
                item: "sword".into(),
                zone: "guk".into()
            }),
            Some("guk".into())
        );
        assert_eq!(
            event_zone(&SoulEvent::ZoneEnter {
                zone: "befallen".into()
            }),
            Some("befallen".into())
        );
        assert_eq!(
            event_zone(&SoulEvent::GroupWipe {
                zone: "lower_guk".into()
            }),
            Some("lower_guk".into())
        );
    }

    #[test]
    fn event_zone_none_for_non_zone_events() {
        assert_eq!(event_zone(&SoulEvent::LevelUp { new_level: 50 }), None);
        assert_eq!(
            event_zone(&SoulEvent::MoodShift {
                from: MoodState::Neutral,
                to: MoodState::Angry,
                reason: "test".into(),
            }),
            None
        );
        assert_eq!(
            event_zone(&SoulEvent::RelationshipChange {
                character: "Test".into(),
                delta: 10.0
            }),
            None
        );
    }

    #[test]
    fn memory_row_decayed_flag() {
        let store = open_memory_store();
        store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 0.01)
            .unwrap();

        // Before pruning, should not be decayed
        let conn = store.connection();
        let decayed: bool = conn
            .query_row(
                "SELECT decayed FROM memories WHERE character_id = 1",
                [],
                |r| r.get::<_, i32>(0).map(|v| v != 0),
            )
            .unwrap();
        assert!(!decayed);

        store.prune_low_importance(1, 0.1).unwrap();

        let decayed: bool = conn
            .query_row(
                "SELECT decayed FROM memories WHERE character_id = 1",
                [],
                |r| r.get::<_, i32>(0).map(|v| v != 0),
            )
            .unwrap();
        assert!(decayed);
    }

    #[test]
    fn conversation_row_clone_and_debug() {
        let store = open_memory_store();
        store
            .record_conversation(1, "Dave", true, "say", "Hello", 0.9)
            .unwrap();
        let convos = store.recall_conversations(1, 1).unwrap();
        let c = convos[0].clone();
        assert_eq!(c.speaker, "Dave");
        assert!(c.is_player);
        assert_eq!(c.channel, "say");
        assert_eq!(c.message, "Hello");
        assert!((c.sentiment.unwrap() - 0.9).abs() < 0.01);
        let _ = format!("{:?}", c);
    }

    #[test]
    fn memory_row_clone_and_debug() {
        let store = open_memory_store();
        store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Excited, 3.0)
            .unwrap();
        let memories = store.recall_recent(1, 1).unwrap();
        let m = memories[0].clone();
        assert_eq!(m.event_type, "kill");
        assert!(!m.decayed);
        let _ = format!("{:?}", m);
    }

    // --- LLM context tests ---

    #[test]
    fn get_llm_context_returns_top_n_by_score() {
        let store = open_memory_store();
        // Insert memories with differing importance — all recent so recency ~1.0
        store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 1.0)
            .unwrap();
        store
            .record(1, &kill_event("orc", "cb"), MoodState::Angry, 5.0)
            .unwrap();
        store
            .record(1, &loot_event("sword", "bb"), MoodState::Happy, 3.0)
            .unwrap();

        let top2 = store.get_llm_context(1, 2).unwrap();
        assert_eq!(top2.len(), 2);
        // Highest importance should come first
        assert!(
            top2[0].importance >= top2[1].importance,
            "Expected memories ordered by score desc: got {} then {}",
            top2[0].importance,
            top2[1].importance
        );
    }

    #[test]
    fn get_llm_context_respects_max_memories() {
        let store = open_memory_store();
        for i in 0..8 {
            store
                .record(
                    1,
                    &kill_event(&format!("mob_{i}"), "zone"),
                    MoodState::Neutral,
                    i as f32 + 1.0,
                )
                .unwrap();
        }
        let context = store.get_llm_context(1, 3).unwrap();
        assert_eq!(context.len(), 3);
    }

    #[test]
    fn get_llm_context_filters_by_character() {
        let store = open_memory_store();
        store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 2.0)
            .unwrap();
        store
            .record(2, &kill_event("orc", "cb"), MoodState::Angry, 9.0)
            .unwrap();

        let ctx1 = store.get_llm_context(1, 10).unwrap();
        let ctx2 = store.get_llm_context(2, 10).unwrap();
        assert_eq!(ctx1.len(), 1);
        assert_eq!(ctx2.len(), 1);
        assert_eq!(ctx1[0].event_type, "kill");
        assert_eq!(ctx2[0].event_type, "kill");
    }

    #[test]
    fn get_llm_context_excludes_decayed_memories() {
        let store = open_memory_store();
        store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 0.01)
            .unwrap();
        store
            .record(1, &kill_event("orc", "cb"), MoodState::Angry, 5.0)
            .unwrap();

        store.prune_low_importance(1, 0.1).unwrap();

        let context = store.get_llm_context(1, 10).unwrap();
        assert_eq!(context.len(), 1, "Decayed memory should be excluded");
        assert_eq!(context[0].event_type, "kill");
    }

    #[test]
    fn get_llm_context_empty_when_no_memories() {
        let store = open_memory_store();
        let context = store.get_llm_context(99, 10).unwrap();
        assert!(context.is_empty());
    }

    #[test]
    fn format_context_for_llm_empty_slice() {
        let text = format_context_for_llm(&[]);
        assert!(text.is_empty());
    }

    #[test]
    fn format_context_for_llm_includes_expected_fields() {
        let store = open_memory_store();
        store
            .record(
                1,
                &kill_event("gnoll", "blackburrow"),
                MoodState::Excited,
                2.5,
            )
            .unwrap();

        let memories = store.get_llm_context(1, 10).unwrap();
        let text = format_context_for_llm(&memories);

        assert!(text.contains("[kill]"), "Should contain event type");
        assert!(text.contains("blackburrow"), "Should contain zone");
        assert!(text.contains("Excited"), "Should contain mood");
        assert!(text.contains("2.50"), "Should contain formatted importance");
    }

    #[test]
    fn format_context_for_llm_multi_memory_separated_by_newline() {
        let store = open_memory_store();
        store
            .record(1, &kill_event("gnoll", "bb"), MoodState::Neutral, 1.0)
            .unwrap();
        store
            .record(1, &loot_event("sword", "bb"), MoodState::Happy, 2.0)
            .unwrap();

        let memories = store.get_llm_context(1, 10).unwrap();
        let text = format_context_for_llm(&memories);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "Two memories should produce two lines");
    }

    #[test]
    fn memory_recency_recent_scores_near_one() {
        // A memory created just now should score close to 1.0
        let now_str = chrono::Utc::now()
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let recency = memory_recency(&now_str);
        assert!(
            recency > 0.99,
            "Expected recency near 1.0 for a fresh memory, got {recency}"
        );
    }

    #[test]
    fn memory_recency_old_memory_scores_zero() {
        // A memory 60 days ago should score 0.0
        let old = (chrono::Utc::now() - chrono::TimeDelta::days(60))
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let recency = memory_recency(&old);
        assert!(
            recency == 0.0,
            "Expected recency 0.0 for a 60-day-old memory, got {recency}"
        );
    }

    #[test]
    fn memory_recency_invalid_timestamp_scores_zero() {
        let recency = memory_recency("not-a-date");
        assert_eq!(recency, 0.0);
    }

    #[test]
    fn csv_field_neutralizes_formula_cells() {
        assert_eq!(csv_field("=1+1"), "'=1+1");
        assert_eq!(csv_field("+SUM(A1:A2)"), "'+SUM(A1:A2)");
        assert_eq!(csv_field("-cmd"), "'-cmd");
        assert_eq!(csv_field("@evil"), "'@evil");
        assert_eq!(csv_field(" =1+1"), "' =1+1");
        assert_eq!(csv_field("\t=1+1"), "'\t=1+1");
        assert_eq!(csv_field("=1,2"), "\"'=1,2\"");
    }

    #[test]
    fn update_speech_patterns_overwrites() {
        let store = open_memory_store();
        let style1 = SpeechStyle {
            vocabulary_level: 0.3,
            ..Default::default()
        };
        store.update_speech_patterns(1, &style1).unwrap();

        let style2 = SpeechStyle {
            vocabulary_level: 0.9,
            catchphrases: vec!["Indeed!".into()],
            ..Default::default()
        };
        store.update_speech_patterns(1, &style2).unwrap();

        let loaded = store.get_speech_patterns(1).unwrap();
        assert!((loaded.vocabulary_level - 0.9).abs() < 0.01);
        assert_eq!(loaded.catchphrases, vec!["Indeed!"]);
    }

    // -- Retry / circuit-breaker / fallback cache tests --

    #[test]
    fn fallback_cache_starts_empty() {
        let store = open_memory_store();
        assert_eq!(store.fallback_cache_len(), 0);
        assert!(!store.is_circuit_open());
    }

    #[test]
    fn circuit_breaker_opens_after_threshold_failures() {
        let health = Arc::new(DbHealth::default());
        let dummy_err = anyhow::anyhow!("simulated DB error");
        for _ in 0..CIRCUIT_BREAKER_THRESHOLD {
            assert!(!health.is_open(), "circuit should still be closed");
            health.record_failure("test_op", &dummy_err);
        }
        assert!(
            health.is_open(),
            "circuit should be open after {} failures",
            CIRCUIT_BREAKER_THRESHOLD
        );
    }

    #[test]
    fn circuit_breaker_closes_on_success() {
        let health = Arc::new(DbHealth::default());
        let dummy_err = anyhow::anyhow!("simulated DB error");
        for _ in 0..CIRCUIT_BREAKER_THRESHOLD {
            health.record_failure("test_op", &dummy_err);
        }
        assert!(health.is_open());
        health.record_success();
        assert!(!health.is_open());
    }

    #[test]
    fn record_uses_fallback_cache_when_circuit_open() {
        let store = open_memory_store();
        // Force the circuit open.
        let dummy_err = anyhow::anyhow!("simulated DB error");
        for _ in 0..CIRCUIT_BREAKER_THRESHOLD {
            store.health.record_failure("test", &dummy_err);
        }
        assert!(store.is_circuit_open());

        let event = kill_event("a gnoll", "blackburrow");
        let id = store.record(1, &event, MoodState::Neutral, 1.0).unwrap();
        // Returns synthetic ID (-1) rather than a real row ID.
        assert_eq!(id, -1);
        // Entry goes into the fallback cache, not the DB.
        assert_eq!(store.fallback_cache_len(), 1);
        let db_count: i64 = store
            .connection()
            .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))
            .unwrap();
        assert_eq!(db_count, 0);
    }

    #[test]
    fn fallback_cache_evicts_oldest_when_full() {
        let store = open_memory_store();
        // Force circuit open so all records go to the cache.
        let dummy_err = anyhow::anyhow!("simulated DB error");
        for _ in 0..CIRCUIT_BREAKER_THRESHOLD {
            store.health.record_failure("test", &dummy_err);
        }

        let event = kill_event("mob", "zone");
        for _ in 0..FALLBACK_CACHE_MAX + 10 {
            store.record(1, &event, MoodState::Neutral, 1.0).unwrap();
        }
        // Cache should be capped at FALLBACK_CACHE_MAX.
        assert_eq!(store.fallback_cache_len(), FALLBACK_CACHE_MAX);
    }

    #[test]
    fn retry_db_op_succeeds_on_first_attempt() {
        let result: Result<i32> = retry_db_op("noop", || Ok(42));
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn retry_db_op_returns_last_error_after_exhaustion() {
        let mut call_count = 0usize;
        let result: Result<i32> = retry_db_op("always_fail", || {
            call_count += 1;
            Err(anyhow::anyhow!("permanent failure"))
        });
        assert!(result.is_err());
        // 1 initial + 5 retries = 6 total attempts.
        assert_eq!(call_count, 6);
    }

    #[test]
    fn retry_db_op_succeeds_on_second_attempt() {
        let mut call_count = 0usize;
        let result: Result<i32> = retry_db_op("fail_once", || {
            call_count += 1;
            if call_count == 1 {
                Err(anyhow::anyhow!("transient"))
            } else {
                Ok(99)
            }
        });
        assert_eq!(result.unwrap(), 99);
        assert_eq!(call_count, 2);
    }
}
