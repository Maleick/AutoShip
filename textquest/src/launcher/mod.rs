//! Login automation — per-client FSM, staggered launch, process spawner.

/// Exponential backoff policy and per-account failure tracker.
pub mod backoff;
/// Launch coordinator — staggered multi-client launching with mass failure
/// detection.
pub mod coordinator;
/// Login state machine — tracks each client through the login flow phases.
pub mod login_sm;
/// Logout sequencer — leave group, issue /quit, and wait for process exit.
pub mod logout_sequencer;
/// Post-login sequencer — character select, server select, enter world.
pub mod post_login;
/// EQ process spawner — launches `eqgame.exe` with the correct arguments.
pub mod spawner;
