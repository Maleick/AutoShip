#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::items_after_statements,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::redundant_field_names
)]

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoinStack {
    pub plat: i64,
    pub gold: i64,
    pub silver: i64,
    pub copper: i64,
}

impl CoinStack {
    pub fn new() -> Self {
        Self {
            plat: 0,
            gold: 0,
            silver: 0,
            copper: 0,
        }
    }

    pub fn from_platinum(plat: i64) -> Self {
        Self {
            plat,
            gold: 0,
            silver: 0,
            copper: 0,
        }
    }

    pub fn total_copper(&self) -> i64 {
        (self.plat * 1000) + (self.gold * 100) + (self.silver * 10) + self.copper
    }

    pub fn from_copper(copper: i64) -> Self {
        let plat = copper / 1000;
        let remainder = copper % 1000;
        let gold = remainder / 100;
        let remainder = remainder % 100;
        let silver = remainder / 10;
        let copper = remainder % 10;
        Self {
            plat,
            gold,
            silver,
            copper,
        }
    }

    pub fn add(&self, other: &CoinStack) -> CoinStack {
        CoinStack {
            plat: self.plat + other.plat,
            gold: self.gold + other.gold,
            silver: self.silver + other.silver,
            copper: self.copper + other.copper,
        }
    }

    pub fn subtract(&self, other: &CoinStack) -> CoinStack {
        CoinStack {
            plat: self.plat - other.plat,
            gold: self.gold - other.gold,
            silver: self.silver - other.silver,
            copper: self.copper - other.copper,
        }
    }

    pub fn format_verbose(&self) -> String {
        let mut parts = Vec::new();
        if self.plat != 0 {
            parts.push(format!("{}p", self.plat));
        }
        if self.gold != 0 {
            parts.push(format!("{}g", self.gold));
        }
        if self.silver != 0 {
            parts.push(format!("{}s", self.silver));
        }
        if self.copper != 0 {
            parts.push(format!("{}c", self.copper));
        }
        if parts.is_empty() {
            String::from("0p")
        } else {
            parts.join(" ")
        }
    }

    pub fn format_compact(&self) -> String {
        format!("{}p", self.total_copper() / 1000)
    }
}

impl Default for CoinStack {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct TransactionRecord {
    #[serde(skip, default = "Instant::now")]
    pub timestamp: Instant,
    pub coin_delta: CoinStack,
    pub transaction_type: TransactionType,
    pub character_id: Option<String>,
    pub item_name: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    VendorSale,
    VendorPurchase,
    LootDrop,
    LootPickup,
    BankDeposit,
    BankWithdrawal,
    TradeReceived,
    TradeSent,
    LootDistribution,
    Repair,
    ContainerPurchase,
    SpellGemPurchase,
    KeyPurchase,
    Other,
}

impl TransactionType {
    fn as_str(&self) -> &'static str {
        match self {
            TransactionType::VendorSale => "vendor_sale",
            TransactionType::VendorPurchase => "vendor_purchase",
            TransactionType::LootDrop => "loot_drop",
            TransactionType::LootPickup => "loot_pickup",
            TransactionType::BankDeposit => "bank_deposit",
            TransactionType::BankWithdrawal => "bank_withdrawal",
            TransactionType::TradeReceived => "trade_received",
            TransactionType::TradeSent => "trade_sent",
            TransactionType::LootDistribution => "loot_distribution",
            TransactionType::Repair => "repair",
            TransactionType::ContainerPurchase => "container_purchase",
            TransactionType::SpellGemPurchase => "spell_gem_purchase",
            TransactionType::KeyPurchase => "key_purchase",
            TransactionType::Other => "other",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "vendor_sale" => TransactionType::VendorSale,
            "vendor_purchase" => TransactionType::VendorPurchase,
            "loot_drop" => TransactionType::LootDrop,
            "loot_pickup" => TransactionType::LootPickup,
            "bank_deposit" => TransactionType::BankDeposit,
            "bank_withdrawal" => TransactionType::BankWithdrawal,
            "trade_received" => TransactionType::TradeReceived,
            "trade_sent" => TransactionType::TradeSent,
            "loot_distribution" => TransactionType::LootDistribution,
            "repair" => TransactionType::Repair,
            "container_purchase" => TransactionType::ContainerPurchase,
            "spell_gem_purchase" => TransactionType::SpellGemPurchase,
            "key_purchase" => TransactionType::KeyPurchase,
            _ => TransactionType::Other,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionSummary {
    #[serde(skip, default = "Instant::now")]
    pub session_start: Instant,
    pub total_gained: CoinStack,
    pub total_spent: CoinStack,
    pub net_change: CoinStack,
    pub transaction_count: u64,
    pub transactions_by_type: std::collections::HashMap<String, u64>,
}

impl SessionSummary {
    pub fn new() -> Self {
        Self {
            session_start: Instant::now(),
            total_gained: CoinStack::new(),
            total_spent: CoinStack::new(),
            net_change: CoinStack::new(),
            transaction_count: 0,
            transactions_by_type: std::collections::HashMap::new(),
        }
    }

    pub fn plat_per_hour(&self) -> f64 {
        let elapsed = self.session_start.elapsed();
        if elapsed.as_secs() == 0 {
            return 0.0;
        }
        let net_plat_copper = self.net_change.total_copper();
        let hours = elapsed.as_secs_f64() / 3600.0;
        net_plat_copper as f64 / 1000.0 / hours
    }

    pub fn session_duration(&self) -> Duration {
        self.session_start.elapsed()
    }

    pub fn format_duration(&self) -> String {
        let dur = self.session_duration();
        let h = dur.as_secs() / 3600;
        let m = (dur.as_secs() % 3600) / 60;
        let s = dur.as_secs() % 60;
        if h > 0 {
            format!("{}h {}m", h, m)
        } else if m > 0 {
            format!("{}m {}s", m, s)
        } else {
            format!("{}s", s)
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for SessionSummary {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PlatTracker {
    summary: SessionSummary,
    recent_transactions: Vec<TransactionRecord>,
    max_recent_transactions: usize,
}

impl PlatTracker {
    pub fn new() -> Self {
        Self {
            summary: SessionSummary::new(),
            recent_transactions: Vec::new(),
            max_recent_transactions: 100,
        }
    }

    pub fn new_with_capacity(max_transactions: usize) -> Self {
        Self {
            summary: SessionSummary::new(),
            recent_transactions: Vec::with_capacity(max_transactions),
            max_recent_transactions: max_transactions,
        }
    }

    pub fn record_transaction(
        &mut self,
        coin_delta: CoinStack,
        transaction_type: TransactionType,
        character_id: Option<String>,
        item_name: Option<String>,
        note: Option<String>,
    ) {
        let record = TransactionRecord {
            timestamp: Instant::now(),
            coin_delta,
            transaction_type,
            character_id,
            item_name,
            note,
        };

        if self.recent_transactions.len() >= self.max_recent_transactions {
            self.recent_transactions.remove(0);
        }
        self.recent_transactions.push(record.clone());

        let delta_copper = coin_delta.total_copper();
        if delta_copper >= 0 {
            self.summary.total_gained = self.summary.total_gained.add(&coin_delta);
        } else {
            self.summary.total_spent = self.summary.total_spent.add(&coin_delta);
        }
        self.summary.net_change = self.summary.net_change.add(&coin_delta);
        self.summary.transaction_count += 1;

        let type_key = record.transaction_type.as_str().to_string();
        *self
            .summary
            .transactions_by_type
            .entry(type_key)
            .or_insert(0) += 1;
    }

    pub fn record_from_ledger(
        &mut self,
        plat_delta: i64,
        source: &str,
        character_id: &str,
        item_name: &str,
    ) {
        let coin_delta = CoinStack::from_platinum(plat_delta);
        let transaction_type = match source {
            "vendor" => {
                if plat_delta >= 0 {
                    TransactionType::VendorSale
                } else {
                    TransactionType::VendorPurchase
                }
            }
            "bank" => {
                if plat_delta >= 0 {
                    TransactionType::BankDeposit
                } else {
                    TransactionType::BankWithdrawal
                }
            }
            "drop" => TransactionType::LootDrop,
            "distribution" => TransactionType::LootDistribution,
            _ => TransactionType::Other,
        };
        self.record_transaction(
            coin_delta,
            transaction_type,
            Some(character_id.to_string()),
            if item_name.is_empty() {
                None
            } else {
                Some(item_name.to_string())
            },
            None,
        );
    }

    pub fn summary(&self) -> &SessionSummary {
        &self.summary
    }

    pub fn recent_transactions(&self) -> &[TransactionRecord] {
        &self.recent_transactions
    }

    pub fn reset_session(&mut self) {
        self.summary.reset();
        self.recent_transactions.clear();
    }
}

impl Default for PlatTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coin_stack_total_copper() {
        let coin = CoinStack {
            plat: 1,
            gold: 2,
            silver: 3,
            copper: 4,
        };
        assert_eq!(coin.total_copper(), 1_234);
    }

    #[test]
    fn coin_stack_from_copper_roundtrip() {
        let original = CoinStack {
            plat: 123,
            gold: 45,
            silver: 6,
            copper: 7,
        };
        let copper = original.total_copper();
        let restored = CoinStack::from_copper(copper);
        assert_eq!(restored.total_copper(), copper);
        assert_eq!(
            restored,
            CoinStack {
                plat: 127,
                gold: 5,
                silver: 6,
                copper: 7,
            }
        );
    }

    #[test]
    fn coin_stack_add() {
        let a = CoinStack {
            plat: 10,
            gold: 5,
            silver: 3,
            copper: 2,
        };
        let b = CoinStack {
            plat: 5,
            gold: 10,
            silver: 20,
            copper: 30,
        };
        let sum = a.add(&b);
        assert_eq!(sum.plat, 15);
        assert_eq!(sum.gold, 15);
        assert_eq!(sum.silver, 23);
        assert_eq!(sum.copper, 32);
    }

    #[test]
    fn coin_stack_subtract() {
        let a = CoinStack {
            plat: 10,
            gold: 5,
            silver: 3,
            copper: 2,
        };
        let b = CoinStack {
            plat: 3,
            gold: 2,
            silver: 1,
            copper: 1,
        };
        let diff = a.subtract(&b);
        assert_eq!(diff.plat, 7);
        assert_eq!(diff.gold, 3);
        assert_eq!(diff.silver, 2);
        assert_eq!(diff.copper, 1);
    }

    #[test]
    fn coin_stack_format_verbose() {
        let coin = CoinStack {
            plat: 100,
            gold: 50,
            silver: 0,
            copper: 5,
        };
        assert_eq!(coin.format_verbose(), "100p 50g 5c");
    }

    #[test]
    fn coin_stack_format_zero() {
        let coin = CoinStack::new();
        assert_eq!(coin.format_verbose(), "0p");
    }

    #[test]
    fn plat_tracker_records_gains() {
        let mut tracker = PlatTracker::new();
        tracker.record_transaction(
            CoinStack::from_platinum(100),
            TransactionType::VendorSale,
            Some("Warrior1".to_string()),
            None,
            None,
        );
        assert_eq!(tracker.summary().total_gained.plat, 100);
        assert_eq!(tracker.summary().net_change.plat, 100);
        assert_eq!(tracker.summary().transaction_count, 1);
    }

    #[test]
    fn plat_tracker_records_losses() {
        let mut tracker = PlatTracker::new();
        tracker.record_transaction(
            CoinStack::from_platinum(-50),
            TransactionType::Repair,
            Some("Warrior1".to_string()),
            None,
            None,
        );
        assert_eq!(tracker.summary().total_spent.plat, -50);
        assert_eq!(tracker.summary().net_change.plat, -50);
    }

    #[test]
    fn plat_tracker_recent_transactions_limit() {
        let mut tracker = PlatTracker::new_with_capacity(5);
        for i in 0..10 {
            tracker.record_transaction(
                CoinStack::from_platinum(i),
                TransactionType::Other,
                None,
                None,
                None,
            );
        }
        assert_eq!(tracker.recent_transactions().len(), 5);
        assert_eq!(tracker.recent_transactions()[0].coin_delta.plat, 5);
        assert_eq!(tracker.recent_transactions()[4].coin_delta.plat, 9);
    }

    #[test]
    fn plat_tracker_reset() {
        let mut tracker = PlatTracker::new();
        tracker.record_transaction(
            CoinStack::from_platinum(100),
            TransactionType::VendorSale,
            None,
            None,
            None,
        );
        tracker.reset_session();
        assert_eq!(tracker.summary().transaction_count, 0);
        assert_eq!(tracker.summary().net_change.plat, 0);
        assert!(tracker.recent_transactions().is_empty());
    }

    #[test]
    fn session_summary_plat_per_hour() {
        let mut summary = SessionSummary::new();
        summary.net_change = CoinStack::from_platinum(600);
        summary.session_start = Instant::now() - Duration::from_secs(3600);
        let rate = summary.plat_per_hour();
        let expected = summary.net_change.total_copper() as f64 / 100.0;
        assert!((rate - expected).abs() < 0.1);
    }

    #[test]
    fn session_summary_plat_per_hour_zero_time() {
        let summary = SessionSummary::new();
        let rate = summary.plat_per_hour();
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn session_summary_format_duration() {
        let summary = SessionSummary::new();
        std::thread::sleep(Duration::from_millis(10));
        let formatted = summary.format_duration();
        assert!(formatted.contains("s") || formatted.contains("m"));
    }

    #[test]
    fn transaction_type_roundtrip() {
        for tt in [
            TransactionType::VendorSale,
            TransactionType::VendorPurchase,
            TransactionType::Repair,
            TransactionType::BankDeposit,
        ] {
            assert_eq!(TransactionType::from_str(tt.as_str()), tt);
        }
    }
}
