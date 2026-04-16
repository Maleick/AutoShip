//! Fleet metrics — SQLite-backed storage for events, DPS, loot, lockouts, and
//! plat.
//!
//! Single DB file with separate tables. Foundation for session monitor and
//! Discord webhook consumers.

pub mod events;
pub mod kill_reporter;
pub mod kill_session_store;
pub mod kill_tracker;
mod store;
pub mod xp_tracker;

pub use events::{FleetEvent, FleetEventLog};
pub use kill_reporter::KillReporter;
pub use kill_session_store::KillSessionStore;
pub use kill_tracker::{ClientDpsStats, EfficiencyScore, KillRecord, KillTracker, MobStats};
pub use store::MetricsStore;
