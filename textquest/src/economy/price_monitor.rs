//! Passive Krono trade-price monitoring from DLL-captured chat.
//!
//! The EverQuest client already streams every rendered chat line through
//! `CEverQuest::dsp_chat`, which TextQuest captures via the DLL's existing
//! `PollChat` IPC path. This module turns those chat lines into structured
//! Krono-denominated trade observations and persists them for later trend work.

use anyhow::{Context, Result};
use regex::Regex;
use rusqlite::{Connection, params};
use std::{collections::HashMap, path::Path, sync::OnceLock};
use textquest_common::chat::{ChatChannel, ChatEvent};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS trade_price_observations (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    observed_at_ms      INTEGER NOT NULL,
    source_pid          INTEGER NOT NULL,
    zone                TEXT NOT NULL,
    channel             TEXT NOT NULL,
    speaker             TEXT NOT NULL,
    intent              TEXT NOT NULL,
    item_name           TEXT NOT NULL,
    price_milli_krono   INTEGER NOT NULL,
    raw_message         TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_trade_price_item_ts
    ON trade_price_observations(item_name, observed_at_ms DESC);
CREATE INDEX IF NOT EXISTS idx_trade_price_zone_ts
    ON trade_price_observations(zone, observed_at_ms DESC);
";

const DEDUPE_WINDOW_MS: i64 = 5_000;
const DEDUPE_RETENTION_MS: i64 = 60_000;
const MAX_STORED_OBSERVATIONS: i64 = 50_000;

/// A normalized buy/sell/trade intent extracted from market chatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TradeIntent {
    Buy,
    Sell,
    Trade,
}

impl TradeIntent {
    fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
            Self::Trade => "trade",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "buy" => Some(Self::Buy),
            "sell" => Some(Self::Sell),
            "trade" => Some(Self::Trade),
            _ => None,
        }
    }
}

/// A single Krono-denominated price sighting captured from trade chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradePriceObservation {
    pub source_pid: u32,
    pub zone: String,
    pub channel: ChatChannel,
    pub speaker: String,
    pub intent: TradeIntent,
    pub item_name: String,
    pub price_milli_krono: i64,
    pub observed_at_ms: i64,
    pub raw_message: String,
}

/// SQLite-backed store for passive trade-price observations.
pub struct TradePriceStore {
    conn: Connection,
}

impl TradePriceStore {
    /// Open (or create) the trade-price database at `path`.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create trade-price DB dir: {}", parent.display())
            })?;
        }

        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open trade-price DB: {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("Failed to initialize trade-price DB pragmas")?;
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize trade-price schema")?;

        Ok(Self { conn })
    }

    /// Open an in-memory trade-price DB for tests.
    #[cfg(test)]
    pub fn open_memory() -> Result<Self> {
        let conn =
            Connection::open_in_memory().context("Failed to open in-memory trade-price DB")?;
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize in-memory trade-price schema")?;
        Ok(Self { conn })
    }

    /// Insert a single observation into the local store.
    pub fn insert_observation(&self, observation: &TradePriceObservation) -> Result<i64> {
        self.conn
            .execute(
                "INSERT INTO trade_price_observations (
                    observed_at_ms, source_pid, zone, channel, speaker, intent,
                    item_name, price_milli_krono, raw_message
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    observation.observed_at_ms,
                    i64::from(observation.source_pid),
                    &observation.zone,
                    channel_key(&observation.channel),
                    &observation.speaker,
                    observation.intent.as_str(),
                    &observation.item_name,
                    observation.price_milli_krono,
                    &observation.raw_message,
                ],
            )
            .context("Failed to insert trade-price observation")?;
        self.prune_observations_over_limit()
            .context("Failed to enforce trade-price retention limit")?;

        Ok(self.conn.last_insert_rowid())
    }

    fn prune_observations_over_limit(&self) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM trade_price_observations
                 WHERE id NOT IN (
                    SELECT id
                    FROM trade_price_observations
                    ORDER BY id DESC
                    LIMIT ?1
                 )",
                params![MAX_STORED_OBSERVATIONS],
            )
            .context("Failed to prune old trade-price observations")?;

        Ok(())
    }

    /// Return the newest trade-price observations first.
    pub fn recent_observations(&self, limit: u32) -> Result<Vec<TradePriceObservation>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT observed_at_ms, source_pid, zone, channel, speaker, intent,
                        item_name, price_milli_krono, raw_message
                 FROM trade_price_observations
                 ORDER BY id DESC
                 LIMIT ?1",
            )
            .context("Failed to prepare trade-price query")?;

        let rows = stmt
            .query_map(params![limit], |row| {
                let channel: String = row.get(3)?;
                let intent: String = row.get(5)?;
                let source_pid = row.get::<_, i64>(1)?;
                Ok(TradePriceObservation {
                    observed_at_ms: row.get(0)?,
                    source_pid: u32::try_from(source_pid).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Integer,
                            Box::new(error),
                        )
                    })?,
                    zone: row.get(2)?,
                    channel: channel_from_key(&channel).ok_or_else(|| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Text,
                            Box::new(std::io::Error::other(format!(
                                "unknown chat channel key: {channel}"
                            ))),
                        )
                    })?,
                    speaker: row.get(4)?,
                    intent: TradeIntent::from_str(&intent).ok_or_else(|| {
                        rusqlite::Error::FromSqlConversionFailure(
                            5,
                            rusqlite::types::Type::Text,
                            Box::new(std::io::Error::other(format!(
                                "unknown trade intent: {intent}"
                            ))),
                        )
                    })?,
                    item_name: row.get(6)?,
                    price_milli_krono: row.get(7)?,
                    raw_message: row.get(8)?,
                })
            })
            .context("Failed to query trade-price observations")?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect trade-price rows")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ObservationFingerprint {
    zone: String,
    channel: String,
    speaker: String,
    intent: TradeIntent,
    item_name: String,
    price_milli_krono: i64,
}

impl ObservationFingerprint {
    fn new(observation: &TradePriceObservation) -> Self {
        Self {
            zone: normalize_zone(&observation.zone),
            channel: channel_key(&observation.channel).to_string(),
            speaker: observation.speaker.to_ascii_lowercase(),
            intent: observation.intent,
            item_name: normalize_space(&observation.item_name).to_ascii_lowercase(),
            price_milli_krono: observation.price_milli_krono,
        }
    }
}

/// In-memory dedupe + persistence wrapper for live trade-chat monitoring.
pub struct TradePriceMonitor {
    store: TradePriceStore,
    recently_seen: HashMap<ObservationFingerprint, i64>,
}

impl TradePriceMonitor {
    /// Open the monitor against a persistent SQLite file.
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            store: TradePriceStore::open(path)?,
            recently_seen: HashMap::new(),
        })
    }

    /// Create an in-memory monitor for unit tests.
    #[cfg(test)]
    pub fn for_tests() -> Result<Self> {
        Ok(Self {
            store: TradePriceStore::open_memory()?,
            recently_seen: HashMap::new(),
        })
    }

    /// Expose the underlying store for assertions and follow-on queries.
    pub fn store(&self) -> &TradePriceStore {
        &self.store
    }

    /// Record a single parsed EQ chat line, returning `true` when it produced a
    /// new persisted price observation.
    pub fn record_chat(
        &mut self,
        source_pid: u32,
        zone: &str,
        local_character_name: Option<&str>,
        chat: &ChatEvent,
        observed_at_ms: i64,
    ) -> Result<bool> {
        let speaker = if chat.sender.eq_ignore_ascii_case("You") {
            local_character_name.unwrap_or("You")
        } else {
            chat.sender.as_str()
        };

        let Some(observation) =
            parse_trade_observation(source_pid, zone, speaker, chat, observed_at_ms)
        else {
            return Ok(false);
        };

        self.prune_recently_seen(observed_at_ms);
        let fingerprint = ObservationFingerprint::new(&observation);
        if self
            .recently_seen
            .get(&fingerprint)
            .is_some_and(|last_seen| observed_at_ms - *last_seen <= DEDUPE_WINDOW_MS)
        {
            return Ok(false);
        }

        self.store.insert_observation(&observation)?;
        self.recently_seen.insert(fingerprint, observed_at_ms);
        Ok(true)
    }

    fn prune_recently_seen(&mut self, now_ms: i64) {
        self.recently_seen
            .retain(|_, last_seen| now_ms - *last_seen <= DEDUPE_RETENTION_MS);
    }
}

/// Parse a Krono-denominated trade observation from a structured chat event.
pub fn parse_trade_observation(
    source_pid: u32,
    zone: &str,
    speaker: &str,
    chat: &ChatEvent,
    observed_at_ms: i64,
) -> Option<TradePriceObservation> {
    if !matches!(chat.channel, ChatChannel::Ooc | ChatChannel::Auction) || !is_trade_hub_zone(zone)
    {
        return None;
    }

    let message = normalize_space(&chat.message);
    let price_match = price_regex().find(&message)?;
    let price_text = &message[price_match.start()..price_match.end()];
    let price_milli_krono = parse_price_milli_krono(price_text)?;

    let left = clean_item_candidate(&message[..price_match.start()]);
    let right = clean_item_candidate(&message[price_match.end()..]);
    let item_name = select_item_candidate(&left, &right)?;
    let intent = parse_intent(&message);

    Some(TradePriceObservation {
        source_pid,
        zone: normalize_zone_key(zone),
        channel: chat.channel.clone(),
        speaker: speaker.trim().to_string(),
        intent,
        item_name,
        price_milli_krono,
        observed_at_ms,
        raw_message: message,
    })
}

fn parse_intent(message: &str) -> TradeIntent {
    if intent_sell_regex().is_match(message) {
        TradeIntent::Sell
    } else if intent_buy_regex().is_match(message) {
        TradeIntent::Buy
    } else {
        TradeIntent::Trade
    }
}

fn select_item_candidate(left: &str, right: &str) -> Option<String> {
    match (score_candidate(left), score_candidate(right)) {
        (0, 0) => None,
        (left_score, right_score) if left_score > right_score => Some(left.to_string()),
        (left_score, right_score) if right_score > left_score => Some(right.to_string()),
        _ if left.len() >= right.len() && !left.is_empty() => Some(left.to_string()),
        _ if !right.is_empty() => Some(right.to_string()),
        _ => None,
    }
}

fn score_candidate(candidate: &str) -> usize {
    candidate
        .split_whitespace()
        .filter(|token| token.chars().any(|ch| ch.is_alphanumeric()))
        .count()
}

fn clean_item_candidate(candidate: &str) -> String {
    let mut cleaned = trim_trade_noise(candidate);

    loop {
        let next = trim_trade_noise(leading_trade_regex().replace(&cleaned, "").as_ref());
        if next == cleaned {
            break;
        }
        cleaned = next;
    }

    loop {
        let next = trim_trade_noise(trailing_trade_regex().replace(&cleaned, "").as_ref());
        if next == cleaned {
            break;
        }
        cleaned = next;
    }

    cleaned
}

fn trim_trade_noise(value: &str) -> String {
    normalize_space(value.trim_matches(|ch: char| {
        matches!(
            ch,
            ' ' | ',' | ';' | ':' | '-' | '/' | '\'' | '"' | '[' | ']' | '(' | ')' | '|'
        )
    }))
}

fn parse_price_milli_krono(value: &str) -> Option<i64> {
    let captures = price_capture_regex().captures(value)?;
    let amount = captures.name("amount")?.as_str();
    let (whole, fractional) = match amount.split_once('.') {
        Some((whole, fractional)) => (whole, fractional),
        None => (amount, ""),
    };

    let whole: i64 = whole.parse().ok()?;
    let mut fractional = fractional.chars().take(3).collect::<String>();
    while fractional.len() < 3 {
        fractional.push('0');
    }

    let fractional: i64 = if fractional.is_empty() {
        0
    } else {
        fractional.parse().ok()?
    };

    whole.checked_mul(1_000)?.checked_add(fractional)
}

fn channel_key(channel: &ChatChannel) -> &'static str {
    match channel {
        ChatChannel::Say => "say",
        ChatChannel::Tell => "tell",
        ChatChannel::TellOut => "tell_out",
        ChatChannel::Group => "group",
        ChatChannel::Guild => "guild",
        ChatChannel::Raid => "raid",
        ChatChannel::Shout => "shout",
        ChatChannel::Ooc => "ooc",
        ChatChannel::Auction => "auction",
    }
}

fn channel_from_key(value: &str) -> Option<ChatChannel> {
    match value {
        "say" => Some(ChatChannel::Say),
        "tell" => Some(ChatChannel::Tell),
        "tell_out" => Some(ChatChannel::TellOut),
        "group" => Some(ChatChannel::Group),
        "guild" => Some(ChatChannel::Guild),
        "raid" => Some(ChatChannel::Raid),
        "shout" => Some(ChatChannel::Shout),
        "ooc" => Some(ChatChannel::Ooc),
        "auction" => Some(ChatChannel::Auction),
        _ => None,
    }
}

fn is_trade_hub_zone(zone: &str) -> bool {
    matches!(
        normalize_zone(zone).as_str(),
        "nexus" | "poknowledge" | "planeofknowledge" | "theplaneofknowledge"
    )
}

fn normalize_zone_key(zone: &str) -> String {
    let normalized = normalize_zone(zone);
    match normalized.as_str() {
        "theplaneofknowledge" | "planeofknowledge" => "poknowledge".to_string(),
        other => other.to_string(),
    }
}

fn normalize_zone(zone: &str) -> String {
    zone.chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn normalize_space(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn price_regex() -> &'static Regex {
    static PRICE_REGEX: OnceLock<Regex> = OnceLock::new();
    PRICE_REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b\d+(?:\.\d+)?\s*(?:krono|kronos|kr)\b").expect("valid trade price regex")
    })
}

fn price_capture_regex() -> &'static Regex {
    static PRICE_CAPTURE_REGEX: OnceLock<Regex> = OnceLock::new();
    PRICE_CAPTURE_REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b(?P<amount>\d+(?:\.\d+)?)\s*(?:krono|kronos|kr)\b")
            .expect("valid trade price capture regex")
    })
}

fn leading_trade_regex() -> &'static Regex {
    static LEADING_TRADE_REGEX: OnceLock<Regex> = OnceLock::new();
    LEADING_TRADE_REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)^(?:wts|wtb|wtt|selling|buying|trading|looking\s+for|lf|paying|for|each|ea|x)\b[\s,:;\-/]*",
        )
        .expect("valid leading trade regex")
    })
}

fn trailing_trade_regex() -> &'static Regex {
    static TRAILING_TRADE_REGEX: OnceLock<Regex> = OnceLock::new();
    TRAILING_TRADE_REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)[\s,:;\-/]*(?:for|each|ea|firm|obo|pst|offers?|only|paying|cheap|negotiable)\b$",
        )
        .expect("valid trailing trade regex")
    })
}

fn intent_sell_regex() -> &'static Regex {
    static INTENT_SELL_REGEX: OnceLock<Regex> = OnceLock::new();
    INTENT_SELL_REGEX
        .get_or_init(|| Regex::new(r"(?i)\b(?:wts|selling)\b").expect("valid sell intent regex"))
}

fn intent_buy_regex() -> &'static Regex {
    static INTENT_BUY_REGEX: OnceLock<Regex> = OnceLock::new();
    INTENT_BUY_REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b(?:wtb|buying|looking\s+for|lf|paying)\b")
            .expect("valid buy intent regex")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_observations_rejects_invalid_source_pid() {
        let store = TradePriceStore::open_memory().expect("in-memory trade store");
        store
            .conn
            .execute(
                "INSERT INTO trade_price_observations (
                    observed_at_ms, source_pid, zone, channel, speaker, intent,
                    item_name, price_milli_krono, raw_message
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    1_i64,
                    -1_i64,
                    "nexus",
                    "auction",
                    "Trader",
                    "sell",
                    "Fungi Tunic",
                    3_000_i64,
                    "WTS fungi 3 krono",
                ],
            )
            .expect("insert corrupted row");

        let error = store
            .recent_observations(1)
            .expect_err("invalid pid should fail conversion");
        let message = format!("{error:#}");
        assert!(message.contains("trade-price rows"));
    }

    #[test]
    fn insert_observation_enforces_max_row_limit() {
        let store = TradePriceStore::open_memory().expect("in-memory trade store");
        let base_observation = TradePriceObservation {
            source_pid: 7,
            zone: "nexus".to_string(),
            channel: ChatChannel::Auction,
            speaker: "Trader".to_string(),
            intent: TradeIntent::Sell,
            item_name: "Fungi Tunic".to_string(),
            price_milli_krono: 3_000,
            observed_at_ms: 0,
            raw_message: "WTS Fungi Tunic 3 krono".to_string(),
        };

        for observed_at_ms in 0..MAX_STORED_OBSERVATIONS {
            store
                .conn
                .execute(
                    "INSERT INTO trade_price_observations (
                        observed_at_ms, source_pid, zone, channel, speaker, intent,
                        item_name, price_milli_krono, raw_message
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        observed_at_ms,
                        i64::from(base_observation.source_pid),
                        &base_observation.zone,
                        base_observation.channel.as_str(),
                        &base_observation.speaker,
                        base_observation.intent.as_str(),
                        &base_observation.item_name,
                        base_observation.price_milli_krono,
                        format!("WTS Fungi Tunic {observed_at_ms} krono"),
                    ],
                )
                .expect("seed trade observation inserted");
        }

        let mut observation = base_observation.clone();
        observation.observed_at_ms = MAX_STORED_OBSERVATIONS;
        observation.raw_message = format!("WTS Fungi Tunic {} krono", MAX_STORED_OBSERVATIONS);
        store
            .insert_observation(&observation)
            .expect("trade observation inserted");
        let history = store
            .recent_observations((MAX_STORED_OBSERVATIONS + 1) as u32)
            .expect("recent trade observation query");

        assert_eq!(history.len() as i64, MAX_STORED_OBSERVATIONS);
        assert_eq!(history[0].observed_at_ms, MAX_STORED_OBSERVATIONS);
        assert_eq!(history.last().map(|obs| obs.observed_at_ms), Some(1));
    }
}
