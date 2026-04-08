//! Fleet metrics — SQLite-backed storage for events, DPS, loot, lockouts, and plat.
//!
//! Single DB file with separate tables. Foundation for session monitor and
//! Discord webhook consumers.

pub mod events;
mod store;

pub use events::{FleetEvent, FleetEventLog};
pub use store::MetricsStore;
