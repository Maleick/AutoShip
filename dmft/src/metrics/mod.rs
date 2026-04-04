//! Fleet metrics — SQLite-backed storage for events, DPS, loot, lockouts, and plat.
//!
//! Single DB file with separate tables. Foundation for session monitor and
//! Discord webhook consumers.

mod store;

pub use store::MetricsStore;
