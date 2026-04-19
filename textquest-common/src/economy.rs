//! Economy ledger schema, transaction logging, and trend metrics.
//!
//! Provides a structured transaction log for tracking income/expenses across
//! the farming operation, with filtering and trend analysis capabilities.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Transaction type for ledger entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransactionType {
    /// Coins earned from vendor sales.
    VendorSale,
    /// Coins earned from loot drops.
    LootDrop,
    /// Coins spent on supplies or repairs.
    Expense,
    /// Group banker deposits.
    Deposit,
    /// Group banker withdrawals.
    Withdrawal,
    /// Zone-wide shared pool transfers.
    PoolTransfer,
}

impl TransactionType {
    /// Human-readable name for this transaction type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VendorSale => "vendor_sale",
            Self::LootDrop => "loot_drop",
            Self::Expense => "expense",
            Self::Deposit => "deposit",
            Self::Withdrawal => "withdrawal",
            Self::PoolTransfer => "pool_transfer",
        }
    }
}

impl std::fmt::Display for TransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single ledger entry tracking an economy transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// UNIX timestamp (seconds) when transaction occurred.
    pub timestamp_secs: u64,
    /// Type of transaction.
    pub transaction_type: TransactionType,
    /// Item name involved (e.g. "Worn Ring" or "Raw Silk"), empty if N/A.
    pub item_name: String,
    /// Platinum delta: positive for income, negative for expense.
    pub delta_plat: i64,
    /// Reason or description of the transaction.
    pub reason: String,
}

impl LedgerEntry {
    /// Create a new ledger entry.
    pub fn new(
        timestamp_secs: u64,
        transaction_type: TransactionType,
        item_name: impl Into<String>,
        delta_plat: i64,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            timestamp_secs,
            transaction_type,
            item_name: item_name.into(),
            delta_plat,
            reason: reason.into(),
        }
    }
}

/// In-memory economy ledger with append and query capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EconomyLedger {
    /// All ledger entries, stored in chronological order.
    entries: Vec<LedgerEntry>,
}

impl EconomyLedger {
    /// Create a new empty ledger.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Append a ledger entry.
    pub fn append(&mut self, entry: LedgerEntry) {
        self.entries.push(entry);
    }

    /// Get all entries (for testing and debugging).
    pub fn all_entries(&self) -> &[LedgerEntry] {
        &self.entries
    }

    /// Query ledger entries within a time range (inclusive).
    ///
    /// # Arguments
    /// * `start_secs` — minimum timestamp (inclusive)
    /// * `end_secs` — maximum timestamp (inclusive)
    ///
    /// # Returns
    /// Vector of matching entries.
    pub fn query_by_time_range(&self, start_secs: u64, end_secs: u64) -> Vec<LedgerEntry> {
        self.entries
            .iter()
            .filter(|e| e.timestamp_secs >= start_secs && e.timestamp_secs <= end_secs)
            .cloned()
            .collect()
    }

    /// Query ledger entries by transaction type.
    ///
    /// # Arguments
    /// * `tx_type` — filter by this transaction type
    ///
    /// # Returns
    /// Vector of matching entries.
    pub fn query_by_type(&self, tx_type: TransactionType) -> Vec<LedgerEntry> {
        self.entries
            .iter()
            .filter(|e| e.transaction_type == tx_type)
            .cloned()
            .collect()
    }

    /// Query ledger entries by both time range and transaction type.
    ///
    /// # Arguments
    /// * `start_secs` — minimum timestamp (inclusive)
    /// * `end_secs` — maximum timestamp (inclusive)
    /// * `tx_type` — filter by this transaction type
    ///
    /// # Returns
    /// Vector of matching entries.
    pub fn query_by_time_and_type(
        &self,
        start_secs: u64,
        end_secs: u64,
        tx_type: TransactionType,
    ) -> Vec<LedgerEntry> {
        self.entries
            .iter()
            .filter(|e| {
                e.timestamp_secs >= start_secs
                    && e.timestamp_secs <= end_secs
                    && e.transaction_type == tx_type
            })
            .cloned()
            .collect()
    }

    /// Calculate rolling profit over a time range (sum of all deltas).
    ///
    /// # Arguments
    /// * `start_secs` — minimum timestamp (inclusive)
    /// * `end_secs` — maximum timestamp (inclusive)
    ///
    /// # Returns
    /// Total platinum gained or lost over the period.
    pub fn rolling_profit(&self, start_secs: u64, end_secs: u64) -> i64 {
        self.query_by_time_range(start_secs, end_secs)
            .iter()
            .map(|e| e.delta_plat)
            .sum()
    }

    /// Calculate item distribution summary: count of distinct items and
    /// frequency.
    ///
    /// # Returns
    /// HashMap of item_name -> count of times it appeared in ledger.
    pub fn item_distribution_summary(&self) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for entry in &self.entries {
            if !entry.item_name.is_empty() {
                *counts.entry(entry.item_name.clone()).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Calculate fairness metric: entropy of item distribution (0.0 = all items
    /// the same, 1.0 = perfectly distributed).
    ///
    /// # Returns
    /// Entropy value clamped to [0.0, 1.0], or None if ledger is empty.
    pub fn distribution_fairness(&self) -> Option<f64> {
        let distribution = self.item_distribution_summary();
        if distribution.is_empty() {
            return None;
        }

        let total = self
            .entries
            .iter()
            .filter(|e| !e.item_name.is_empty())
            .count();
        if total == 0 {
            return None;
        }

        let max_entropy = (distribution.len() as f64).ln();
        if max_entropy == 0.0 {
            return Some(0.0);
        }

        let entropy = distribution
            .values()
            .map(|&count| {
                let p = count as f64 / total as f64;
                if p > 0.0 { -p * p.ln() } else { 0.0 }
            })
            .sum::<f64>();

        Some((entropy / max_entropy).clamp(0.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ledger() -> EconomyLedger {
        let mut ledger = EconomyLedger::new();

        // Day 1: 1000 seconds in (epoch-based)
        ledger.append(LedgerEntry::new(
            1000,
            TransactionType::VendorSale,
            "Worn Ring",
            150,
            "Sold to merchant",
        ));

        ledger.append(LedgerEntry::new(
            1500,
            TransactionType::LootDrop,
            "Raw Silk",
            100,
            "Loot from giant spider",
        ));

        // Day 2: 100000 seconds in
        ledger.append(LedgerEntry::new(
            100000,
            TransactionType::VendorSale,
            "Worn Ring",
            150,
            "Sold to merchant",
        ));

        ledger.append(LedgerEntry::new(
            100500,
            TransactionType::Expense,
            "",
            -50,
            "Paid for spell reagents",
        ));

        // Day 3: 200000 seconds in
        ledger.append(LedgerEntry::new(
            200000,
            TransactionType::LootDrop,
            "Raw Silk",
            100,
            "Loot from giant spider",
        ));

        ledger.append(LedgerEntry::new(
            200100,
            TransactionType::Deposit,
            "",
            250,
            "Group banker deposit",
        ));

        ledger
    }

    #[test]
    fn test_append_and_all_entries() {
        let ledger = sample_ledger();
        assert_eq!(ledger.all_entries().len(), 6);
    }

    #[test]
    fn test_query_by_time_range() {
        let ledger = sample_ledger();
        let range_entries = ledger.query_by_time_range(1000, 2000);
        assert_eq!(range_entries.len(), 2);
        assert!(range_entries[0].timestamp_secs >= 1000);
        assert!(range_entries[1].timestamp_secs <= 2000);
    }

    #[test]
    fn test_query_by_type() {
        let ledger = sample_ledger();
        let vendor_sales = ledger.query_by_type(TransactionType::VendorSale);
        assert_eq!(vendor_sales.len(), 2);
        assert!(
            vendor_sales
                .iter()
                .all(|e| e.transaction_type == TransactionType::VendorSale)
        );
    }

    #[test]
    fn test_rolling_profit() {
        let ledger = sample_ledger();

        // All entries
        let total_profit = ledger.rolling_profit(0, u64::MAX);
        assert_eq!(total_profit, 150 + 100 + 150 - 50 + 100 + 250); // = 700

        // Time range: first two entries
        let early_profit = ledger.rolling_profit(1000, 2000);
        assert_eq!(early_profit, 150 + 100); // = 250
    }

    #[test]
    fn test_item_distribution_summary() {
        let ledger = sample_ledger();
        let distribution = ledger.item_distribution_summary();

        assert_eq!(distribution.get("Worn Ring"), Some(&2));
        assert_eq!(distribution.get("Raw Silk"), Some(&2));
        assert!(!distribution.contains_key(""));
    }

    #[test]
    fn test_distribution_fairness() {
        let ledger = sample_ledger();
        let fairness = ledger.distribution_fairness();

        assert!(fairness.is_some());
        let f = fairness.unwrap();
        assert!((0.0..=1.0).contains(&f));
        // With 2 items appearing 2 times each, entropy should be high (fair)
        assert!(f > 0.5);
    }

    #[test]
    fn test_distribution_fairness_empty_ledger() {
        let ledger = EconomyLedger::new();
        assert!(ledger.distribution_fairness().is_none());
    }
}
