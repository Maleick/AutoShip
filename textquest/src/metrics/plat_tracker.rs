//! Platinum tracking — MQ2PlatTracker parity.
//!
//! Records per-transaction platinum changes, computes plat/hour rates,
//! and keeps per-character session history for integration with the
//! economy/ledger and self-improvement loop aggregate tables.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::time::Instant;

const MAX_TRANSACTIONS: usize = 2000;
const WINDOW_SECS: f32 = 3600.0;

/// Category of a platinum transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatCategory {
    /// Looted from a mob corpse.
    Loot,
    /// Sold to a vendor.
    VendorSell,
    /// Purchased from a vendor.
    VendorBuy,
    /// Repair cost paid.
    Repair,
    /// Tradeskill material cost or product sale.
    Tradeskill,
    /// Player-to-player trade.
    Trade,
    /// Other or unknown source.
    Other,
}

/// A single platinum change event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatTransaction {
    /// Character name associated with this event.
    pub character: String,
    /// Plat delta (positive = gained, negative = spent).
    pub delta: i64,
    /// Category for ledger integration.
    pub category: PlatCategory,
    /// Optional note (mob name, item, etc.).
    pub note: Option<String>,
    /// Unix epoch timestamp of the transaction.
    pub timestamp: i64,
    /// Instant for rate calculations (not serialized).
    #[serde(skip)]
    pub instant: Option<Instant>,
}

/// Tracks platinum transactions and computes plat/hour rate.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PlatTracker {
    transactions: VecDeque<PlatTransaction>,
}

impl PlatTracker {
    pub fn new() -> Self {
        Self {
            transactions: VecDeque::new(),
        }
    }

    /// Record a new platinum transaction.
    pub fn record(
        &mut self,
        character: impl Into<String>,
        delta: i64,
        category: PlatCategory,
        note: Option<String>,
        timestamp: i64,
        now: Instant,
    ) {
        if self.transactions.len() >= MAX_TRANSACTIONS {
            self.transactions.pop_front();
        }
        self.transactions.push_back(PlatTransaction {
            character: character.into(),
            delta,
            category,
            note,
            timestamp,
            instant: Some(now),
        });
    }

    /// Net plat gained over the 60-minute rolling window for `character`.
    ///
    /// Only transactions with a valid `instant` within the window are counted.
    /// Returns 0.0 if fewer than two qualifying transactions exist.
    pub fn plat_per_hour(&self, character: &str, now: Instant) -> f64 {
        let window = std::time::Duration::from_secs_f32(WINDOW_SECS);

        let qualifying: Vec<&PlatTransaction> = self
            .transactions
            .iter()
            .filter(|t| {
                t.character == character
                    && t.instant
                        .map(|i| now.duration_since(i) <= window)
                        .unwrap_or(false)
            })
            .collect();

        if qualifying.len() < 2 {
            return 0.0;
        }

        let oldest = qualifying.first().expect("len >= 2");
        let newest = qualifying.last().expect("len >= 2");

        let oldest_instant = oldest.instant.expect("filtered for Some");
        let newest_instant = newest.instant.expect("filtered for Some");
        let elapsed_secs = newest_instant
            .duration_since(oldest_instant)
            .as_secs_f64();

        if elapsed_secs <= 0.0 {
            return 0.0;
        }

        let net: i64 = qualifying.iter().map(|t| t.delta).sum();
        net as f64 / elapsed_secs * f64::from(WINDOW_SECS)
    }

    /// Total net plat change this session for `character`.
    pub fn session_net(&self, character: &str) -> i64 {
        self.transactions
            .iter()
            .filter(|t| t.character == character)
            .map(|t| t.delta)
            .sum()
    }

    /// Net plat broken down by category for `character`.
    pub fn by_category(&self, character: &str) -> HashMap<PlatCategory, i64> {
        let mut map: HashMap<PlatCategory, i64> = HashMap::new();
        for t in self.transactions.iter().filter(|t| t.character == character) {
            *map.entry(t.category).or_insert(0) += t.delta;
        }
        map
    }

    /// Most recent `limit` transactions for `character`.
    pub fn recent(&self, character: &str, limit: usize) -> Vec<&PlatTransaction> {
        let mut result: Vec<&PlatTransaction> = self
            .transactions
            .iter()
            .filter(|t| t.character == character)
            .collect();
        if result.len() > limit {
            let skip = result.len() - limit;
            result.drain(..skip);
        }
        result
    }

    /// All transactions (for serialization/persistence).
    pub fn all_transactions(&self) -> &VecDeque<PlatTransaction> {
        &self.transactions
    }
}

/// Per-character session store — mirrors `KillSessionStore` pattern.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PlatSessionStore {
    sessions: HashMap<String, PlatTracker>,
}

impl PlatSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start tracking a character if not already present.
    pub fn start_session(&mut self, character: impl Into<String>) {
        let character = character.into();
        self.sessions
            .entry(character)
            .or_insert_with(PlatTracker::new);
    }

    pub fn get_session_mut(&mut self, character: &str) -> Option<&mut PlatTracker> {
        self.sessions.get_mut(character)
    }

    pub fn get_session(&self, character: &str) -> Option<&PlatTracker> {
        self.sessions.get(character)
    }

    /// Session-level net plat for each character.
    pub fn summary(&self) -> HashMap<String, i64> {
        self.sessions
            .iter()
            .map(|(k, v)| (k.clone(), v.session_net(k)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn base() -> Instant {
        Instant::now()
    }

    fn offset(b: Instant, secs: u64) -> Instant {
        b + Duration::from_secs(secs)
    }

    #[test]
    fn plat_per_hour_basic() {
        let mut tracker = PlatTracker::new();
        let b = base();
        // 100 plat gained at t=0, then 0 delta at t=1800s → net 100 over 30 min → 200/hr
        tracker.record("Warrior", 100, PlatCategory::Loot, None, 0, b);
        tracker.record("Warrior", 0, PlatCategory::Loot, None, 1800, offset(b, 1800));
        let rate = tracker.plat_per_hour("Warrior", offset(b, 1800));
        let diff = (rate - 200.0_f64).abs();
        assert!(diff < 0.1, "expected ~200, got {rate}");
    }

    #[test]
    fn plat_per_hour_one_transaction_returns_zero() {
        let mut tracker = PlatTracker::new();
        let b = base();
        tracker.record("Cleric", 500, PlatCategory::VendorSell, None, 0, b);
        let rate = tracker.plat_per_hour("Cleric", b);
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn session_net_sums_correctly() {
        let mut tracker = PlatTracker::new();
        let b = base();
        tracker.record("Rogue", 200, PlatCategory::Loot, None, 0, b);
        tracker.record("Rogue", -50, PlatCategory::Repair, None, 60, offset(b, 60));
        tracker.record("Rogue", 300, PlatCategory::VendorSell, None, 120, offset(b, 120));
        assert_eq!(tracker.session_net("Rogue"), 450);
    }

    #[test]
    fn by_category_breakdown() {
        let mut tracker = PlatTracker::new();
        let b = base();
        tracker.record("Shaman", 100, PlatCategory::Loot, None, 0, b);
        tracker.record("Shaman", 200, PlatCategory::Loot, None, 1, offset(b, 1));
        tracker.record("Shaman", -30, PlatCategory::Repair, None, 2, offset(b, 2));
        let breakdown = tracker.by_category("Shaman");
        assert_eq!(*breakdown.get(&PlatCategory::Loot).unwrap(), 300);
        assert_eq!(*breakdown.get(&PlatCategory::Repair).unwrap(), -30);
    }

    #[test]
    fn recent_filters_by_character() {
        let mut tracker = PlatTracker::new();
        let b = base();
        tracker.record("Druid", 100, PlatCategory::Loot, None, 0, b);
        tracker.record("Wizard", 50, PlatCategory::Loot, None, 1, offset(b, 1));
        let result = tracker.recent("Druid", 10);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].character, "Druid");
    }

    #[test]
    fn recent_limit_respected() {
        let mut tracker = PlatTracker::new();
        let b = base();
        for i in 0_i64..10 {
            tracker.record(
                "Monk",
                i * 10,
                PlatCategory::Loot,
                None,
                i,
                offset(b, i as u64),
            );
        }
        let result = tracker.recent("Monk", 3);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn cap_at_max_transactions() {
        let mut tracker = PlatTracker::new();
        let b = base();
        for i in 0..2100_u64 {
            tracker.record("Bard", 1, PlatCategory::Loot, None, i as i64, offset(b, i));
        }
        assert_eq!(tracker.all_transactions().len(), MAX_TRANSACTIONS);
    }

    #[test]
    fn session_store_start_and_get() {
        let mut store = PlatSessionStore::new();
        store.start_session("Paladin");
        let b = base();
        store
            .get_session_mut("Paladin")
            .unwrap()
            .record("Paladin", 100, PlatCategory::Loot, None, 0, b);
        let net = store.get_session("Paladin").unwrap().session_net("Paladin");
        assert_eq!(net, 100);
    }

    #[test]
    fn plat_per_hour_outside_window_excluded() {
        let mut tracker = PlatTracker::new();
        let b = base();
        // Old transaction at b; "now" is 90 minutes later → outside 60-min window
        tracker.record("Necromancer", 500, PlatCategory::Loot, None, 0, b);
        let now = offset(b, 90 * 60);
        let rate = tracker.plat_per_hour("Necromancer", now);
        assert_eq!(rate, 0.0, "single in-window txn should return 0");
    }
}
