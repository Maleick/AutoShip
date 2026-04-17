//! Chat message timestamp configuration (MQ2Timestamp parity).
//!
//! Stores the per-client timestamp format and enabled flag.  Changes take
//! effect immediately — the chat hook reads the current state when formatting
//! each message.

use once_cell::sync::Lazy;
use std::sync::RwLock;
use textquest_common::chat::TimestampFormat;

static TIMESTAMP_CONFIG: Lazy<RwLock<TimestampState>> =
    Lazy::new(|| RwLock::new(TimestampState::default()));

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TimestampState {
    pub enabled: bool,
    pub format: TimestampFormat,
}

pub fn get() -> TimestampState {
    TIMESTAMP_CONFIG
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

pub fn set(enabled: bool, format: TimestampFormat) {
    if let Ok(mut guard) = TIMESTAMP_CONFIG.write() {
        guard.enabled = enabled;
        guard.format = format;
    }
}

pub fn apply(enabled: bool, format: TimestampFormat) {
    set(enabled, format);
    tracing::info!(
        enabled,
        format = ?format,
        "Chat timestamp config updated"
    );
}
