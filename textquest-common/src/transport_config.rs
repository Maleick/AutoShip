//! Unified transport configuration for cross-client coordination.
//!
//! # MQ2NetMQ deferral note
//!
//! MQ2NetMQ (ZeroMQ) — no live operator demand as of 2026-04. The
//! `NetMqDeferred` variant is a stub; the runtime falls back to `Native`.

use serde::{Deserialize, Serialize};

/// Which cross-client transport the operator has selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransportVariant {
    /// TextQuest's native JSON-line relay (default). No external server needed.
    #[default]
    Native,
    /// EQBC-compatible TCP relay. Enables `/bc`, `/bca`, `/bct` interop with
    /// MQ2EQBC clients.
    Eqbc,
    /// DanNet peer-to-peer transport. Required for rgmercs compatibility.
    DanNet,
    /// MQ2NetMQ (ZeroMQ) — deferred stub; runtime treats as `Native`.
    NetMqDeferred,
}

impl TransportVariant {
    /// Returns `true` when the variant has a full runtime implementation.
    #[must_use]
    pub fn is_implemented(self) -> bool {
        matches!(self, Self::Native | Self::Eqbc | Self::DanNet)
    }

    /// Resolves unimplemented stubs to `Native`.
    #[must_use]
    pub fn effective(self) -> Self {
        if self.is_implemented() {
            self
        } else {
            Self::Native
        }
    }
}

/// Operator-level configuration for the chosen cross-client transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TransportConfig {
    /// Which transport to use.
    pub variant: TransportVariant,
    /// Remote host (EQBC server or DanNet bootstrap peer). Ignored for Native.
    pub host: String,
    /// TCP port for EQBC or DanNet; listening port for the Native relay.
    pub port: u16,
    /// Connect automatically when the orchestrator starts.
    pub auto_connect: bool,
    /// Peer node name advertised to remotes. Empty = use system hostname.
    pub node_name: String,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            variant: TransportVariant::Native,
            host: "127.0.0.1".to_string(),
            port: 2112,
            auto_connect: false,
            node_name: String::new(),
        }
    }
}

impl TransportConfig {
    /// Validate the config before applying it at runtime.
    ///
    /// # Errors
    ///
    /// Returns an error when mandatory fields are missing for the selected
    /// variant, or the port is zero.
    pub fn validate(&self) -> anyhow::Result<()> {
        use anyhow::bail;
        if self.port == 0 {
            bail!("transport port must be non-zero");
        }
        match self.variant {
            TransportVariant::Eqbc if self.host.trim().is_empty() => {
                bail!("EQBC transport requires a non-empty host");
            }
            TransportVariant::DanNet if self.host.trim().is_empty() => {
                bail!("DanNet transport requires a non-empty bootstrap host");
            }
            _ => {}
        }
        Ok(())
    }

    /// Effective variant: resolves unimplemented stubs to `Native`.
    #[must_use]
    pub fn effective_variant(&self) -> TransportVariant {
        self.variant.effective()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_native_port_2112() {
        let cfg = TransportConfig::default();
        assert_eq!(cfg.variant, TransportVariant::Native);
        assert_eq!(cfg.port, 2112);
        assert!(!cfg.auto_connect);
    }

    #[test]
    fn implemented_variants() {
        assert!(TransportVariant::Native.is_implemented());
        assert!(TransportVariant::Eqbc.is_implemented());
        assert!(TransportVariant::DanNet.is_implemented());
        assert!(!TransportVariant::NetMqDeferred.is_implemented());
    }

    #[test]
    fn netmq_deferred_effective_is_native() {
        assert_eq!(
            TransportVariant::NetMqDeferred.effective(),
            TransportVariant::Native
        );
    }

    #[test]
    fn implemented_effective_returns_self() {
        assert_eq!(
            TransportVariant::Native.effective(),
            TransportVariant::Native
        );
        assert_eq!(TransportVariant::Eqbc.effective(), TransportVariant::Eqbc);
        assert_eq!(
            TransportVariant::DanNet.effective(),
            TransportVariant::DanNet
        );
    }

    #[test]
    fn validate_rejects_zero_port() {
        let cfg = TransportConfig {
            port: 0,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_rejects_eqbc_empty_host() {
        let cfg = TransportConfig {
            variant: TransportVariant::Eqbc,
            host: "   ".to_string(),
            port: 2112,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_rejects_dannet_empty_host() {
        let cfg = TransportConfig {
            variant: TransportVariant::DanNet,
            host: "   ".to_string(),
            port: 2112,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_passes_native_default() {
        assert!(TransportConfig::default().validate().is_ok());
    }

    #[test]
    fn validate_passes_eqbc_with_host() {
        let cfg = TransportConfig {
            variant: TransportVariant::Eqbc,
            host: "192.168.1.10".to_string(),
            port: 2112,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_passes_dannet_with_host() {
        let cfg = TransportConfig {
            variant: TransportVariant::DanNet,
            host: "raid-host.local".to_string(),
            port: 2114,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_passes_netmq_deferred_no_host() {
        let cfg = TransportConfig {
            variant: TransportVariant::NetMqDeferred,
            host: String::new(),
            port: 5555,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn effective_variant_resolves_deferred() {
        let cfg = TransportConfig {
            variant: TransportVariant::NetMqDeferred,
            port: 5555,
            ..Default::default()
        };
        assert_eq!(cfg.effective_variant(), TransportVariant::Native);
    }

    #[test]
    fn roundtrip_json() {
        let cfg = TransportConfig {
            variant: TransportVariant::DanNet,
            host: "my-node.local".to_string(),
            port: 2116,
            auto_connect: true,
            node_name: "RaidHost".to_string(),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: TransportConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, decoded);
    }
}
