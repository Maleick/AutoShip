//! Multi-client management — sessions, self-healing monitor, CPU affinity.

/// CPU affinity pinning for EQ client processes.
pub mod affinity;
/// Optional UDP multicast peer discovery for orchestrator instances.
pub mod discovery;
/// Death-triggered auto-camp/relog scheduler.
pub mod death_camp;
/// Self-healing monitor for detecting and recovering crashed clients.
pub mod healing;
/// Client lifecycle manager — attach, detach, reconnect.
pub mod manager;
/// Per-client session state and data.
pub mod session;
