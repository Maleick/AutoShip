//! Transport selection and endpoint configuration.

use serde::{Deserialize, Serialize};

/// Which primary transport this TextQuest instance uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    /// Legacy MQ2EQBC-compatible TCP relay (default, widest compatibility).
    #[default]
    Eqbc,
    /// Peer-to-peer DanNet-compatible transport (required by rgmercs).
    DanNet,
    /// Run both EQBC and DanNet concurrently.
    Both,
}

/// EQBC server/client configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EqbcConfig {
    /// Act as an EQBC server (hub) in addition to a client.
    pub serve: bool,
    /// Remote EQBC server host to connect to.
    pub host: String,
    /// EQBC server port.
    pub port: u16,
    /// Automatically connect on startup.
    pub auto_connect: bool,
    /// Character name reported to the hub (auto-detected when empty).
    pub character_name: String,
}

impl Default for EqbcConfig {
    fn default() -> Self {
        Self {
            serve: false,
            host: "127.0.0.1".to_string(),
            port: 2112,
            auto_connect: false,
            character_name: String::new(),
        }
    }
}

/// DanNet peer configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DanNetConfig {
    /// UDP port this peer binds and announces on.
    pub port: u16,
    /// Explicit peer addresses to connect to (`host:port`).
    /// Empty list → rely on multicast discovery only.
    pub peers: Vec<String>,
    /// Multicast group for peer discovery.
    pub multicast_group: String,
    /// Server group name this peer belongs to.
    pub group: String,
    /// Character name reported to peers (auto-detected when empty).
    pub character_name: String,
}

impl Default for DanNetConfig {
    fn default() -> Self {
        Self {
            port: 2114,
            peers: Vec::new(),
            multicast_group: "239.255.0.1".to_string(),
            group: "all".to_string(),
            character_name: String::new(),
        }
    }
}

/// NetBots vitals-broadcast configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetBotsConfig {
    /// Emit vitals over the chosen transport.
    pub enabled: bool,
    /// Maximum publish interval (milliseconds). Vitals publish immediately on
    /// change; this caps the burst rate. Must be ≤ 250 to meet the acceptance
    /// criterion.
    pub publish_interval_ms: u64,
    /// Include buff list in every vitals packet.
    pub include_buffs: bool,
}

impl Default for NetBotsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            publish_interval_ms: 100,
            include_buffs: false,
        }
    }
}

/// Top-level transport configuration consumed by the orchestrator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TransportConfig {
    /// Active transport(s).
    pub kind: TransportKind,
    /// EQBC settings (used when kind is Eqbc or Both).
    pub eqbc: EqbcConfig,
    /// DanNet settings (used when kind is DanNet or Both).
    pub dannet: DanNetConfig,
    /// NetBots vitals broadcast settings.
    pub netbots: NetBotsConfig,
}
