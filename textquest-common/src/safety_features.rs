//! Configuration types for the five auto-acceptance and safety features.
//!
//! Covers MQ2AutoAccept, MQ2AutoCamp, MQ2Paranoid (config-only).
//! MQ2Rez config lives in `ipc::AutoRezConfig`.
//! MQ2GMCheck config lives in `textquest::eq::gm_detector::GmAlertConfig`.

use serde::{Deserialize, Serialize};

use crate::trust_list::TrustList;

// ── AutoAccept ────────────────────────────────────────────────────────────────

/// EQ prompt types that AutoAccept can handle automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptablePromptType {
    /// Group join invitation.
    GroupInvite,
    /// Trade window open request.
    Trade,
    /// Task add offer (shared task invitation).
    TaskAdd,
    /// Expedition (DZ) add offer.
    DzAdd,
    /// Translocate spell acceptance.
    Translocate,
    /// Guild hall anchor offer.
    Anchor,
}

impl AcceptablePromptType {
    /// Human-readable label for UI display.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::GroupInvite => "Group Invite",
            Self::Trade => "Trade",
            Self::TaskAdd => "Task Add",
            Self::DzAdd => "Expedition Add",
            Self::Translocate => "Translocate",
            Self::Anchor => "Anchor",
        }
    }
}

/// Per-type enable flags for AutoAccept.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutoAcceptTypeFlags {
    pub group_invites: bool,
    pub trades: bool,
    pub task_adds: bool,
    pub dz_adds: bool,
    pub translocates: bool,
    pub anchors: bool,
}

impl Default for AutoAcceptTypeFlags {
    fn default() -> Self {
        Self {
            group_invites: true,
            trades: false,
            task_adds: true,
            dz_adds: true,
            translocates: true,
            anchors: false,
        }
    }
}

impl AutoAcceptTypeFlags {
    /// Returns true if this prompt type is enabled in the flags.
    #[must_use]
    pub fn is_enabled(&self, prompt_type: AcceptablePromptType) -> bool {
        match prompt_type {
            AcceptablePromptType::GroupInvite => self.group_invites,
            AcceptablePromptType::Trade => self.trades,
            AcceptablePromptType::TaskAdd => self.task_adds,
            AcceptablePromptType::DzAdd => self.dz_adds,
            AcceptablePromptType::Translocate => self.translocates,
            AcceptablePromptType::Anchor => self.anchors,
        }
    }
}

/// Configuration for MQ2AutoAccept — auto-accept prompts per trust list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutoAcceptConfig {
    /// Master enable switch.
    pub enabled: bool,
    /// Per-type enable flags.
    pub prompt_types: AutoAcceptTypeFlags,
    /// When true, only accept prompts from names on the trust list.
    /// When false, accept from anyone (per type flags still apply).
    pub require_trust_list: bool,
    /// Shared trust list. Names must match exactly (case-insensitive).
    pub trust_list: TrustList,
}

impl Default for AutoAcceptConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prompt_types: AutoAcceptTypeFlags::default(),
            require_trust_list: true,
            trust_list: TrustList::default(),
        }
    }
}

// ── AutoCamp ──────────────────────────────────────────────────────────────────

/// Current state of the AutoCamp death-recovery workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoCampState {
    /// No death detected; automation running normally.
    Idle,
    /// Death detected; waiting for camp_delay_secs before issuing /camp.
    PendingCamp,
    /// /camp desktop command has been issued; waiting for process exit.
    Camping,
    /// Client is camped; waiting for relogin_delay_secs before AutoLogin.
    PendingRelogin,
    /// AutoLogin re-login sequence is in progress.
    Relogging,
}

/// Configuration for MQ2AutoCamp — camp desktop on death, then re-login.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutoCampConfig {
    /// Master enable switch.
    pub enabled: bool,
    /// Seconds to wait after death before issuing `/camp desktop`.
    pub camp_delay_secs: u32,
    /// Seconds to wait after camping before attempting re-login.
    pub relogin_delay_secs: u32,
    /// Send a Discord alert before camping.
    pub discord_alert_enabled: bool,
    /// Discord webhook URL for death alerts.
    pub discord_webhook_url: Option<String>,
}

impl Default for AutoCampConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            camp_delay_secs: 30,
            relogin_delay_secs: 60,
            discord_alert_enabled: false,
            discord_webhook_url: None,
        }
    }
}

// ── Paranoid ──────────────────────────────────────────────────────────────────

/// Which player zone transitions Paranoid should report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ParanoidFilter {
    /// Alert on any PC zoning in or out.
    All,
    /// Alert only on strangers (not on the trust list).
    #[default]
    Strangers,
    /// Alert only on trusted friends.
    Friends,
}


/// Configuration for MQ2Paranoid — alert when PCs zone in/out.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParanoidConfig {
    /// Master enable switch.
    pub enabled: bool,
    /// Which zone transitions to report.
    pub filter: ParanoidFilter,
    /// Play a sound on alert.
    pub sound_enabled: bool,
    /// Show TUI toast notification.
    pub toast_enabled: bool,
    /// Discord webhook URL for zone-in alerts.
    pub discord_webhook_url: Option<String>,
    /// Shared friends list used when filter is Friends or Strangers.
    pub trust_list: TrustList,
}

impl Default for ParanoidConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            filter: ParanoidFilter::default(),
            sound_enabled: false,
            toast_enabled: true,
            discord_webhook_url: None,
            trust_list: TrustList::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_type_flags_default() {
        let flags = AutoAcceptTypeFlags::default();
        assert!(flags.is_enabled(AcceptablePromptType::GroupInvite));
        assert!(!flags.is_enabled(AcceptablePromptType::Trade));
        assert!(flags.is_enabled(AcceptablePromptType::TaskAdd));
        assert!(flags.is_enabled(AcceptablePromptType::DzAdd));
        assert!(flags.is_enabled(AcceptablePromptType::Translocate));
        assert!(!flags.is_enabled(AcceptablePromptType::Anchor));
    }

    #[test]
    fn auto_accept_default_requires_trust_list() {
        let cfg = AutoAcceptConfig::default();
        assert!(cfg.enabled);
        assert!(cfg.require_trust_list);
        assert!(cfg.trust_list.is_empty());
    }

    #[test]
    fn auto_camp_default_disabled() {
        let cfg = AutoCampConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.camp_delay_secs, 30);
        assert_eq!(cfg.relogin_delay_secs, 60);
    }

    #[test]
    fn paranoid_default_filter_is_strangers() {
        let cfg = ParanoidConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.filter, ParanoidFilter::Strangers);
    }

    #[test]
    fn prompt_type_labels() {
        assert_eq!(AcceptablePromptType::GroupInvite.label(), "Group Invite");
        assert_eq!(AcceptablePromptType::DzAdd.label(), "Expedition Add");
    }

    #[test]
    fn auto_accept_config_serde_round_trip() {
        let mut cfg = AutoAcceptConfig::default();
        cfg.trust_list.add("Healer".into());
        let json = serde_json::to_string(&cfg).expect("serialize");
        let decoded: AutoAcceptConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, cfg);
    }
}
