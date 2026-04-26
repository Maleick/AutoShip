use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use textquest_common::audio_alerts::AudioAlertConfig;
use textquest_common::box_chat::BoxChatConfig;
use textquest_soul::config::SoulConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerFilterMode {
    #[default]
    All,
    StrangersOnly,
    FriendsOnly,
}

// ─── Account Configuration ───────────────────────────────────────────────

/// A single account entry from config/accounts.toml.
#[derive(Debug, Deserialize, Clone)]
pub struct AccountEntry {
    /// Account login name (e.g., "dmft01").
    pub name: String,
    /// Target server name (e.g., "Firiona Vie").
    pub server: String,
    /// Character name to log in as.
    pub character: String,
    /// Short class code (e.g., "WAR", "CLR").
    #[serde(default = "default_class")]
    pub class: String,
    /// Group ID this account belongs to (0 = ungrouped).
    #[serde(default = "default_group")]
    pub group: u32,
}

fn default_class() -> String {
    "UNK".to_string()
}

fn default_group() -> u32 {
    0
}

/// A named profile group mapping a human-readable name and optional hotkey to a
/// group ID.
///
/// Profile groups allow launching all accounts in a numeric group with a single
/// name or keyboard hotkey, matching the MQ2 AutoLogin profile group concept.
///
/// # TOML example
///
/// ```toml
/// [[profile_groups]]
/// id = 1
/// name = "MainRaid"
/// hotkey = "F1"
///
/// [[profile_groups]]
/// id = 2
/// name = "SecondRaid"
/// hotkey = "F2"
/// ```
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ProfileGroup {
    /// Numeric group ID matching `AccountEntry::group`.
    pub id: u32,
    /// Human-readable profile name (e.g., "MainRaid").
    pub name: String,
    /// Optional function key hotkey to launch this profile from the TUI (e.g.,
    /// `"F1"`–`"F9"`).
    #[serde(default)]
    pub hotkey: Option<String>,
}

/// Top-level wrapper for config/accounts.toml.
#[derive(Debug, Deserialize, Clone)]
pub struct AccountsConfig {
    /// List of account entries defined in the config file.
    #[serde(default)]
    pub accounts: Vec<AccountEntry>,
    /// Named profile groups with optional hotkeys for one-action
    /// multi-character launches.
    #[serde(default)]
    pub profile_groups: Vec<ProfileGroup>,
    /// Camera presets for quick viewpoint actions.
    #[serde(default)]
    pub camera_presets: Vec<CameraPreset>,
}

/// A named camera preset for quick viewpoint actions.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct CameraPreset {
    /// Human-readable preset name (e.g., "Close", "Far", "First Person").
    pub name: String,
    /// Optional hotkey to activate this preset (e.g., "F5").
    #[serde(default)]
    pub hotkey: Option<String>,
    /// Optional camera distance or zoom level (game-dependent).
    #[serde(default)]
    pub distance: Option<f32>,
    /// Optional camera pitch angle in degrees.
    #[serde(default)]
    pub pitch: Option<f32>,
    /// Optional camera yaw angle in degrees.
    #[serde(default)]
    pub yaw: Option<f32>,
    /// Whether this preset is the default on startup.
    #[serde(default)]
    pub is_default: bool,
}

impl AccountsConfig {
    /// Find a camera preset by hotkey (case-insensitive).
    #[must_use]
    pub fn camera_preset_by_hotkey(&self, hotkey: &str) -> Option<&CameraPreset> {
        let lower = hotkey.to_lowercase();
        self.camera_presets.iter().find(|cp| {
            cp.hotkey
                .as_ref()
                .is_some_and(|h| h.to_lowercase() == lower)
        })
    }
}

impl AccountsConfig {
    /// Load account definitions from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read accounts config: {}", path.display()))?;
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse accounts config: {}", path.display()))?;
        Ok(config)
    }

    /// Return accounts belonging to a specific group.
    #[must_use]
    pub fn accounts_for_group(&self, group_id: u32) -> Vec<&AccountEntry> {
        self.accounts
            .iter()
            .filter(|a| a.group == group_id)
            .collect()
    }

    /// Return all accounts belonging to a named profile group.
    ///
    /// Returns `None` if no profile group with that name exists.
    #[must_use]
    pub fn accounts_for_profile_name(&self, name: &str) -> Option<Vec<&AccountEntry>> {
        let pg = self.profile_by_name(name)?;
        Some(self.accounts_for_group(pg.id))
    }

    /// Find a profile group by name (case-insensitive).
    #[must_use]
    pub fn profile_by_name(&self, name: &str) -> Option<&ProfileGroup> {
        let lower = name.to_lowercase();
        self.profile_groups
            .iter()
            .find(|pg| pg.name.to_lowercase() == lower)
    }

    /// Find a profile group by hotkey (case-insensitive, e.g., `"F1"`).
    #[must_use]
    pub fn profile_by_hotkey(&self, hotkey: &str) -> Option<&ProfileGroup> {
        let lower = hotkey.to_lowercase();
        self.profile_groups.iter().find(|pg| {
            pg.hotkey
                .as_ref()
                .is_some_and(|h| h.to_lowercase() == lower)
        })
    }

    /// Find a single account by name (case-insensitive).
    #[must_use]
    pub fn find_account(&self, name: &str) -> Option<&AccountEntry> {
        let lower = name.to_lowercase();
        self.accounts
            .iter()
            .find(|a| a.name.to_lowercase() == lower)
    }

    /// Convert an `AccountEntry` into the `AccountInfo` used by the launch
    /// system.
    #[must_use]
    pub fn to_account_info(entry: &AccountEntry) -> textquest_common::login::AccountInfo {
        textquest_common::login::AccountInfo {
            account_name: entry.name.clone(),
            character_name: entry.character.clone(),
            class_name: entry.class.clone(),
            level: 1,
            group_id: entry.group,
            server_name: entry.server.clone(),
        }
    }

    /// Resolve a profile name to a list of `AccountInfo` values.
    ///
    /// Loads `config/accounts.toml` from the given path, finds the named
    /// profile group, and returns the accounts in that group as `AccountInfo`.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file cannot be read or parsed, or if no
    /// profile group with the given name exists.
    pub fn load_accounts_for_profile(
        accounts_path: &Path,
        profile_name: &str,
    ) -> Result<Vec<textquest_common::login::AccountInfo>> {
        let cfg = Self::load(accounts_path)?;
        let entries = cfg.accounts_for_profile_name(profile_name).ok_or_else(|| {
            anyhow::anyhow!("No profile group named '{profile_name}' found in accounts config")
        })?;
        Ok(entries.into_iter().map(Self::to_account_info).collect())
    }

    /// Load all accounts from the accounts config file as `AccountInfo` values.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file cannot be read or parsed.
    pub fn load_all_accounts(
        accounts_path: &Path,
    ) -> Result<Vec<textquest_common::login::AccountInfo>> {
        let cfg = Self::load(accounts_path)?;
        Ok(cfg.accounts.iter().map(Self::to_account_info).collect())
    }

    /// Validate that every account in `accounts` has a credential entry in the
    /// provided credential store.
    ///
    /// Returns `Ok(())` if all accounts are present.  Returns a descriptive
    /// error listing the first missing account name on failure.
    ///
    /// # Errors
    ///
    /// Returns an error if the credential store cannot be queried, or if any
    /// account in `accounts` is missing from the store.
    #[cfg(windows)]
    pub fn validate_credentials_exist(
        accounts: &[textquest_common::login::AccountInfo],
        store: &crate::credentials::store::CredentialStore,
    ) -> Result<()> {
        let stored = store.list_accounts()?;
        let stored_lower: std::collections::HashSet<String> =
            stored.iter().map(|s| s.to_lowercase()).collect();
        for acct in accounts {
            if !stored_lower.contains(&acct.account_name.to_lowercase()) {
                anyhow::bail!(
                    "Account '{}' is not in the credential store",
                    acct.account_name
                );
            }
        }
        Ok(())
    }
}

/// Top-level application configuration loaded from the TOML config file.
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    /// Name of the EQ process to attach to (default: "eqgame.exe")
    #[serde(default = "default_process_name")]
    pub process_name: String,

    /// Maximum spawns to read from the linked list (safety limit)
    #[serde(default = "default_max_spawns")]
    pub max_spawns: usize,

    /// Group definitions (optional for M1, needed for later milestones)
    #[serde(default)]
    #[allow(dead_code)]
    pub group: Vec<GroupConfig>,

    /// Launch configuration for starting EQ clients
    #[serde(default)]
    #[allow(dead_code)]
    pub launch: LaunchConfig,

    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,

    /// Retry / backoff configuration
    #[serde(default)]
    #[allow(dead_code)]
    pub retry: RetryConfig,

    /// Soul Engine configuration
    #[serde(default)]
    pub soul: SoulConfig,

    /// Discord integration configuration
    #[serde(default)]
    pub discord: DiscordConfig,

    /// Operational alerting configuration.
    #[serde(default)]
    pub alerts: AlertingConfig,

    /// Orchestrator event loop configuration
    #[serde(default)]
    pub orchestrator: OrchestratorConfig,

    /// Persisted hotkey and slash-command registration metadata.
    #[serde(default)]
    pub input_bindings: crate::registry::RegistryConfig,

    /// Spawn watch / alert feed configuration
    #[serde(default)]
    pub spawn_watch: SpawnWatchConfig,

    /// Vendor item watch configuration for merchant browse alerts.
    #[serde(default)]
    pub vendor_watch: VendorWatchConfig,

    /// Enable periodic hook unhook/rehook rotation to evade point-in-time
    /// scans.
    #[serde(default)]
    pub hook_rotation_enabled: bool,

    /// Interval (ms) between hook rotation cycles.
    #[serde(default = "default_hook_rotation_interval_ms")]
    pub hook_rotation_interval_ms: u64,

    /// Optional decentralized UDP multicast peer discovery.
    #[serde(default)]
    pub discovery: PeerDiscoveryConfig,

    /// TCP relay settings for EQBC-style box-chat commands on the local host.
    #[serde(default)]
    pub box_chat: BoxChatConfig,

    /// Chat logging configuration (MQ2Log parity).
    #[serde(default)]
    pub chat_log: crate::chat_log::ChatLogConfig,

    /// Enable timing-based anti-debug evasion correction.
    ///
    /// When enabled, hooks correct timing APIs (`GetTickCount` and
    /// `QueryPerformanceCounter`) by subtracting hook overhead from observed
    /// values.
    #[serde(default)]
    pub timing_correction: bool,

    /// Kill tracker configuration for auto-reporting and session tracking.
    #[serde(default)]
    pub kill_tracker: KillTrackerConfig,

    /// XP and AA tracker configuration.
    #[serde(default)]
    pub xp_tracker: XpTrackerConfig,

    /// Platinum tracker configuration.
    #[serde(default)]
    pub plat_tracker: PlatTrackerConfig,

    /// Say detection and alerting configuration.
    #[serde(default)]
    pub say_detection: SayDetectionConfig,

    /// Log file rotation and retention configuration.
    #[serde(default)]
    pub log: LogConfig,

    /// Self-improvement loop configuration for Bayesian suggestion engine.
    #[serde(default)]
    pub improve: ImprovementConfig,

    /// Session recording configuration for self-improvement loop.
    #[cfg(windows)]
    #[serde(default)]
    pub session_recorder: crate::metrics::SessionRecorderConfig,
}

/// Kill tracker auto-reporting configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KillTrackerConfig {
    /// Whether session tracking and auto-reporting are enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Minutes between automatic status reports. `0` disables auto-reporting.
    #[serde(default = "default_kill_tracker_interval_minutes")]
    pub auto_report_interval_minutes: u32,
    /// In-game chat channel used for auto-reports.
    #[serde(default = "default_kill_tracker_channel")]
    pub auto_report_channel: String,
    /// Include top mob breakdowns in generated reports.
    #[serde(default = "default_true")]
    pub auto_report_include_mobs: bool,
    /// Include kills-per-hour and efficiency summary lines.
    #[serde(default = "default_true")]
    pub auto_report_include_kph: bool,
    /// Track per-character session history instead of a single global bucket.
    #[serde(default = "default_true")]
    pub track_per_character: bool,
    /// Maximum number of historical sessions to retain in memory/on disk.
    #[serde(default = "default_kill_tracker_max_session_history")]
    pub max_session_history: usize,
}

impl Default for KillTrackerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_report_interval_minutes: default_kill_tracker_interval_minutes(),
            auto_report_channel: default_kill_tracker_channel(),
            auto_report_include_mobs: true,
            auto_report_include_kph: true,
            track_per_character: true,
            max_session_history: default_kill_tracker_max_session_history(),
        }
    }
}

/// Say detection rule actions.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SayRuleAction {
    /// Fire an alert through the configured notification channels.
    #[default]
    Alert,
    /// Send a slash command string via IPC to all clients.
    Broadcast,
    /// Run a command on the client that saw the `/say` line.
    Command,
}

fn default_true() -> bool {
    true
}

fn default_kill_tracker_interval_minutes() -> u32 {
    10
}

fn default_kill_tracker_channel() -> String {
    "group".to_string()
}

fn default_kill_tracker_max_session_history() -> usize {
    100
}

/// XP and AA tracker configuration (MQ2XPTracker parity).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct XpTrackerConfig {
    /// Whether XP and AA tracking are enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum rolling samples retained per session.
    #[serde(default = "default_xp_tracker_max_samples")]
    pub max_samples: usize,
}

impl Default for XpTrackerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_samples: default_xp_tracker_max_samples(),
        }
    }
}

fn default_xp_tracker_max_samples() -> usize {
    1000
}

/// Plat tracker configuration (MQ2PlatTracker parity).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlatTrackerConfig {
    /// Whether platinum tracking is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum transactions retained per session.
    #[serde(default = "default_plat_tracker_max_transactions")]
    pub max_transactions: usize,
    /// Track per-character sessions (vs. global aggregate).
    #[serde(default = "default_true")]
    pub track_per_character: bool,
}

impl Default for PlatTrackerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_transactions: default_plat_tracker_max_transactions(),
            track_per_character: true,
        }
    }
}

fn default_plat_tracker_max_transactions() -> usize {
    2000
}

/// Say pattern matching mode.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SayPatternType {
    /// Exact substring match (case-insensitive).
    #[default]
    Substring,
    /// Case-insensitive exact string match.
    Exact,
    /// Regular expression pattern.
    Regex,
}

/// A single say detection rule (persisted to config file).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SayRuleConfig {
    /// Human-readable name for this rule.
    pub name: String,
    /// Pattern text to match against say messages.
    pub pattern: String,
    /// Pattern matching mode.
    #[serde(default)]
    pub pattern_type: SayPatternType,
    /// Action to take when matched.
    #[serde(default)]
    pub action_type: SayRuleAction,
    /// Optional action payload used by command and broadcast rules.
    #[serde(default)]
    pub action_value: Option<String>,
    /// Whether this rule is active.
    #[serde(default = "default_rule_enabled")]
    pub enabled: bool,
}

fn default_rule_enabled() -> bool {
    true
}

/// Top-level say detection configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SayDetectionConfig {
    /// Enable the say detection engine.
    #[serde(default)]
    pub enabled: bool,
    /// Enable audio alert delivery for alert rules.
    #[serde(default = "default_rule_enabled")]
    pub sound_enabled: bool,
    /// Optional sound file name for alert rules.
    #[serde(default = "default_say_sound_file")]
    pub sound_file: Option<String>,
    /// Enable toast-style operator notifications for alert rules.
    #[serde(default = "default_rule_enabled")]
    pub toast_enabled: bool,
    /// Optional Discord webhook for alert rules.
    #[serde(default)]
    pub discord_webhook_url: Option<String>,
    /// Mirror alert notifications to all clients via `/echo`.
    #[serde(default)]
    pub broadcast_all_clients: bool,
    /// List of detection rules.
    #[serde(default)]
    pub rules: Vec<SayRuleConfig>,
}

fn default_say_sound_file() -> Option<String> {
    Some("say_alert.wav".to_string())
}

impl Default for SayDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sound_enabled: true,
            sound_file: default_say_sound_file(),
            toast_enabled: true,
            discord_webhook_url: None,
            broadcast_all_clients: false,
            rules: Vec::new(),
        }
    }
}

/// Discord webhook and bot configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DiscordConfig {
    /// Bot token for the embedded Discord bot. Empty = bot disabled.
    /// Configure via `[discord] bot_token = "..."` in TOML config.
    #[serde(default)]
    pub bot_token: String,
    /// Guild (server) ID for registering slash commands.
    #[serde(default)]
    pub guild_id: u64,
    /// Default webhook URL for alerts not routed to a specific channel.
    /// Empty = disabled.
    pub webhook_url: String,
    /// Per-category webhook channel routing.
    /// Keys: "kills", "loot", "timers", "feats", "status".
    /// Each maps to a Discord webhook URL for that category's channel.
    #[serde(default)]
    pub channels: std::collections::HashMap<String, String>,
    /// Whether to send alerts for HVT (high-value target) detections.
    pub alert_hvt: bool,
    /// Whether to send alerts for client crashes/disconnects.
    pub alert_crashes: bool,
    /// Whether to send alerts for mass login failures.
    pub alert_mass_failures: bool,
    /// Whether to send status updates (camp started, login complete).
    pub alert_status: bool,
    /// Discord sender names allowed to execute bridge commands.
    /// Empty list disables remote command execution.
    #[serde(default)]
    pub command_allowed_senders: Vec<String>,
    /// Per-chat-channel webhook routing for in-game chat relay.
    ///
    /// Keys match [`crate::discord::relay::ChatChannel::config_key`] values:
    /// `"group"`, `"raid"`, `"guild"`, `"ooc"`, `"shout"`, `"say"`, `"tell"`.
    ///
    /// # TOML example
    ///
    /// ```toml
    /// [discord.chat_channels]
    /// group = "https://discord.com/api/webhooks/.../group-chat"
    /// raid  = "https://discord.com/api/webhooks/.../raid-chat"
    /// guild = "https://discord.com/api/webhooks/.../guild-chat"
    /// ```
    #[serde(default)]
    pub chat_channels: std::collections::HashMap<String, String>,
}

impl Default for DiscordConfig {
    fn default() -> Self {
        Self {
            bot_token: String::new(),
            guild_id: 0,
            webhook_url: String::new(),
            channels: std::collections::HashMap::new(),
            alert_hvt: true,
            alert_crashes: true,
            alert_mass_failures: true,
            alert_status: false,
            command_allowed_senders: Vec::new(),
            chat_channels: std::collections::HashMap::new(),
        }
    }
}

/// Thresholds that drive warning and timeout alert generation.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct AlertThresholdConfig {
    pub death_alert: bool,
    pub stuck_alert: bool,
    pub memory_warning_mb: u32,
    pub ipc_latency_warning_ms: u32,
    pub error_rate_warning_per_min: u32,
    pub dps_drop_warning_pct: u32,
    pub zone_timeout_secs: u64,
}

impl Default for AlertThresholdConfig {
    fn default() -> Self {
        Self {
            death_alert: true,
            stuck_alert: true,
            memory_warning_mb: 200,
            ipc_latency_warning_ms: 10,
            error_rate_warning_per_min: 5,
            dps_drop_warning_pct: 20,
            zone_timeout_secs: 60,
        }
    }
}

/// Operational alert delivery configuration for Discord, email, and batching.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct AlertingConfig {
    pub enable_discord: bool,
    pub discord_webhook_url: String,
    pub enable_email: bool,
    pub smtp_server: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub email_from: String,
    pub email_recipients: Vec<String>,
    pub email_subject_prefix: String,
    pub warning_batch_window_secs: u64,
    pub thresholds: AlertThresholdConfig,
    pub audio: AudioAlertConfig,
}

impl Default for AlertingConfig {
    fn default() -> Self {
        Self {
            enable_discord: false,
            discord_webhook_url: String::new(),
            enable_email: false,
            smtp_server: String::new(),
            smtp_port: 587,
            smtp_username: String::new(),
            smtp_password: String::new(),
            email_from: String::new(),
            email_recipients: Vec::new(),
            email_subject_prefix: String::from("[TextQuest] "),
            warning_batch_window_secs: 300,
            thresholds: AlertThresholdConfig::default(),
            audio: AudioAlertConfig::default(),
        }
    }
}

/// Configuration for a group of characters that play together.
#[allow(dead_code)] // Deserialized from config, consumed in later milestones
#[derive(Debug, Default, Deserialize, Clone)]
#[serde(default)]
pub struct GroupConfig {
    /// Numeric group identifier.
    pub id: u32,
    /// Human-readable group name.
    pub name: String,
    /// Toon (character) definitions within this group.
    #[serde(default)]
    pub toon: Vec<ToonConfig>,
}

/// Backward-compatible placeholder for legacy unattended camp-out settings.
///
/// This type is kept public to avoid breaking downstream code and to allow
/// older configuration files that still contain the `auto_camp_on_death`
/// section to continue deserializing successfully.
#[deprecated(
    note = "auto_camp_on_death has been retired; this compatibility type remains for backward compatibility"
)]
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct AutoCampOnDeathConfig {
    /// Whether the death-camp workflow is enabled for this toon.
    pub enabled: bool,
    /// Delay before camping the character out after death.
    pub camp_delay_secs: u64,
    /// Delay before attempting an automated relog after the camp-out.
    pub relog_wait_secs: u64,
}

#[allow(deprecated)]
impl Default for AutoCampOnDeathConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            camp_delay_secs: 30,
            relog_wait_secs: 900,
        }
    }
}

/// Configuration for a single character (toon) within a group.
#[allow(dead_code)] // Deserialized from config, consumed in later milestones
#[allow(deprecated)] // auto_camp_on_death field uses deprecated backward-compat type
#[derive(Debug, Deserialize, Clone)]
pub struct ToonConfig {
    /// Character name.
    pub name: String,
    /// Class short code (e.g., "WAR").
    pub class: String,
    /// Role assignment (e.g., "tank", "healer").
    pub role: String,
    /// EQ window title for targeting this client.
    #[serde(default)]
    pub eq_window_title: String,
    /// Account name this toon belongs to.
    #[serde(default)]
    pub account: Option<String>,
    /// Unattended death auto-camp and relog behavior.
    #[serde(default)]
    #[allow(deprecated)]
    pub auto_camp_on_death: AutoCampOnDeathConfig,
}

/// Configuration for EQ client launching — paths, stagger timing, and resource
/// limits.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LaunchConfig {
    /// Path to the EQ installation directory.
    pub eq_path: String,
    /// Minimum delay between client launches (seconds).
    pub stagger_min_secs: u64,
    /// Maximum delay between client launches (seconds).
    pub stagger_max_secs: u64,
    /// Maximum number of clients launching simultaneously.
    pub max_concurrent_launches: usize,
    /// Additional command-line arguments for the EQ process.
    pub launch_args: Vec<String>,
    /// Maximum physical RAM (working set) per EQ client in MB. 0 = unlimited.
    pub max_working_set_mb: u32,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            eq_path: String::new(),
            stagger_min_secs: 3,
            stagger_max_secs: 15,
            max_concurrent_launches: 3,
            launch_args: Vec::new(),
            max_working_set_mb: 800,
        }
    }
}

/// EQ server connection settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Server name (e.g., "Firiona Vie").
    pub name: String,
    /// URL for server status checks.
    pub status_url: Option<String>,
    /// Timeout in seconds for server status HTTP checks.
    pub status_check_timeout_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: "Firiona Vie".to_string(),
            status_url: None,
            status_check_timeout_secs: 10,
        }
    }
}

/// Retry and backoff policy for failed client launches.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RetryConfig {
    /// Maximum number of retry attempts before giving up.
    pub max_retries: u32,
    /// Base backoff delay in seconds for the first retry.
    pub base_backoff_secs: u64,
    /// Maximum backoff delay in seconds (caps exponential growth).
    pub max_backoff_secs: u64,
    /// Exponential multiplier applied to the base delay on each retry
    /// (e.g., 2.0 → doubles each attempt).
    pub backoff_multiplier: f64,
    /// Jitter fraction added to the computed delay (0.0 = none, 0.25 = up to
    /// 25% extra). Prevents thundering-herd during mass reconnect events.
    pub backoff_jitter: f64,
    /// Number of failures within the window to trigger mass-failure mode.
    pub mass_failure_threshold: u32,
    /// Time window in seconds for mass-failure detection.
    pub mass_failure_window_secs: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_backoff_secs: 1,
            max_backoff_secs: 60,
            backoff_multiplier: 2.0,
            backoff_jitter: 0.1,
            mass_failure_threshold: 5,
            mass_failure_window_secs: 60,
        }
    }
}

/// Configuration for the orchestrator event loop tick rates.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct OrchestratorConfig {
    /// Interval in milliseconds between health checks on connected clients.
    pub health_check_interval_ms: u64,
    /// Interval in milliseconds between launch coordinator ticks.
    pub launch_tick_interval_ms: u64,
    /// Interval in milliseconds between camp/hunt orchestrator ticks.
    pub orchestrator_tick_interval_ms: u64,
    /// Interval in milliseconds between game state polls from shared memory.
    pub state_poll_interval_ms: u64,
    /// Interval in milliseconds between fleet progress report emissions.
    ///
    /// A `ProgressReported` [`crate::orchestrator_loop::LoopEvent`] is emitted
    /// every `progress_report_interval_ms` milliseconds. Set to 0 to disable.
    /// Defaults to 30 000 ms (30 seconds).
    pub progress_report_interval_ms: u64,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            health_check_interval_ms: 5000,
            launch_tick_interval_ms: 1000,
            orchestrator_tick_interval_ms: 250,
            state_poll_interval_ms: 100,
            progress_report_interval_ms: 30_000,
        }
    }
}

/// Configuration for optional UDP multicast peer discovery between
/// orchestrators.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PeerDiscoveryConfig {
    /// Enable multicast peer discovery announcements and peer listening.
    pub multicast_enabled: bool,
    /// Local IPv4 bind address for the discovery listener.
    pub bind_addr: String,
    /// IPv4 multicast group address used for announcements.
    pub multicast_addr: String,
    /// UDP port shared by all discovery participants.
    pub port: u16,
    /// Interval between outbound announcements.
    pub announce_interval_ms: u64,
    /// Time-to-live for remote peers before they expire locally.
    pub peer_ttl_ms: u64,
    /// Optional operator-defined node label. Empty falls back to hostname.
    pub node_name: String,
    /// Multicast packet TTL for routed subnets.
    pub multicast_ttl: u32,
}

impl Default for PeerDiscoveryConfig {
    fn default() -> Self {
        Self {
            multicast_enabled: false,
            bind_addr: "0.0.0.0".to_string(),
            multicast_addr: "239.255.42.99".to_string(),
            port: 35353,
            announce_interval_ms: 1_000,
            peer_ttl_ms: 5_000,
            node_name: String::new(),
            multicast_ttl: 1,
        }
    }
}

/// Log file rotation and retention configuration.
///
/// Controls how many log files are kept, their maximum combined size, and how
/// long files are retained before automatic deletion.
///
/// # TOML example
///
/// ```toml
/// [log]
/// max_size_mb  = 200   # Delete oldest files until total log dir size < 200 MB
/// max_files    = 14    # Keep at most 14 rolling files (daily rotation)
/// max_age_days = 14    # Delete files older than 14 days
/// ```
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LogConfig {
    /// Maximum total size in megabytes for all log files matching a given
    /// prefix.  When exceeded, the oldest files are removed first.  `0`
    /// disables size-based pruning.
    pub max_size_mb: u64,
    /// Maximum number of rolling log files to retain.  Passed directly to
    /// `tracing-appender`'s `RollingFileAppender`.  `0` uses the appender's
    /// default (unlimited).
    pub max_files: usize,
    /// Delete log files older than this many days.  `0` disables age-based
    /// pruning.
    pub max_age_days: u64,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            max_size_mb: 100,
            max_files: 7,
            max_age_days: 30,
        }
    }
}

/// Self-improvement loop configuration for Bayesian suggestion engine.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct ImprovementConfig {
    /// Enable Bayesian tier for the suggestion engine.
    pub bayesian_tier_enabled: bool,
    /// Path to the posteriors database (relative to working directory).
    pub posteriors_db_path: String,
}

impl Default for ImprovementConfig {
    fn default() -> Self {
        Self {
            bayesian_tier_enabled: false,
            posteriors_db_path: "data/posteriors.db".to_string(),
        }
    }
}

fn default_process_name() -> String {
    "eqgame.exe".to_string()
}

fn default_max_spawns() -> usize {
    2048
}

fn default_hook_rotation_interval_ms() -> u64 {
    30_000
}

impl AppConfig {
    /// Load application configuration from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        config.input_bindings.validate().with_context(|| {
            format!("Invalid input bindings in config file: {}", path.display())
        })?;
        Ok(config)
    }

    /// Returns a default configuration with sensible defaults.
    #[must_use]
    pub fn default_config() -> Self {
        Self {
            process_name: default_process_name(),
            max_spawns: default_max_spawns(),
            group: Vec::new(),
            launch: LaunchConfig::default(),
            server: ServerConfig::default(),
            retry: RetryConfig::default(),
            soul: SoulConfig::default(),
            discord: DiscordConfig::default(),
            alerts: AlertingConfig::default(),
            orchestrator: OrchestratorConfig::default(),
            input_bindings: crate::registry::RegistryConfig::default(),
            spawn_watch: SpawnWatchConfig::default(),
            vendor_watch: VendorWatchConfig::default(),
            hook_rotation_enabled: false,
            hook_rotation_interval_ms: default_hook_rotation_interval_ms(),
            discovery: PeerDiscoveryConfig::default(),
            box_chat: BoxChatConfig::default(),
            chat_log: crate::chat_log::ChatLogConfig::default(),
            timing_correction: false,
            kill_tracker: KillTrackerConfig::default(),
            xp_tracker: XpTrackerConfig::default(),
            plat_tracker: PlatTrackerConfig::default(),
            say_detection: SayDetectionConfig::default(),
            log: LogConfig::default(),
            improve: ImprovementConfig::default(),
        }
    }
}

/// Configuration for spawn watch alerts and the alert feed.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SpawnWatchConfig {
    pub enabled: bool,
    pub watch_names: Vec<String>,
    pub alert_named: bool,
    pub max_feed_entries: usize,
    pub player_filter_mode: PlayerFilterMode,
    pub sound_on_player_zone_in: bool,
    pub friends: Vec<String>,
}

impl Default for SpawnWatchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            watch_names: Vec::new(),
            alert_named: true,
            max_feed_entries: 200,
            player_filter_mode: PlayerFilterMode::default(),
            sound_on_player_zone_in: false,
            friends: Vec::new(),
        }
    }
}

/// One vendor item watch entry loaded from config.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct VendorWatchItemConfig {
    pub item_name: String,
    #[serde(default)]
    pub max_price_copper: Option<u64>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Vendor watch configuration for merchant browse alerts.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct VendorWatchConfig {
    pub enabled: bool,
    pub items: Vec<VendorWatchItemConfig>,
}

impl Default for VendorWatchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            items: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ACCOUNTS_TOML: &str = r#"
[[accounts]]
name = "dmft01"
server = "Firiona Vie"
character = "Camrene"
class = "WAR"
group = 1

[[accounts]]
name = "dmft02"
server = "Firiona Vie"
character = "Zisdarenu"
class = "SHM"
group = 1

[[accounts]]
name = "dmft07"
server = "Firiona Vie"
character = "Paladin"
class = "PAL"
group = 2
"#;

    fn parse_sample() -> AccountsConfig {
        toml::from_str(SAMPLE_ACCOUNTS_TOML).unwrap()
    }

    #[test]
    fn parse_accounts_toml() {
        let cfg = parse_sample();
        assert_eq!(cfg.accounts.len(), 3);
        assert_eq!(cfg.accounts[0].name, "dmft01");
        assert_eq!(cfg.accounts[0].character, "Camrene");
        assert_eq!(cfg.accounts[0].class, "WAR");
        assert_eq!(cfg.accounts[0].group, 1);
        assert_eq!(cfg.accounts[0].server, "Firiona Vie");
    }

    #[test]
    fn accounts_for_group_filters_correctly() {
        let cfg = parse_sample();
        let g1 = cfg.accounts_for_group(1);
        assert_eq!(g1.len(), 2);
        assert!(g1.iter().all(|a| a.group == 1));

        let g2 = cfg.accounts_for_group(2);
        assert_eq!(g2.len(), 1);
        assert_eq!(g2[0].name, "dmft07");

        let g99 = cfg.accounts_for_group(99);
        assert!(g99.is_empty());
    }

    #[test]
    fn find_account_case_insensitive() {
        let cfg = parse_sample();
        assert!(cfg.find_account("dmft01").is_some());
        assert!(cfg.find_account("DMFT01").is_some());
        assert!(cfg.find_account("Dmft01").is_some());
        assert!(cfg.find_account("nonexistent").is_none());
    }

    #[test]
    fn to_account_info_conversion() {
        let cfg = parse_sample();
        let info = AccountsConfig::to_account_info(&cfg.accounts[0]);
        assert_eq!(info.account_name, "dmft01");
        assert_eq!(info.character_name, "Camrene");
        assert_eq!(info.class_name, "WAR");
        assert_eq!(info.group_id, 1);
        assert_eq!(info.server_name, "Firiona Vie");
    }

    #[test]
    fn empty_accounts_toml() {
        let cfg: AccountsConfig = toml::from_str("").unwrap();
        assert!(cfg.accounts.is_empty());
    }

    #[test]
    fn defaults_for_optional_fields() {
        let cfg: AccountsConfig = toml::from_str(
            r#"
[[accounts]]
name = "test"
server = "Test"
character = "Foo"
"#,
        )
        .unwrap();
        assert_eq!(cfg.accounts[0].class, "UNK");
        assert_eq!(cfg.accounts[0].group, 0);
    }

    #[test]
    fn app_config_defaults() {
        let cfg = AppConfig::default_config();
        assert_eq!(cfg.process_name, "eqgame.exe");
        assert_eq!(cfg.max_spawns, 2048);
        assert!(cfg.group.is_empty());
        assert!(!cfg.discovery.multicast_enabled);
        assert_eq!(cfg.box_chat, BoxChatConfig::default());
        assert!(!cfg.chat_log.enabled);
        assert!(!cfg.timing_correction);
        assert!(!cfg.hook_rotation_enabled);
        assert_eq!(cfg.hook_rotation_interval_ms, 30_000);
    }

    #[test]
    fn spawn_watch_defaults_are_sensible() {
        let cfg = AppConfig::default_config();
        assert!(cfg.spawn_watch.enabled);
        assert!(cfg.spawn_watch.watch_names.is_empty());
        assert!(cfg.spawn_watch.alert_named);
        assert_eq!(cfg.spawn_watch.max_feed_entries, 200);
    }

    #[test]
    fn spawn_watch_defaults_include_player_notifications() {
        let cfg = AppConfig::default_config();
        assert_eq!(cfg.spawn_watch.player_filter_mode, PlayerFilterMode::All);
        assert!(!cfg.spawn_watch.sound_on_player_zone_in);
        assert!(cfg.spawn_watch.friends.is_empty());
    }

    #[test]
    fn vendor_watch_defaults_are_enabled_with_empty_items() {
        let cfg = AppConfig::default_config();
        assert!(cfg.vendor_watch.enabled);
        assert!(cfg.vendor_watch.items.is_empty());
    }

    #[test]
    fn player_filter_mode_default_is_all() {
        assert_eq!(PlayerFilterMode::default(), PlayerFilterMode::All);
    }

    #[test]
    fn spawn_watch_player_filter_mode_parses_from_toml() {
        let cfg: AppConfig = toml::from_str(
            r#"
[spawn_watch]
player_filter_mode = "strangers_only"
"#,
        )
        .unwrap();
        assert_eq!(
            cfg.spawn_watch.player_filter_mode,
            PlayerFilterMode::StrangersOnly
        );
    }

    #[test]
    fn spawn_watch_watch_names_parse_from_toml() {
        let cfg: AppConfig = toml::from_str(
            r#"
[spawn_watch]
watch_names = ["Quillmane", "Raster of Guk"]
"#,
        )
        .unwrap();
        assert_eq!(cfg.spawn_watch.watch_names.len(), 2);
        assert_eq!(cfg.spawn_watch.watch_names[0], "Quillmane");
        assert_eq!(cfg.spawn_watch.watch_names[1], "Raster of Guk");
    }

    #[test]
    fn spawn_watch_friends_parses_from_toml() {
        let cfg: AppConfig = toml::from_str(
            r#"
[spawn_watch]
friends = ["Camrene", "Zisdarenu"]
"#,
        )
        .unwrap();
        assert_eq!(cfg.spawn_watch.friends.len(), 2);
        assert!(cfg.spawn_watch.friends.contains(&"Camrene".to_string()));
        assert!(cfg.spawn_watch.friends.contains(&"Zisdarenu".to_string()));
    }

    #[test]
    fn spawn_watch_custom_values_parse_from_toml() {
        let cfg: AppConfig = toml::from_str(
            r#"
[spawn_watch]
enabled = false
alert_named = false
max_feed_entries = 42
"#,
        )
        .unwrap();
        assert!(!cfg.spawn_watch.enabled);
        assert!(!cfg.spawn_watch.alert_named);
        assert_eq!(cfg.spawn_watch.max_feed_entries, 42);
    }

    #[test]
    fn vendor_watch_items_parse_from_toml() {
        let cfg: AppConfig = toml::from_str(
            r#"
[vendor_watch]
enabled = true

[[vendor_watch.items]]
item_name = "Fungi Covered Scale Tunic"
max_price_copper = 500000

[[vendor_watch.items]]
item_name = "Holgresh Elder Beads"
enabled = false
"#,
        )
        .unwrap();
        assert!(cfg.vendor_watch.enabled);
        assert_eq!(cfg.vendor_watch.items.len(), 2);
        assert_eq!(
            cfg.vendor_watch.items[0].item_name,
            "Fungi Covered Scale Tunic"
        );
        assert_eq!(cfg.vendor_watch.items[0].max_price_copper, Some(500000));
        assert!(cfg.vendor_watch.items[0].enabled);
        assert_eq!(cfg.vendor_watch.items[1].item_name, "Holgresh Elder Beads");
        assert_eq!(cfg.vendor_watch.items[1].max_price_copper, None);
        assert!(!cfg.vendor_watch.items[1].enabled);
    }

    #[test]
    fn app_config_timing_correction_can_be_deserialized() {
        let cfg: AppConfig = toml::from_str(
            r#"
timing_correction = true
"#,
        )
        .unwrap();

        assert!(cfg.timing_correction);
    }

    #[test]
    fn app_config_hook_rotation_fields_parse() {
        let toml_str = r#"
            hook_rotation_enabled = true
            hook_rotation_interval_ms = 7500
        "#;
        let cfg: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.hook_rotation_enabled);
        assert_eq!(cfg.hook_rotation_interval_ms, 7500);
    }

    #[test]
    fn log_config_defaults() {
        let cfg = AppConfig::default_config();
        assert_eq!(cfg.log.max_size_mb, 100);
        assert_eq!(cfg.log.max_files, 7);
        assert_eq!(cfg.log.max_age_days, 30);
    }

    #[test]
    fn log_config_parses_from_toml() {
        let cfg: AppConfig = toml::from_str(
            r#"
[log]
max_size_mb  = 500
max_files    = 14
max_age_days = 60
"#,
        )
        .unwrap();
        assert_eq!(cfg.log.max_size_mb, 500);
        assert_eq!(cfg.log.max_files, 14);
        assert_eq!(cfg.log.max_age_days, 60);
    }

    #[test]
    fn log_config_partial_override() {
        let cfg: AppConfig = toml::from_str(
            r#"
[log]
max_files = 3
"#,
        )
        .unwrap();
        // Only max_files overridden; others stay at defaults.
        assert_eq!(cfg.log.max_files, 3);
        assert_eq!(cfg.log.max_size_mb, 100);
        assert_eq!(cfg.log.max_age_days, 30);
    }

    #[test]
    fn log_config_zero_disables_pruning() {
        let cfg: AppConfig = toml::from_str(
            r#"
[log]
max_size_mb  = 0
max_age_days = 0
"#,
        )
        .unwrap();
        assert_eq!(cfg.log.max_size_mb, 0);
        assert_eq!(cfg.log.max_age_days, 0);
    }

    #[test]
    fn app_config_timing_correction_defaults() {
        let cfg = AppConfig::default_config();
        assert!(!cfg.timing_correction);
    }

    #[test]
    fn launch_config_defaults() {
        let cfg = LaunchConfig::default();
        assert_eq!(cfg.stagger_min_secs, 3);
        assert_eq!(cfg.stagger_max_secs, 15);
        assert_eq!(cfg.max_concurrent_launches, 3);
        assert_eq!(cfg.max_working_set_mb, 800);
        assert!(cfg.launch_args.is_empty());
    }

    #[test]
    fn server_config_defaults() {
        let cfg = ServerConfig::default();
        assert_eq!(cfg.name, "Firiona Vie");
        assert!(cfg.status_url.is_none());
        assert_eq!(cfg.status_check_timeout_secs, 10);
    }

    #[test]
    fn retry_config_defaults() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.max_retries, 3);
        assert_eq!(cfg.base_backoff_secs, 1);
        assert_eq!(cfg.max_backoff_secs, 60);
        assert!((cfg.backoff_multiplier - 2.0).abs() < f64::EPSILON);
        assert!((cfg.backoff_jitter - 0.1).abs() < f64::EPSILON);
        assert_eq!(cfg.mass_failure_threshold, 5);
        assert_eq!(cfg.mass_failure_window_secs, 60);
    }

    #[test]
    fn peer_discovery_defaults() {
        let cfg = PeerDiscoveryConfig::default();
        assert!(!cfg.multicast_enabled);
        assert_eq!(cfg.bind_addr, "0.0.0.0");
        assert_eq!(cfg.multicast_addr, "239.255.42.99");
        assert_eq!(cfg.port, 35353);
        assert_eq!(cfg.announce_interval_ms, 1_000);
        assert_eq!(cfg.peer_ttl_ms, 5_000);
        assert_eq!(cfg.multicast_ttl, 1);
        assert!(cfg.node_name.is_empty());
    }

    #[test]
    fn app_config_minimal_toml() {
        let toml_str = r#"
            process_name = "test.exe"
            max_spawns = 100
        "#;
        let cfg: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.process_name, "test.exe");
        assert_eq!(cfg.max_spawns, 100);
        // Defaults for nested configs
        assert!(cfg.group.is_empty());
        assert_eq!(cfg.server.name, "Firiona Vie");
    }

    #[test]
    fn load_real_accounts_toml() {
        let path = std::path::Path::new("config/accounts.toml");
        if path.exists() {
            let cfg = AccountsConfig::load(path).unwrap();
            assert!(!cfg.accounts.is_empty());
            // Every account should have a non-empty name
            for acct in &cfg.accounts {
                assert!(!acct.name.is_empty());
                assert!(!acct.server.is_empty());
            }
        }
    }

    #[test]
    fn discord_config_defaults() {
        let cfg = DiscordConfig::default();
        assert!(cfg.webhook_url.is_empty());
        assert!(cfg.channels.is_empty());
        assert!(cfg.alert_hvt);
        assert!(cfg.alert_crashes);
        assert!(cfg.alert_mass_failures);
        assert!(!cfg.alert_status);
        assert!(cfg.command_allowed_senders.is_empty());
        assert!(cfg.chat_channels.is_empty());
    }

    #[test]
    fn discord_config_chat_channels_parsed() {
        let toml_str = r#"
            [discord]
            webhook_url = "https://example.com/webhook"

            [discord.chat_channels]
            group = "https://example.com/group"
            raid  = "https://example.com/raid"
        "#;
        let cfg: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.discord.chat_channels.len(), 2);
        assert_eq!(
            cfg.discord.chat_channels.get("group").unwrap(),
            "https://example.com/group"
        );
        assert_eq!(
            cfg.discord.chat_channels.get("raid").unwrap(),
            "https://example.com/raid"
        );
        assert!(!cfg.discord.chat_channels.contains_key("guild"));
    }

    #[test]
    fn accounts_for_group_returns_empty_for_no_match() {
        let cfg = parse_sample();
        let g0 = cfg.accounts_for_group(0);
        assert!(g0.is_empty());
    }

    #[test]
    fn find_account_returns_correct_entry() {
        let cfg = parse_sample();
        let acct = cfg.find_account("dmft02").unwrap();
        assert_eq!(acct.character, "Zisdarenu");
        assert_eq!(acct.class, "SHM");
        assert_eq!(acct.group, 1);
    }

    #[test]
    fn to_account_info_level_defaults_to_one() {
        let cfg = parse_sample();
        let info = AccountsConfig::to_account_info(&cfg.accounts[0]);
        assert_eq!(info.level, 1);
    }

    #[test]
    fn app_config_default_has_empty_eq_path() {
        let cfg = AppConfig::default_config();
        assert!(cfg.launch.eq_path.is_empty());
    }

    #[test]
    fn app_config_full_toml_parse() {
        let toml_str = r#"
            process_name = "custom.exe"
            max_spawns = 500

            [server]
            name = "TestServer"
            status_check_timeout_secs = 30

            [retry]
            max_retries = 5
            base_backoff_secs = 10
            mass_failure_threshold = 3
            mass_failure_window_secs = 120

            [launch]
            eq_path = "C:/EQ"
            stagger_min_secs = 5
            stagger_max_secs = 20
            max_concurrent_launches = 6
            max_working_set_mb = 1024

            [discord]
            webhook_url = "https://example.com/webhook"
            alert_hvt = false
            alert_crashes = true
            alert_mass_failures = false
            alert_status = true
            command_allowed_senders = ["RaidLead", "OfficerBot"]

            [discord.channels]
            kills = "https://example.com/kills"
            loot = "https://example.com/loot"
            timers = "https://example.com/timers"
            feats = "https://example.com/feats"

            [discovery]
            multicast_enabled = true
            bind_addr = "0.0.0.0"
            multicast_addr = "239.255.42.123"
            port = 39001
            announce_interval_ms = 2500
            peer_ttl_ms = 9000
            node_name = "basement-rig"
            multicast_ttl = 2
        "#;
        let cfg: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.process_name, "custom.exe");
        assert_eq!(cfg.max_spawns, 500);
        assert_eq!(cfg.server.name, "TestServer");
        assert_eq!(cfg.server.status_check_timeout_secs, 30);
        assert_eq!(cfg.retry.max_retries, 5);
        assert_eq!(cfg.retry.base_backoff_secs, 10);
        assert_eq!(cfg.launch.eq_path, "C:/EQ");
        assert_eq!(cfg.launch.max_concurrent_launches, 6);
        assert_eq!(cfg.launch.max_working_set_mb, 1024);
        assert_eq!(cfg.discord.webhook_url, "https://example.com/webhook");
        assert!(!cfg.discord.alert_hvt);
        assert!(cfg.discord.alert_crashes);
        assert!(!cfg.discord.alert_mass_failures);
        assert!(cfg.discord.alert_status);
        assert_eq!(cfg.discord.command_allowed_senders.len(), 2);
        assert_eq!(cfg.discord.command_allowed_senders[0], "RaidLead");
        assert_eq!(cfg.discord.channels.len(), 4);
        assert_eq!(
            cfg.discord.channels.get("kills").unwrap(),
            "https://example.com/kills"
        );
        assert!(cfg.discovery.multicast_enabled);
        assert_eq!(cfg.discovery.multicast_addr, "239.255.42.123");
        assert_eq!(cfg.discovery.port, 39001);
        assert_eq!(cfg.discovery.announce_interval_ms, 2500);
        assert_eq!(cfg.discovery.peer_ttl_ms, 9000);
        assert_eq!(cfg.discovery.node_name, "basement-rig");
        assert_eq!(cfg.discovery.multicast_ttl, 2);
    }

    #[test]
    fn app_config_with_groups() {
        let toml_str = r#"
            [[group]]
            id = 1
            name = "Group Alpha"

            [[group.toon]]
            name = "Tank"
            class = "WAR"
            role = "tank"

            [[group.toon]]
            name = "Healer"
            class = "CLR"
            role = "healer"
        "#;
        let cfg: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.group.len(), 1);
        assert_eq!(cfg.group[0].id, 1);
        assert_eq!(cfg.group[0].name, "Group Alpha");
        assert_eq!(cfg.group[0].toon.len(), 2);
        assert_eq!(cfg.group[0].toon[0].name, "Tank");
        assert_eq!(cfg.group[0].toon[0].role, "tank");
    }

    #[test]
    fn toon_config_optional_fields_default() {
        let toml_str = r#"
            [[group]]
            id = 1
            name = "Test"

            [[group.toon]]
            name = "Foo"
            class = "WAR"
            role = "dps"
        "#;
        let cfg: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.group[0].toon[0].eq_window_title.is_empty());
        assert!(cfg.group[0].toon[0].account.is_none());
    }

    #[test]
    fn default_process_name_fn() {
        assert_eq!(default_process_name(), "eqgame.exe");
    }

    #[test]
    fn default_max_spawns_fn() {
        assert_eq!(default_max_spawns(), 2048);
    }

    // ─── Profile Group Tests ────────────────────────────────────────────────

    const SAMPLE_WITH_PROFILES: &str = r#"
[[accounts]]
name = "dmft01"
server = "Firiona Vie"
character = "Camrene"
class = "WAR"
group = 1

[[accounts]]
name = "dmft02"
server = "Firiona Vie"
character = "Zisdarenu"
class = "SHM"
group = 1

[[accounts]]
name = "dmft07"
server = "Firiona Vie"
character = "Paladin"
class = "PAL"
group = 2

[[profile_groups]]
id = 1
name = "MainRaid"
hotkey = "F1"

[[profile_groups]]
id = 2
name = "SecondRaid"
hotkey = "F2"

[[profile_groups]]
id = 3
name = "AltGroup"

[[camera_presets]]
name = "First Person"
hotkey = "F5"
pitch = 0.0
yaw = 0.0
is_default = true

[[camera_presets]]
name = "Close"
hotkey = "F6"
distance = 15.0

[[camera_presets]]
name = "Far"
hotkey = "F7"
distance = 100.0

[[camera_presets]]
name = "Overhead"
hotkey = "F8"
pitch = 60.0
"#;

    fn parse_with_profiles() -> AccountsConfig {
        toml::from_str(SAMPLE_WITH_PROFILES).unwrap()
    }

    #[test]
    fn parse_profile_groups() {
        let cfg = parse_with_profiles();
        assert_eq!(cfg.profile_groups.len(), 3);
        assert_eq!(cfg.profile_groups[0].id, 1);
        assert_eq!(cfg.profile_groups[0].name, "MainRaid");
        assert_eq!(cfg.profile_groups[0].hotkey, Some("F1".to_string()));
        assert_eq!(cfg.profile_groups[1].id, 2);
        assert_eq!(cfg.profile_groups[1].name, "SecondRaid");
        assert_eq!(cfg.profile_groups[1].hotkey, Some("F2".to_string()));
        assert_eq!(cfg.profile_groups[2].id, 3);
        assert_eq!(cfg.profile_groups[2].name, "AltGroup");
        assert!(cfg.profile_groups[2].hotkey.is_none());
    }

    #[test]
    fn profile_groups_empty_when_not_in_toml() {
        let cfg = parse_sample();
        assert!(cfg.profile_groups.is_empty());
    }

    #[test]
    fn profile_by_name_case_insensitive() {
        let cfg = parse_with_profiles();
        assert!(cfg.profile_by_name("mainraid").is_some());
        assert!(cfg.profile_by_name("MAINRAID").is_some());
        assert!(cfg.profile_by_name("MainRaid").is_some());
        assert_eq!(cfg.profile_by_name("MainRaid").unwrap().id, 1);
        assert!(cfg.profile_by_name("NoSuchProfile").is_none());
    }

    #[test]
    fn profile_by_hotkey_case_insensitive() {
        let cfg = parse_with_profiles();
        assert!(cfg.profile_by_hotkey("f1").is_some());
        assert!(cfg.profile_by_hotkey("F1").is_some());
        assert_eq!(cfg.profile_by_hotkey("F1").unwrap().name, "MainRaid");
        assert!(cfg.profile_by_hotkey("F2").is_some());
        assert_eq!(cfg.profile_by_hotkey("F2").unwrap().name, "SecondRaid");
        assert!(cfg.profile_by_hotkey("F9").is_none());
    }

    #[test]
    fn profile_by_hotkey_no_hotkey_returns_none() {
        let cfg = parse_with_profiles();
        // AltGroup has no hotkey
        assert!(cfg.profile_by_hotkey("").is_none());
    }

    #[test]
    fn accounts_for_profile_name_returns_correct_accounts() {
        let cfg = parse_with_profiles();
        let accts = cfg.accounts_for_profile_name("MainRaid").unwrap();
        assert_eq!(accts.len(), 2);
        assert!(accts.iter().all(|a| a.group == 1));

        let accts2 = cfg.accounts_for_profile_name("SecondRaid").unwrap();
        assert_eq!(accts2.len(), 1);
        assert_eq!(accts2[0].name, "dmft07");
    }

    #[test]
    fn accounts_for_profile_name_unknown_returns_none() {
        let cfg = parse_with_profiles();
        assert!(cfg.accounts_for_profile_name("NoSuchProfile").is_none());
    }

    #[test]
    fn accounts_for_profile_name_empty_group_returns_empty_vec() {
        let cfg = parse_with_profiles();
        // AltGroup (id=3) exists but no accounts have group=3
        let accts = cfg.accounts_for_profile_name("AltGroup").unwrap();
        assert!(accts.is_empty());
    }

    #[test]
    fn parse_camera_presets() {
        let cfg = parse_with_profiles();
        assert_eq!(cfg.camera_presets.len(), 4);
        assert_eq!(cfg.camera_presets[0].name, "First Person");
        assert_eq!(cfg.camera_presets[0].hotkey, Some("F5".to_string()));
        assert!(cfg.camera_presets[0].is_default);
        assert_eq!(cfg.camera_presets[1].name, "Close");
        assert_eq!(cfg.camera_presets[2].name, "Far");
        assert_eq!(cfg.camera_presets[3].name, "Overhead");
    }

    #[test]
    fn camera_preset_by_hotkey_case_insensitive() {
        let cfg = parse_with_profiles();
        assert!(cfg.camera_preset_by_hotkey("f5").is_some());
        assert!(cfg.camera_preset_by_hotkey("F5").is_some());
        assert_eq!(
            cfg.camera_preset_by_hotkey("F5").unwrap().name,
            "First Person"
        );
        assert!(cfg.camera_preset_by_hotkey("F9").is_none());
    }

    // ─── Account Loader / Credential Validation Tests ──────────────────────

    /// Write a temporary accounts.toml with the given TOML content and return
    /// its path.
    fn write_temp_accounts_toml(content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("accounts.toml");
        std::fs::write(&path, content).expect("write temp accounts.toml");
        (dir, path)
    }

    #[test]
    fn load_all_accounts_returns_all_entries() {
        let (_dir, path) = write_temp_accounts_toml(SAMPLE_ACCOUNTS_TOML);
        let accounts = AccountsConfig::load_all_accounts(&path).unwrap();
        assert_eq!(accounts.len(), 3);
        let names: Vec<&str> = accounts.iter().map(|a| a.account_name.as_str()).collect();
        assert!(names.contains(&"dmft01"));
        assert!(names.contains(&"dmft02"));
        assert!(names.contains(&"dmft07"));
    }

    #[test]
    fn load_all_accounts_empty_toml_returns_empty_vec() {
        let (_dir, path) = write_temp_accounts_toml("");
        let accounts = AccountsConfig::load_all_accounts(&path).unwrap();
        assert!(accounts.is_empty());
    }

    #[test]
    fn load_all_accounts_missing_file_returns_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nonexistent.toml");
        let result = AccountsConfig::load_all_accounts(&path);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("nonexistent.toml") || msg.contains("Failed to read"));
    }

    #[test]
    fn load_accounts_for_profile_returns_correct_group() {
        let (_dir, path) = write_temp_accounts_toml(SAMPLE_WITH_PROFILES);
        let accounts = AccountsConfig::load_accounts_for_profile(&path, "MainRaid").unwrap();
        assert_eq!(accounts.len(), 2);
        assert!(accounts.iter().all(|a| a.group_id == 1));
    }

    #[test]
    fn load_accounts_for_profile_case_insensitive() {
        let (_dir, path) = write_temp_accounts_toml(SAMPLE_WITH_PROFILES);
        let lower = AccountsConfig::load_accounts_for_profile(&path, "mainraid").unwrap();
        let upper = AccountsConfig::load_accounts_for_profile(&path, "MAINRAID").unwrap();
        assert_eq!(lower.len(), upper.len());
    }

    #[test]
    fn load_accounts_for_profile_missing_profile_returns_error() {
        let (_dir, path) = write_temp_accounts_toml(SAMPLE_WITH_PROFILES);
        let result = AccountsConfig::load_accounts_for_profile(&path, "NoSuchProfile");
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("NoSuchProfile"));
    }

    #[test]
    fn load_accounts_for_profile_empty_group_returns_empty_vec() {
        let (_dir, path) = write_temp_accounts_toml(SAMPLE_WITH_PROFILES);
        // AltGroup (id=3) has no accounts assigned to it
        let accounts = AccountsConfig::load_accounts_for_profile(&path, "AltGroup").unwrap();
        assert!(accounts.is_empty());
    }

    // ─── validate_credentials_exist tests (Windows-only) ───────────────────

    #[cfg(windows)]
    fn make_account_info(name: &str) -> textquest_common::login::AccountInfo {
        textquest_common::login::AccountInfo {
            account_name: name.to_string(),
            character_name: String::new(),
            class_name: String::new(),
            level: 1,
            group_id: 0,
            server_name: String::new(),
        }
    }

    #[cfg(windows)]
    fn open_test_store() -> crate::credentials::store::CredentialStore {
        use crate::credentials::crypto;
        use std::path::PathBuf;
        let salt = crypto::generate_salt();
        let key = crypto::derive_key("test_pw", &salt).unwrap();
        crate::credentials::store::CredentialStore::open(&PathBuf::from(":memory:"), key).unwrap()
    }

    #[cfg(windows)]
    #[test]
    fn validate_credentials_exist_passes_when_all_present() {
        let store = open_test_store();
        store.add_account("dmft01", "pw1").unwrap();
        store.add_account("dmft02", "pw2").unwrap();
        let accounts = vec![make_account_info("dmft01"), make_account_info("dmft02")];
        let result = AccountsConfig::validate_credentials_exist(&accounts, &store);
        assert!(result.is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn validate_credentials_exist_fails_for_missing_account() {
        let store = open_test_store();
        store.add_account("dmft01", "pw1").unwrap();
        let accounts = vec![make_account_info("dmft01"), make_account_info("dmft99")];
        let result = AccountsConfig::validate_credentials_exist(&accounts, &store);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("dmft99"));
    }

    #[cfg(windows)]
    #[test]
    fn validate_credentials_exist_case_insensitive() {
        let store = open_test_store();
        store.add_account("DMFT01", "pw1").unwrap();
        // Config might store it as lowercase — check case-insensitive match
        let accounts = vec![make_account_info("dmft01")];
        let result = AccountsConfig::validate_credentials_exist(&accounts, &store);
        assert!(result.is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn validate_credentials_exist_passes_for_empty_account_list() {
        let store = open_test_store();
        let result = AccountsConfig::validate_credentials_exist(&[], &store);
        assert!(result.is_ok());
    }
}
