//! Unified error handling framework for TextQuest.
//!
//! This module provides structured error types and automatic recovery actions
//! covering IPC, Zone, Combat, Config, Injection, and Network error categories.
//! Each error variant includes a severity level (Fatal/Recoverable/Warning) and
//! can be mapped to automatic recovery actions (Retry, Reconnect, Rezone,
//! etc.).

use crate::types::ClientId;
use std::{
    fmt, io,
    time::{SystemTime, UNIX_EPOCH},
};

/// Severity level for an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    /// Fatal error — requires immediate intervention or process restart.
    Fatal,
    /// Recoverable error — can be handled via automatic recovery action.
    Recoverable,
    /// Warning — logged but does not require immediate action.
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fatal => write!(f, "FATAL"),
            Self::Recoverable => write!(f, "RECOVERABLE"),
            Self::Warning => write!(f, "WARNING"),
        }
    }
}

/// Automatic recovery action to attempt for an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecoveryAction {
    /// Retry the failed operation (with exponential backoff).
    Retry,
    /// Reconnect the IPC channel.
    Reconnect,
    /// Recover from a failed zone transition (request safe coordinates).
    Rezone,
    /// Restart the affected client.
    RestartClient,
    /// Alert operator (Discord, log, etc.) but continue.
    Alert,
    /// Ignore the error and continue normally.
    Ignore,
}

impl fmt::Display for RecoveryAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Retry => write!(f, "Retry"),
            Self::Reconnect => write!(f, "Reconnect"),
            Self::Rezone => write!(f, "Rezone"),
            Self::RestartClient => write!(f, "RestartClient"),
            Self::Alert => write!(f, "Alert"),
            Self::Ignore => write!(f, "Ignore"),
        }
    }
}

/// Unified error type covering all TextQuest domains.
///
/// Each variant pairs a specific error with:
/// - Severity level (Fatal/Recoverable/Warning)
/// - Suggested recovery action
/// - Optional context (client_id, zone, etc.)
#[derive(Debug, Clone)]
pub enum TextQuestError {
    // ─── IPC Errors ────────────────────────────────────────────────────
    /// Failed to send a command over the IPC channel.
    IpcSendFailed {
        client_id: Option<ClientId>,
        reason: String,
        severity: Severity,
    },
    /// Failed to receive a response on the IPC channel.
    IpcRecvFailed {
        client_id: Option<ClientId>,
        reason: String,
        severity: Severity,
    },
    /// IPC channel is not connected or has been closed.
    IpcDisconnected {
        client_id: Option<ClientId>,
        reason: String,
    },
    /// IPC message serialization/deserialization failed.
    IpcProtocolError {
        client_id: Option<ClientId>,
        reason: String,
    },
    /// IPC request timed out waiting for response.
    IpcTimeout {
        client_id: ClientId,
        command: String,
    },

    // ─── Zone Errors ──────────────────────────────────────────────────
    /// Zone transition was denied (invalid landing coordinates).
    ZoneDenied {
        client_id: ClientId,
        target_zone: String,
        invalid_pos: (f32, f32, f32),
    },
    /// Safe coordinates could not be obtained for zone recovery.
    SafeCoordsFailed {
        client_id: ClientId,
        target_zone: String,
        reason: String,
    },
    /// Zone loading timeout (player did not appear in zone).
    ZoneLoadTimeout {
        client_id: ClientId,
        target_zone: String,
    },
    /// Zone validation failed (geometry check, collision, etc.).
    ZoneValidationFailed {
        client_id: ClientId,
        zone: String,
        reason: String,
    },

    // ─── Combat Errors ────────────────────────────────────────────────
    /// Spell cast failed (no target, out of range, resisting, etc.).
    SpellCastFailed {
        client_id: ClientId,
        spell_name: String,
        reason: String,
    },
    /// Combat targeting error (target lost, dead, invalid).
    TargetingError { client_id: ClientId, reason: String },
    /// Combat rotation error (out of mana, interrupted, etc.).
    RotationError { client_id: ClientId, reason: String },
    /// Melee combat failed (unreachable, blocked, etc.).
    MeleeFailed { client_id: ClientId, reason: String },

    // ─── Config Errors ────────────────────────────────────────────────
    /// Configuration file not found or unreadable.
    ConfigLoadFailed { path: String, reason: String },
    /// Configuration parsing failed (invalid TOML, etc.).
    ConfigParseFailed { path: String, reason: String },
    /// Required configuration field is missing.
    ConfigMissing {
        field: String,
        section: Option<String>,
    },
    /// Configuration validation failed (invalid values, etc.).
    ConfigValidationFailed { section: String, reason: String },

    // ─── Injection Errors ────────────────────────────────────────────
    /// DLL injection into target process failed.
    InjectionFailed {
        target_pid: Option<u32>,
        reason: String,
    },
    /// Target process not found or invalid.
    ProcessNotFound {
        pid: Option<u32>,
        process_name: Option<String>,
    },
    /// DLL hook initialization failed.
    HookInitFailed { reason: String },
    /// Memory access violation or invalid pointer.
    MemoryAccessFailed {
        address: Option<u64>,
        reason: String,
    },

    // ─── Network Errors ───────────────────────────────────────────────
    /// Network socket operation failed.
    NetworkError { reason: String, severity: Severity },
    /// Packet send/recv failed.
    PacketError { direction: String, reason: String },

    // ─── Generic/Unknown Errors ──────────────────────────────────────
    /// Generic error not fitting other categories.
    Other { message: String, severity: Severity },
}

impl TextQuestError {
    /// Get the severity level of this error.
    pub fn severity(&self) -> Severity {
        match self {
            // IPC errors are usually recoverable
            Self::IpcSendFailed { severity, .. }
            | Self::IpcRecvFailed { severity, .. }
            | Self::NetworkError { severity, .. } => *severity,
            Self::IpcDisconnected { .. } => Severity::Recoverable,
            Self::IpcProtocolError { .. } => Severity::Recoverable,
            Self::IpcTimeout { .. } => Severity::Recoverable,

            // Zone errors are recoverable
            Self::ZoneDenied { .. } => Severity::Recoverable,
            Self::SafeCoordsFailed { .. } => Severity::Recoverable,
            Self::ZoneLoadTimeout { .. } => Severity::Recoverable,
            Self::ZoneValidationFailed { .. } => Severity::Warning,

            // Combat errors vary
            Self::SpellCastFailed { .. } => Severity::Warning,
            Self::TargetingError { .. } => Severity::Warning,
            Self::RotationError { .. } => Severity::Warning,
            Self::MeleeFailed { .. } => Severity::Warning,

            // Config errors are fatal
            Self::ConfigLoadFailed { .. } => Severity::Fatal,
            Self::ConfigParseFailed { .. } => Severity::Fatal,
            Self::ConfigMissing { .. } => Severity::Fatal,
            Self::ConfigValidationFailed { .. } => Severity::Fatal,

            // Injection errors are fatal
            Self::InjectionFailed { .. } => Severity::Fatal,
            Self::ProcessNotFound { .. } => Severity::Fatal,
            Self::HookInitFailed { .. } => Severity::Fatal,
            Self::MemoryAccessFailed { .. } => Severity::Fatal,

            // Network errors vary
            Self::PacketError { .. } => Severity::Recoverable,

            // Generic
            Self::Other { severity, .. } => *severity,
        }
    }

    /// Get the recommended recovery action for this error.
    pub fn recovery_action(&self) -> RecoveryAction {
        match self {
            // IPC errors → Reconnect
            Self::IpcSendFailed { .. } | Self::IpcRecvFailed { .. } => RecoveryAction::Reconnect,
            Self::IpcDisconnected { .. } => RecoveryAction::Reconnect,
            Self::IpcProtocolError { .. } => RecoveryAction::Reconnect,
            Self::IpcTimeout { .. } => RecoveryAction::Retry,

            // Zone errors → Rezone
            Self::ZoneDenied { .. } => RecoveryAction::Rezone,
            Self::SafeCoordsFailed { .. } => RecoveryAction::Rezone,
            Self::ZoneLoadTimeout { .. } => RecoveryAction::Retry,
            Self::ZoneValidationFailed { .. } => RecoveryAction::Alert,

            // Combat errors → Alert
            Self::SpellCastFailed { .. } => RecoveryAction::Alert,
            Self::TargetingError { .. } => RecoveryAction::Alert,
            Self::RotationError { .. } => RecoveryAction::Alert,
            Self::MeleeFailed { .. } => RecoveryAction::Alert,

            // Config errors → Alert
            Self::ConfigLoadFailed { .. }
            | Self::ConfigParseFailed { .. }
            | Self::ConfigMissing { .. }
            | Self::ConfigValidationFailed { .. } => RecoveryAction::Alert,

            // Injection errors → RestartClient
            Self::InjectionFailed { .. } => RecoveryAction::RestartClient,
            Self::ProcessNotFound { .. } => RecoveryAction::RestartClient,
            Self::HookInitFailed { .. } => RecoveryAction::RestartClient,
            Self::MemoryAccessFailed { .. } => RecoveryAction::RestartClient,

            // Network errors → Reconnect
            Self::NetworkError { .. } => RecoveryAction::Reconnect,
            Self::PacketError { .. } => RecoveryAction::Retry,

            // Generic → Ignore by default
            Self::Other { .. } => RecoveryAction::Ignore,
        }
    }

    /// Get the client_id associated with this error (if any).
    pub fn client_id(&self) -> Option<ClientId> {
        match self {
            Self::IpcSendFailed { client_id, .. }
            | Self::IpcRecvFailed { client_id, .. }
            | Self::IpcDisconnected { client_id, .. }
            | Self::IpcProtocolError { client_id, .. } => *client_id,
            Self::IpcTimeout { client_id, .. }
            | Self::ZoneDenied { client_id, .. }
            | Self::SafeCoordsFailed { client_id, .. }
            | Self::ZoneLoadTimeout { client_id, .. }
            | Self::ZoneValidationFailed { client_id, .. }
            | Self::SpellCastFailed { client_id, .. }
            | Self::TargetingError { client_id, .. }
            | Self::RotationError { client_id, .. }
            | Self::MeleeFailed { client_id, .. } => Some(*client_id),
            Self::ConfigLoadFailed { .. }
            | Self::ConfigParseFailed { .. }
            | Self::ConfigMissing { .. }
            | Self::ConfigValidationFailed { .. }
            | Self::InjectionFailed { .. }
            | Self::ProcessNotFound { .. }
            | Self::HookInitFailed { .. }
            | Self::MemoryAccessFailed { .. }
            | Self::NetworkError { .. }
            | Self::PacketError { .. }
            | Self::Other { .. } => None,
        }
    }
}

impl fmt::Display for TextQuestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IpcSendFailed {
                client_id, reason, ..
            } => {
                write!(
                    f,
                    "IPC send failed{}: {}",
                    client_id
                        .map(|cid| format!(" (client {})", cid))
                        .unwrap_or_default(),
                    reason
                )
            }
            Self::IpcRecvFailed {
                client_id, reason, ..
            } => {
                write!(
                    f,
                    "IPC recv failed{}: {}",
                    client_id
                        .map(|cid| format!(" (client {})", cid))
                        .unwrap_or_default(),
                    reason
                )
            }
            Self::IpcDisconnected { client_id, reason } => {
                write!(
                    f,
                    "IPC disconnected{}: {}",
                    client_id
                        .map(|cid| format!(" (client {})", cid))
                        .unwrap_or_default(),
                    reason
                )
            }
            Self::IpcProtocolError { client_id, reason } => {
                write!(
                    f,
                    "IPC protocol error{}: {}",
                    client_id
                        .map(|cid| format!(" (client {})", cid))
                        .unwrap_or_default(),
                    reason
                )
            }
            Self::IpcTimeout { client_id, command } => {
                write!(
                    f,
                    "IPC timeout (client {}, command: {})",
                    client_id, command
                )
            }
            Self::ZoneDenied {
                client_id,
                target_zone,
                invalid_pos,
            } => {
                write!(
                    f,
                    "Zone denied for client {}: {} at {:?}",
                    client_id, target_zone, invalid_pos
                )
            }
            Self::SafeCoordsFailed {
                client_id,
                target_zone,
                reason,
            } => {
                write!(
                    f,
                    "Safe coords failed for client {} (zone {}): {}",
                    client_id, target_zone, reason
                )
            }
            Self::ZoneLoadTimeout {
                client_id,
                target_zone,
            } => {
                write!(
                    f,
                    "Zone load timeout for client {} (zone {})",
                    client_id, target_zone
                )
            }
            Self::ZoneValidationFailed {
                client_id,
                zone,
                reason,
            } => {
                write!(
                    f,
                    "Zone validation failed for client {} (zone {}): {}",
                    client_id, zone, reason
                )
            }
            Self::SpellCastFailed {
                client_id,
                spell_name,
                reason,
            } => {
                write!(
                    f,
                    "Spell cast failed for client {} ({}): {}",
                    client_id, spell_name, reason
                )
            }
            Self::TargetingError { client_id, reason } => {
                write!(f, "Targeting error for client {}: {}", client_id, reason)
            }
            Self::RotationError { client_id, reason } => {
                write!(f, "Rotation error for client {}: {}", client_id, reason)
            }
            Self::MeleeFailed { client_id, reason } => {
                write!(f, "Melee failed for client {}: {}", client_id, reason)
            }
            Self::ConfigLoadFailed { path, reason } => {
                write!(f, "Config load failed ({}): {}", path, reason)
            }
            Self::ConfigParseFailed { path, reason } => {
                write!(f, "Config parse failed ({}): {}", path, reason)
            }
            Self::ConfigMissing { field, section } => {
                write!(
                    f,
                    "Config missing{}: {}",
                    section
                        .as_ref()
                        .map(|s| format!(" [{}]", s))
                        .unwrap_or_default(),
                    field
                )
            }
            Self::ConfigValidationFailed { section, reason } => {
                write!(f, "Config validation failed [{}]: {}", section, reason)
            }
            Self::InjectionFailed { target_pid, reason } => {
                write!(
                    f,
                    "DLL injection failed{}: {}",
                    target_pid
                        .map(|pid| format!(" (pid {})", pid))
                        .unwrap_or_default(),
                    reason
                )
            }
            Self::ProcessNotFound { pid, process_name } => {
                write!(
                    f,
                    "Process not found{}{}",
                    pid.map(|p| format!(" (pid {})", p)).unwrap_or_default(),
                    process_name
                        .as_ref()
                        .map(|n| format!(" ({})", n))
                        .unwrap_or_default()
                )
            }
            Self::HookInitFailed { reason } => {
                write!(f, "Hook initialization failed: {}", reason)
            }
            Self::MemoryAccessFailed { address, reason } => {
                write!(
                    f,
                    "Memory access failed{}: {}",
                    address.map(|a| format!(" (0x{:X})", a)).unwrap_or_default(),
                    reason
                )
            }
            Self::NetworkError { reason, .. } => {
                write!(f, "Network error: {}", reason)
            }
            Self::PacketError { direction, reason } => {
                write!(f, "Packet error ({}): {}", direction, reason)
            }
            Self::Other { message, .. } => {
                write!(f, "Error: {}", message)
            }
        }
    }
}

impl std::error::Error for TextQuestError {}

// ─── Conversion implementations ────────────────────────────────────────

/// Convert from std::io::Error to TextQuestError.
impl From<io::Error> for TextQuestError {
    fn from(err: io::Error) -> Self {
        let reason = err.to_string();
        match err.kind() {
            io::ErrorKind::NotFound => Self::ConfigLoadFailed {
                path: "<unknown>".to_string(),
                reason,
            },
            io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted => Self::IpcDisconnected {
                client_id: None,
                reason,
            },
            io::ErrorKind::TimedOut => Self::IpcTimeout {
                client_id: 0,
                command: "<unknown>".to_string(),
            },
            _ => Self::NetworkError {
                reason,
                severity: Severity::Recoverable,
            },
        }
    }
}

/// Convert from serde_json::Error to TextQuestError.
impl From<serde_json::Error> for TextQuestError {
    fn from(err: serde_json::Error) -> Self {
        Self::IpcProtocolError {
            client_id: None,
            reason: err.to_string(),
        }
    }
}

/// Error context with timestamp, client_id, zone, and stack information.
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// UNIX timestamp when the error occurred (milliseconds).
    pub timestamp_ms: u64,
    /// Client ID if this error is client-specific.
    pub client_id: Option<ClientId>,
    /// Zone name if this error is zone-specific.
    pub zone: Option<String>,
    /// Stack frame context (for debugging).
    pub stack_context: Option<String>,
}

impl ErrorContext {
    /// Create a new error context with the current timestamp.
    pub fn now() -> Self {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self {
            timestamp_ms,
            client_id: None,
            zone: None,
            stack_context: None,
        }
    }

    /// Set the client_id for this context.
    pub fn with_client(mut self, client_id: ClientId) -> Self {
        self.client_id = Some(client_id);
        self
    }

    /// Set the zone for this context.
    pub fn with_zone(mut self, zone: String) -> Self {
        self.zone = Some(zone);
        self
    }

    /// Set the stack context for this context.
    pub fn with_stack_context(mut self, context: String) -> Self {
        self.stack_context = Some(context);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Severity tests ───────────────────────────────────────────────

    #[test]
    fn severity_display() {
        assert_eq!(Severity::Fatal.to_string(), "FATAL");
        assert_eq!(Severity::Recoverable.to_string(), "RECOVERABLE");
        assert_eq!(Severity::Warning.to_string(), "WARNING");
    }

    // ─── RecoveryAction tests ──────────────────────────────────────────

    #[test]
    fn recovery_action_display() {
        assert_eq!(RecoveryAction::Retry.to_string(), "Retry");
        assert_eq!(RecoveryAction::Reconnect.to_string(), "Reconnect");
        assert_eq!(RecoveryAction::Rezone.to_string(), "Rezone");
        assert_eq!(RecoveryAction::RestartClient.to_string(), "RestartClient");
        assert_eq!(RecoveryAction::Alert.to_string(), "Alert");
        assert_eq!(RecoveryAction::Ignore.to_string(), "Ignore");
    }

    // ─── TextQuestError severity tests ────────────────────────────────

    #[test]
    fn error_severity_ipc_errors() {
        let err = TextQuestError::IpcSendFailed {
            client_id: None,
            reason: "test".to_string(),
            severity: Severity::Recoverable,
        };
        assert_eq!(err.severity(), Severity::Recoverable);

        let err = TextQuestError::IpcDisconnected {
            client_id: None,
            reason: "test".to_string(),
        };
        assert_eq!(err.severity(), Severity::Recoverable);
    }

    #[test]
    fn error_severity_zone_errors() {
        let err = TextQuestError::ZoneDenied {
            client_id: 1,
            target_zone: "qey2hh1".to_string(),
            invalid_pos: (0.0, 0.0, 0.0),
        };
        assert_eq!(err.severity(), Severity::Recoverable);

        let err = TextQuestError::ZoneLoadTimeout {
            client_id: 1,
            target_zone: "qey2hh1".to_string(),
        };
        assert_eq!(err.severity(), Severity::Recoverable);
    }

    #[test]
    fn error_severity_combat_errors() {
        let err = TextQuestError::SpellCastFailed {
            client_id: 1,
            spell_name: "fireball".to_string(),
            reason: "no target".to_string(),
        };
        assert_eq!(err.severity(), Severity::Warning);
    }

    #[test]
    fn error_severity_config_errors() {
        let err = TextQuestError::ConfigLoadFailed {
            path: "config.toml".to_string(),
            reason: "not found".to_string(),
        };
        assert_eq!(err.severity(), Severity::Fatal);
    }

    #[test]
    fn error_severity_injection_errors() {
        let err = TextQuestError::InjectionFailed {
            target_pid: Some(1234),
            reason: "access denied".to_string(),
        };
        assert_eq!(err.severity(), Severity::Fatal);
    }

    // ─── Recovery action mapping tests ────────────────────────────────

    #[test]
    fn error_recovery_ipc_errors() {
        let err = TextQuestError::IpcSendFailed {
            client_id: None,
            reason: "test".to_string(),
            severity: Severity::Recoverable,
        };
        assert_eq!(err.recovery_action(), RecoveryAction::Reconnect);

        let err = TextQuestError::IpcTimeout {
            client_id: 1,
            command: "MoveTo".to_string(),
        };
        assert_eq!(err.recovery_action(), RecoveryAction::Retry);
    }

    #[test]
    fn error_recovery_zone_errors() {
        let err = TextQuestError::ZoneDenied {
            client_id: 1,
            target_zone: "qey2hh1".to_string(),
            invalid_pos: (99_999.0, 0.0, 0.0),
        };
        assert_eq!(err.recovery_action(), RecoveryAction::Rezone);

        let err = TextQuestError::SafeCoordsFailed {
            client_id: 1,
            target_zone: "nektulos".to_string(),
            reason: "DLL error".to_string(),
        };
        assert_eq!(err.recovery_action(), RecoveryAction::Rezone);
    }

    #[test]
    fn error_recovery_combat_errors() {
        let err = TextQuestError::SpellCastFailed {
            client_id: 1,
            spell_name: "fireball".to_string(),
            reason: "no target".to_string(),
        };
        assert_eq!(err.recovery_action(), RecoveryAction::Alert);
    }

    #[test]
    fn error_recovery_config_errors() {
        let err = TextQuestError::ConfigLoadFailed {
            path: "config.toml".to_string(),
            reason: "not found".to_string(),
        };
        assert_eq!(err.recovery_action(), RecoveryAction::Alert);
    }

    #[test]
    fn error_recovery_injection_errors() {
        let err = TextQuestError::InjectionFailed {
            target_pid: Some(1234),
            reason: "access denied".to_string(),
        };
        assert_eq!(err.recovery_action(), RecoveryAction::RestartClient);
    }

    #[test]
    fn error_recovery_network_errors() {
        let err = TextQuestError::NetworkError {
            reason: "no route to host".to_string(),
            severity: Severity::Recoverable,
        };
        assert_eq!(err.recovery_action(), RecoveryAction::Reconnect);
    }

    // ─── Client ID extraction tests ────────────────────────────────────

    #[test]
    fn error_client_id_extraction() {
        let err = TextQuestError::IpcSendFailed {
            client_id: Some(42),
            reason: "test".to_string(),
            severity: Severity::Recoverable,
        };
        assert_eq!(err.client_id(), Some(42));

        let err = TextQuestError::ZoneDenied {
            client_id: 5,
            target_zone: "gfay".to_string(),
            invalid_pos: (0.0, 0.0, 0.0),
        };
        assert_eq!(err.client_id(), Some(5));

        let err = TextQuestError::ConfigLoadFailed {
            path: "config.toml".to_string(),
            reason: "test".to_string(),
        };
        assert_eq!(err.client_id(), None);
    }

    // ─── Display and string representation tests ──────────────────────

    #[test]
    fn error_display_ipc_error() {
        let err = TextQuestError::IpcSendFailed {
            client_id: Some(1),
            reason: "broken pipe".to_string(),
            severity: Severity::Recoverable,
        };
        let display = err.to_string();
        assert!(display.contains("IPC send failed"));
        assert!(display.contains("client 1"));
        assert!(display.contains("broken pipe"));
    }

    #[test]
    fn error_display_zone_denied() {
        let err = TextQuestError::ZoneDenied {
            client_id: 2,
            target_zone: "nektulos".to_string(),
            invalid_pos: (100.0, 200.0, 0.0),
        };
        let display = err.to_string();
        assert!(display.contains("Zone denied"));
        assert!(display.contains("client 2"));
        assert!(display.contains("nektulos"));
    }

    #[test]
    fn error_display_config_error() {
        let err = TextQuestError::ConfigLoadFailed {
            path: "/etc/config.toml".to_string(),
            reason: "permission denied".to_string(),
        };
        let display = err.to_string();
        assert!(display.contains("Config load failed"));
        assert!(display.contains("/etc/config.toml"));
    }

    #[test]
    fn error_display_injection_error() {
        let err = TextQuestError::InjectionFailed {
            target_pid: Some(9999),
            reason: "access denied".to_string(),
        };
        let display = err.to_string();
        assert!(display.contains("DLL injection failed"));
        assert!(display.contains("9999"));
    }

    // ─── From<io::Error> conversion tests ──────────────────────────────

    #[test]
    fn from_io_error_not_found() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err: TextQuestError = io_err.into();
        match err {
            TextQuestError::ConfigLoadFailed { .. } => (),
            _ => panic!("expected ConfigLoadFailed"),
        }
    }

    #[test]
    fn from_io_error_connection_refused() {
        let io_err = io::Error::new(io::ErrorKind::ConnectionRefused, "connection refused");
        let err: TextQuestError = io_err.into();
        match err {
            TextQuestError::IpcDisconnected { .. } => (),
            _ => panic!("expected IpcDisconnected"),
        }
    }

    #[test]
    fn from_io_error_timeout() {
        let io_err = io::Error::new(io::ErrorKind::TimedOut, "timed out");
        let err: TextQuestError = io_err.into();
        match err {
            TextQuestError::IpcTimeout { .. } => (),
            _ => panic!("expected IpcTimeout"),
        }
    }

    // ─── From<serde_json::Error> conversion tests ─────────────────────

    #[test]
    fn from_serde_json_error() {
        let json_err = serde_json::from_str::<i32>("invalid json").unwrap_err();
        let err: TextQuestError = json_err.into();
        match err {
            TextQuestError::IpcProtocolError { .. } => (),
            _ => panic!("expected IpcProtocolError"),
        }
    }

    // ─── ErrorContext tests ────────────────────────────────────────────

    #[test]
    fn error_context_now() {
        let ctx = ErrorContext::now();
        assert!(ctx.timestamp_ms > 0);
        assert_eq!(ctx.client_id, None);
        assert_eq!(ctx.zone, None);
        assert_eq!(ctx.stack_context, None);
    }

    #[test]
    fn error_context_with_client() {
        let ctx = ErrorContext::now().with_client(42);
        assert_eq!(ctx.client_id, Some(42));
    }

    #[test]
    fn error_context_with_zone() {
        let ctx = ErrorContext::now().with_zone("gfay".to_string());
        assert_eq!(ctx.zone, Some("gfay".to_string()));
    }

    #[test]
    fn error_context_builder_chain() {
        let ctx = ErrorContext::now()
            .with_client(5)
            .with_zone("nektulos".to_string())
            .with_stack_context("location.rs:123".to_string());
        assert_eq!(ctx.client_id, Some(5));
        assert_eq!(ctx.zone, Some("nektulos".to_string()));
        assert_eq!(ctx.stack_context, Some("location.rs:123".to_string()));
        assert!(ctx.timestamp_ms > 0);
    }
}
