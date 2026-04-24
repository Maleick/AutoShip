//! Orchestrator — wires the camp loop state machine to IPC command delivery.

/// Cross-group emergency coordination for same-zone rez and assist flows.
pub mod cross_group;
/// Session and group control model for the orchestrator.
pub mod session_control;
/// Cross-group outside-group assist — MQ2XAssist parity.
pub mod xassist;

use self::{
    cross_group::CrossGroupCoordinator,
    session_control::{SessionControl, build_admin_session_inventory},
    xassist::{XAssist, XAssistConfig},
};
use crate::{
    camp::{
        cc::CcType,
        config::CampConfig,
        hunt::{HuntLoop, HuntSnapshot, OperatingMode, Pos2D},
        progression::{CampDatabase, CampProgressionEvent, check_progression},
        state::{CampAction, CampEvent, CampLoop, CampMember, CampSnapshot, CampState, Role},
        vendor::{SellCycle, SellState, VendorConfig, VendorObservation},
    },
    chat_log::{ChatChannel, ChatLogConfig, ChatLogManager},
    combat::coordinator::CombatCoordinator,
    economy::price_monitor::TradePriceMonitor,
    ipc::{pipe::CommandPipe, shared::SharedStateReader},
    loot::vendor_cycle::VendorInventoryItem,
    metrics::{
        AdminMonitoringStore, MetricsCollector, SessionErrorKind, SessionMonitoringSnapshot,
    },
    say_detection::{SayAction, SayDetector, SayPattern, SayRule},
};
use std::{
    collections::HashMap,
    env, fs, io,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};
use textquest_common::{
    character_config::{CharacterConfigMap, RewardAutomationConfig, load_character_configs},
    combat::HateTargetCategory,
    ipc::{ChatMessageInfo, Command, Response, SessionControlCommand, SessionToken},
    routing::RoutingScope,
    shared_client_state::{SharedClientState, extended_state_enabled},
    spawn_finder::{LiveSpawnObserver, LiveSpawnSnapshot},
    types::{ClientId, GameState},
};

/// Generate a cryptographically random 32-byte session token using OS entropy.
#[allow(dead_code)] // Used when IPC is wired up in later milestones
fn generate_session_token() -> SessionToken {
    textquest_common::ipc::generate_random_token()
}

/// Maximum number of ticks a critical role's state can be stale before
/// `build_camp_snapshot` refuses to produce a snapshot.
const STALE_TICK_THRESHOLD: u64 = 3;

/// How often (in ticks) to check camp progression for level-based advances.
const PROGRESSION_CHECK_INTERVAL: u64 = 50;

/// Ticks before CC expiry to push a `CcExpiring` event.
const CC_EXPIRY_BUFFER: u64 = 3;
/// How often to check the persisted character config file for reward updates.
const REWARD_CONFIG_SYNC_INTERVAL: u64 = 50;

/// Returns the runtime data root: `TEXTQUEST_DATA_DIR`, exe parent, then cwd fallback.
fn data_dir() -> PathBuf {
    if let Ok(dir) = env::var("TEXTQUEST_DATA_DIR")
        && !dir.trim().is_empty()
    {
        return PathBuf::from(dir);
    }

    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn character_config_path() -> PathBuf {
    data_dir().join("config/character-configs.json")
}

fn live_session_snapshot_path() -> PathBuf {
    data_dir().join("data/runtime/live_sessions.json")
}

fn admin_session_snapshot_path() -> PathBuf {
    live_session_snapshot_path().with_file_name("admin_sessions.json")
}

fn live_spawn_snapshot_path() -> PathBuf {
    live_session_snapshot_path().with_file_name("live_spawns.json")
}

fn replace_snapshot_file(temp_path: &Path, path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path)?;
    }

    fs::rename(temp_path, path)
}

fn persist_shared_client_states_to_path(
    path: &Path,
    states: &[SharedClientState],
) -> io::Result<()> {
    persist_json_to_path(path, states)
}

fn persist_live_spawn_snapshot_to_path(
    path: &Path,
    snapshot: &LiveSpawnSnapshot,
) -> io::Result<()> {
    persist_json_to_path(path, snapshot)
}

fn persist_json_to_path<T: serde::Serialize + ?Sized>(path: &Path, payload: &T) -> io::Result<()> {
    let payload =
        serde_json::to_vec(payload).map_err(|error| io::Error::other(error.to_string()))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, payload)?;
    replace_snapshot_file(&temp_path, path)?;
    Ok(())
}

/// Poll interval for passive trade-chat capture.
const TRADE_CHAT_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Top-level orchestrator that ticks the camp loop and dispatches commands.
pub struct Orchestrator {
    /// Process IDs of all registered EQ clients.
    pub client_pids: Vec<u32>,
    /// Mapping of PID to character name for each client.
    pub client_names: HashMap<u32, String>,
    /// Mapping of PID to group ID for per-client coordination.
    pub client_groups: HashMap<u32, u8>,
    /// Mapping of PID to class name for per-client metadata.
    pub client_class_names: HashMap<u32, String>,
    /// Active camp loop state machine, if a camp is running.
    pub active_camp: Option<CampLoop>,
    /// Group combat coordinator (assist, CC, CH chain).
    pub combat: CombatCoordinator,
    /// Monotonically increasing tick counter.
    pub tick_count: u64,
    /// Commands dispatched this tick (for status display).
    pub last_dispatched: Vec<(u32, CampAction)>,
    /// Latest game state per client PID.
    pub game_states: HashMap<u32, GameState>,
    /// Shared memory readers per client PID.
    state_readers: HashMap<u32, SharedStateReader>,
    /// CSPRNG session tokens per client PID (generated at registration time).
    session_tokens: HashMap<u32, SessionToken>,
    /// Session routing/group metadata for each live client.
    session_controls: HashMap<u32, SessionControl>,
    /// Tick number when each client's game state was last updated.
    state_timestamps: HashMap<u32, u64>,
    /// Last broadcast cross-client roster snapshot.
    last_shared_client_states: Vec<SharedClientState>,
    /// Tracks whether the current process has persisted the live session file.
    persisted_shared_client_states: bool,
    /// Last persisted spawn finder snapshot for the web dashboard.
    last_live_spawn_snapshot: LiveSpawnSnapshot,
    /// Tracks whether the current process has persisted the live spawn file.
    persisted_live_spawn_snapshot: bool,
    /// Stable admin-monitoring store reused by later diagnostics surfaces.
    monitoring: AdminMonitoringStore,
    /// Real-time fleet metrics collector fed from game state, IPC, loot, and
    /// combat-event hooks.
    metrics_collector: MetricsCollector,
    // Pipe connections are created per-command (connect → token → command → drop).
    // The DLL's pipe server disconnects after each command, so persistent
    // connections would fail on the second write.

    // --- Integration fields ---
    /// Current operating mode: Camp (stationary) or Hunt (roaming).
    pub operating_mode: OperatingMode,
    /// Active hunt loop (used when `operating_mode` == Hunt).
    pub active_hunt: Option<HuntLoop>,
    /// Vendor sell cycle (ticked during camp Idle/Medding).
    pub sell_cycle: Option<SellCycle>,
    /// Latest inventory snapshot used to build vendor plans.
    vendor_inventory: Vec<VendorInventoryItem>,
    /// Camp progression database for level-based camp advancement.
    pub camp_db: Option<CampDatabase>,
    /// Suggested camp from progression check (for TUI display).
    pub suggested_camp: Option<String>,
    /// Previous CC state snapshot for charm break detection (`spawn_id` ->
    /// `CcType`).
    prev_cc_state: HashMap<u32, CcType>,
    /// Previous nearby spawn IDs for add detection.
    prev_nearby_spawns: HashMap<u32, String>,
    /// Path to the persisted character configuration file.
    character_config_path: PathBuf,
    /// Last observed modification time for the character config file.
    character_config_mtime: Option<SystemTime>,
    /// Cached character configs loaded from disk.
    cached_character_configs: CharacterConfigMap,
    /// Last reward automation config sent to each live client.
    last_sent_reward_configs: HashMap<u32, RewardAutomationConfig>,
    /// Lazy-initialized passive trade-price monitor.
    trade_price_monitor: Option<TradePriceMonitor>,
    /// Prevent repeated warning spam when the local trade-price DB cannot be
    /// opened.
    trade_price_monitor_open_failed: bool,
    /// Last time passive trade chat was polled from the DLL.
    last_trade_chat_poll: Option<Instant>,
    /// Chat log manager for per-character log files (MQ2Log parity).
    chat_log_manager: Option<ChatLogManager>,
    /// Last time chat was polled for logging.
    last_chat_log_poll: Option<Instant>,
    /// Poll interval for chat logging.
    chat_log_poll_interval: Duration,
    /// Same-zone cross-group rescue coordinator.
    cross_group: CrossGroupCoordinator,
    /// Cross-group outside-group assist (MQ2XAssist parity).
    xassist: XAssist,
    /// Say channel detection and alerting (MQ2Say parity).
    say_detector: SayDetector,
    /// Alert routing and delivery configuration for say detection.
    say_detection_config: crate::config::SayDetectionConfig,
    /// Optional Discord webhook sender for say alerts.
    say_detection_webhook: Option<crate::discord::webhook::WebhookSender>,

    // --- M8 Orchestrator routing ---
    /// Active routing scope (synced from TUI `App::routing_scope` each tick).
    ///
    /// Determines which client PIDs receive camp-loop dispatches.  Defaults to
    /// `AllSession` so all registered clients are targeted until the operator
    /// narrows the scope via `:scope`.
    pub routing_scope: RoutingScope,
    /// Pre-computed set of in-scope PIDs (synced from `App::focused_pids()`).
    ///
    /// The TUI app owns the canonical scope-to-PID mapping (including account-
    /// range-based group membership).  The orchestrator consumes this list
    /// directly instead of duplicating the mapping logic.
    pub scope_pids: Vec<u32>,
}

impl Orchestrator {
    /// Create a new orchestrator with no registered clients.
    #[must_use]
    pub fn new() -> Self {
        let orchestrator = Self {
            client_pids: Vec::new(),
            client_names: HashMap::new(),
            client_groups: HashMap::new(),
            client_class_names: HashMap::new(),
            active_camp: None,
            combat: CombatCoordinator::new(),
            tick_count: 0,
            last_dispatched: Vec::new(),
            game_states: HashMap::new(),
            state_readers: HashMap::new(),
            session_tokens: HashMap::new(),
            session_controls: HashMap::new(),
            state_timestamps: HashMap::new(),
            last_shared_client_states: Vec::new(),
            persisted_shared_client_states: false,
            last_live_spawn_snapshot: LiveSpawnSnapshot::default(),
            persisted_live_spawn_snapshot: false,
            monitoring: AdminMonitoringStore::new(),
            metrics_collector: MetricsCollector::new(),
            operating_mode: OperatingMode::Camp,
            active_hunt: None,
            sell_cycle: None,
            vendor_inventory: Vec::new(),
            camp_db: None,
            suggested_camp: None,
            prev_cc_state: HashMap::new(),
            prev_nearby_spawns: HashMap::new(),
            character_config_path: character_config_path(),
            character_config_mtime: None,
            cached_character_configs: HashMap::new(),
            last_sent_reward_configs: HashMap::new(),
            trade_price_monitor: None,
            trade_price_monitor_open_failed: false,
            last_trade_chat_poll: None,
            chat_log_manager: None,
            last_chat_log_poll: None,
            chat_log_poll_interval: Duration::from_secs(1),
            cross_group: CrossGroupCoordinator::new(),
            xassist: XAssist::new(),
            say_detector: SayDetector::new(),
            say_detection_config: crate::config::SayDetectionConfig::default(),
            say_detection_webhook: None,
            routing_scope: RoutingScope::AllSession,
            scope_pids: Vec::new(),
        };
        orchestrator.persist_admin_session_inventory();
        orchestrator
    }

    fn build_shared_client_states(&self) -> Vec<SharedClientState> {
        let include_extended = extended_state_enabled();
        self.client_pids
            .iter()
            .filter_map(|pid| {
                let state = self.game_states.get(pid)?;
                SharedClientState::from_game_state(
                    self.client_names.get(pid).map(String::as_str),
                    state,
                    include_extended,
                )
            })
            .collect()
    }

    fn sync_shared_client_states(&mut self) {
        let states = self.build_shared_client_states();
        if states == self.last_shared_client_states {
            return;
        }

        let recipients = self.client_pids.clone();
        let mut all_sent = true;
        for pid in recipients {
            all_sent &= self.send_ipc_command(
                pid,
                Command::UpdateSharedClientStates {
                    states: states.clone(),
                },
            );
        }

        if all_sent {
            self.last_shared_client_states = states;
        }
    }

    /// Get the latest game state for a client PID.
    #[allow(dead_code)]
    #[must_use]
    pub fn get_client_state(&self, pid: u32) -> Option<&GameState> {
        self.game_states.get(&pid)
    }

    /// Read game state from shared memory for all known clients.
    fn poll_game_states(&mut self) {
        for &pid in &self.client_pids {
            // Lazily create readers
            if let std::collections::hash_map::Entry::Vacant(e) = self.state_readers.entry(pid) {
                let session_id = self
                    .session_tokens
                    .get(&pid)
                    .map_or(0, textquest_common::ipc::session_id_from_token);
                match SharedStateReader::new(pid, session_id) {
                    Ok(reader) => {
                        e.insert(reader);
                    }
                    Err(e) => {
                        tracing::debug!(pid, error = %e, "Failed to open shared memory reader");
                        continue;
                    }
                }
            }

            if let Some(reader) = self.state_readers.get_mut(&pid)
                && let Some(state) = reader.read()
            {
                self.game_states.insert(pid, state);
                self.state_timestamps.insert(pid, self.tick_count);
            }
        }
    }

    fn build_live_spawn_snapshot(&self) -> LiveSpawnSnapshot {
        LiveSpawnSnapshot {
            observers: self
                .client_pids
                .iter()
                .filter_map(|pid| {
                    let state = self.game_states.get(pid)?;
                    LiveSpawnObserver::from_game_state(
                        self.client_names.get(pid).map(String::as_str),
                        state,
                    )
                })
                .collect(),
        }
    }

    fn sync_runtime_snapshots(&mut self) {
        let states = self.build_shared_client_states();
        if states != self.last_shared_client_states || !self.persisted_shared_client_states {
            let recipients = self.client_pids.clone();
            for pid in recipients {
                self.send_ipc_command(
                    pid,
                    Command::UpdateSharedClientStates {
                        states: states.clone(),
                    },
                );
            }

            self.persisted_shared_client_states = match persist_shared_client_states_to_path(
                &live_session_snapshot_path(),
                &states,
            ) {
                Ok(()) => true,
                Err(error) => {
                    tracing::warn!(%error, "Failed to persist live session snapshot");
                    false
                }
            };

            self.last_shared_client_states = states;
        }

        let spawn_snapshot = self.build_live_spawn_snapshot();
        if spawn_snapshot != self.last_live_spawn_snapshot || !self.persisted_live_spawn_snapshot {
            self.persisted_live_spawn_snapshot = match persist_live_spawn_snapshot_to_path(
                &live_spawn_snapshot_path(),
                &spawn_snapshot,
            ) {
                Ok(()) => true,
                Err(error) => {
                    tracing::warn!(%error, "Failed to persist live spawn snapshot");
                    false
                }
            };

            self.last_live_spawn_snapshot = spawn_snapshot;
        }
    }

    fn persist_admin_session_inventory(&self) {
        let inventory = build_admin_session_inventory(
            &self.session_controls,
            &self.client_names,
            &self.client_class_names,
        );
        if let Err(error) = persist_json_to_path(&admin_session_snapshot_path(), &inventory) {
            tracing::warn!(%error, "Failed to persist admin session inventory");
        }
    }

    fn refresh_character_configs_if_changed(&mut self) {
        let modified = std::fs::metadata(&self.character_config_path)
            .and_then(|metadata| metadata.modified())
            .ok();
        if modified == self.character_config_mtime {
            return;
        }

        match load_character_configs(&self.character_config_path) {
            Ok(configs) => {
                self.cached_character_configs = configs;
                self.character_config_mtime = modified;
            }
            Err(error) => {
                tracing::warn!(
                    %error,
                    path = %self.character_config_path.display(),
                    "Failed to reload character configs for reward automation; keeping last known good configs"
                );
            }
        }
    }

    pub(crate) fn sync_reward_automation_configs(&mut self) {
        self.refresh_character_configs_if_changed();

        let known_pids = self.client_pids.clone();
        self.last_sent_reward_configs
            .retain(|pid, _| known_pids.contains(pid));

        for pid in known_pids {
            let Some(character_name) = self.client_names.get(&pid).cloned() else {
                continue;
            };

            let config = self
                .cached_character_configs
                .get(&character_name)
                .map(|cfg| cfg.reward_automation.clone())
                .unwrap_or_default();

            if self.last_sent_reward_configs.get(&pid) == Some(&config) {
                continue;
            }

            // Only mark the config as "sent" if IPC actually delivered it.
            // Otherwise a transient pipe failure during client startup would
            // permanently starve the DLL's reward automation path, because
            // the cache short-circuits future sync passes until the config
            // changes again or the client is removed.
            let dispatched = self.send_ipc_command(
                pid,
                Command::SetRewardAutomation {
                    config: config.clone(),
                },
            );
            if dispatched {
                self.last_sent_reward_configs.insert(pid, config);
            } else {
                // Drop any stale entry so the next pass retries.
                self.last_sent_reward_configs.remove(&pid);
            }
        }
    }
    /// Build a `CampSnapshot` from live game state for the active camp's
    /// members. Returns `None` if any critical role (tank/healer) has stale
    /// state.
    fn build_camp_snapshot(&self) -> Option<CampSnapshot> {
        let camp = self.active_camp.as_ref()?;

        let tank = camp.members.iter().find(|m| m.role == Role::Tank)?;
        let healer = camp.members.iter().find(|m| m.role == Role::Healer)?;

        // Staleness check: refuse to act on data older than STALE_TICK_THRESHOLD ticks
        for critical in [tank, healer] {
            if let Some(&last_update) = self.state_timestamps.get(&critical.pid)
                && self.tick_count.saturating_sub(last_update) > STALE_TICK_THRESHOLD
            {
                tracing::warn!(
                    pid = critical.pid,
                    name = %critical.name,
                    role = ?critical.role,
                    stale_ticks = self.tick_count - last_update,
                    "Stale game state for critical role — skipping snapshot"
                );
                return None;
            }
            // No timestamp at all means we never read state — handled by get()
            // below
        }

        let tank_state = self.game_states.get(&tank.pid)?;
        let healer_state = self.game_states.get(&healer.pid)?;

        let tank_hp_pct = tank_state
            .local_player
            .as_ref()
            .map_or(100.0, textquest_common::types::SpawnData::hp_pct);

        let healer_mana_pct = healer_state
            .local_player
            .as_ref()
            .map_or(100.0, textquest_common::types::SpawnData::mana_pct);

        // Use the tank's target for target HP and spawn ID
        let (target_hp_pct, target_is_dead, target_spawn_id) =
            tank_state.target.as_ref().map_or((None, false, None), |t| {
                (Some(t.hp_pct()), t.hp_current <= 0, Some(t.spawn_id))
            });

        // Collect per-member HP for death detection
        let member_hp: Vec<(u32, i32)> = camp
            .members
            .iter()
            .filter_map(|m| {
                self.game_states.get(&m.pid).and_then(|gs| {
                    gs.local_player
                        .as_ref()
                        .map(|lp| (m.pid, lp.hp_current.clamp(0, i64::from(i32::MAX)) as i32))
                })
            })
            .collect();

        // Build per-member combat state from game state.
        // A character is considered "in combat" if their Combatant FSM is not
        // idle/recovering.
        let member_in_combat: Vec<(u32, bool)> = camp
            .members
            .iter()
            .filter_map(|m| {
                self.game_states.get(&m.pid).map(|gs| {
                    let in_combat = !matches!(
                        gs.combat_status,
                        textquest_common::combat::CombatStatus::Idle
                            | textquest_common::combat::CombatStatus::Recovering
                    );
                    (m.pid, in_combat)
                })
            })
            .collect();

        Some(CampSnapshot {
            healer_mana_pct,
            tank_hp_pct,
            target_hp_pct,
            target_is_dead,
            target_spawn_id,
            member_hp,
            member_in_combat,
        })
    }

    /// Advance the camp/hunt loop (if active), collect commands, and send via
    /// IPC. Returns the number of commands dispatched.
    pub fn tick(&mut self) -> usize {
        self.tick_count += 1;
        self.last_dispatched.clear();

        self.poll_game_states();
        self.metrics_collector
            .collect_game_state_snapshots(&self.client_names, &self.game_states);
        self.metrics_collector.tick();
        if self.tick_count == 1 || self.tick_count.is_multiple_of(REWARD_CONFIG_SYNC_INTERVAL) {
            self.sync_reward_automation_configs();
        }
        self.sync_runtime_snapshots();
        self.poll_trade_chat_if_due();
        self.poll_chat_log_if_due();
        let say_matches = self.poll_and_evaluate_say_detection();

        let commands = match self.operating_mode {
            OperatingMode::Camp => self.tick_camp(),
            OperatingMode::Hunt => self.tick_hunt(),
        };
        // Filter to in-scope PIDs so the operator's routing scope is respected
        // by camp/hunt loop commands just as it is for TUI-initiated commands.
        let in_scope = self.pids_in_scope();
        let scoped: Vec<_> = commands
            .into_iter()
            .filter(|(pid, _)| in_scope.is_empty() || in_scope.contains(pid))
            .collect();
        let xassist_commands = self.xassist.tick(&self.game_states);
        let emergency = self.cross_group.tick(
            &self.session_controls,
            &self.client_names,
            &self.client_class_names,
            &self.game_states,
            &self.state_timestamps,
            self.tick_count,
        );

        let count = scoped.len() + emergency.len() + xassist_commands.len();
        for (pid, action) in &scoped {
            self.dispatch_action(*pid, action);
        }
        for (pid, action) in &emergency {
            self.dispatch_action(*pid, action);
        }
        for (pid, cmd) in &xassist_commands {
            let xassist::AssistCommand::Target(spawn_id) = cmd;
            self.send_ipc_command(
                *pid,
                Command::SetTarget {
                    spawn_id: *spawn_id,
                },
            );
        }
        self.last_dispatched = scoped;
        self.last_dispatched.extend(emergency);
        self.last_dispatched
            .extend(xassist_commands.iter().map(|(pid, cmd)| {
                let xassist::AssistCommand::Target(spawn_id) = cmd;
                (*pid, CampAction::Slash(format!("/target spawn:{spawn_id}")))
            }));
        count + say_matches
    }

    /// Returns the PIDs that should receive camp/hunt loop dispatches under the
    /// current routing scope.
    ///
    /// - `AllSession` (or an empty `scope_pids` list) → every registered
    ///   client.
    /// - Narrowed scope → only PIDs that were pre-computed by the TUI app and
    ///   stored in `scope_pids`.
    pub fn pids_in_scope(&self) -> Vec<u32> {
        if self.routing_scope.is_all_session() || self.scope_pids.is_empty() {
            self.client_pids.clone()
        } else {
            self.scope_pids.clone()
        }
    }

    /// Tick the camp loop, including sell cycle, progression checks, and event
    /// production.
    fn tick_camp(&mut self) -> Vec<(u32, CampAction)> {
        let snapshot = self.build_camp_snapshot();

        // --- Task 5: Event production (charm breaks, adds, CC expiry) ---
        self.produce_camp_events(&snapshot);

        // --- Task 2: Vendor sell cycle ---
        let sell_cmds = self.tick_sell_cycle();

        // --- Task 4: Camp progression auto-advance ---
        if self.tick_count.is_multiple_of(PROGRESSION_CHECK_INTERVAL) {
            self.check_camp_progression();
        }

        // Tick the main camp loop
        let mut commands = match self.active_camp.as_mut() {
            Some(camp) => camp.tick(snapshot.as_ref()),
            None => {
                return sell_cmds
                    .into_iter()
                    .map(|(pid, cmd)| (pid, CampAction::Slash(cmd)))
                    .collect();
            }
        };

        // Append sell cycle commands (only during Idle/Medding — the sell cycle
        // itself returns empty when not active)
        if !sell_cmds.is_empty() {
            commands.extend(
                sell_cmds
                    .into_iter()
                    .map(|(pid, cmd)| (pid, CampAction::Slash(cmd))),
            );
        }

        commands
    }

    /// Tick the hunt loop.
    fn tick_hunt(&mut self) -> Vec<(u32, CampAction)> {
        let hunt_snapshot = self.build_hunt_snapshot();
        match self.active_hunt.as_mut() {
            Some(hunt) => {
                let slash_cmds = hunt.tick(hunt_snapshot.as_ref());
                CampAction::from_slash_vec(slash_cmds)
            }
            None => Vec::new(),
        }
    }

    /// Build a `HuntSnapshot` from live game state for the hunt loop.
    fn build_hunt_snapshot(&self) -> Option<HuntSnapshot> {
        let hunt = self.active_hunt.as_ref()?;
        let tank = hunt.members.iter().find(|m| m.role == Role::Tank)?;
        let healer = hunt.members.iter().find(|m| m.role == Role::Healer)?;

        let tank_state = self.game_states.get(&tank.pid)?;
        let healer_state = self.game_states.get(&healer.pid)?;

        let tank_lp = tank_state.local_player.as_ref()?;
        let healer_lp = healer_state.local_player.as_ref()?;

        let member_positions: Vec<(u32, Pos2D)> = hunt
            .members
            .iter()
            .filter_map(|m| {
                self.game_states.get(&m.pid).and_then(|gs| {
                    gs.local_player
                        .as_ref()
                        .map(|lp| (m.pid, Pos2D::new(lp.x, lp.y)))
                })
            })
            .collect();

        let target_is_dead = tank_state
            .target
            .as_ref()
            .is_some_and(|t| t.hp_current <= 0);

        Some(HuntSnapshot {
            tank_pos: Pos2D::new(tank_lp.x, tank_lp.y),
            member_positions,
            target_is_dead,
            tank_hp_pct: tank_lp.hp_pct(),
            healer_mana_pct: healer_lp.mana_pct(),
        })
    }

    /// Tick the sell cycle if active and camp is in Idle or Medding state.
    fn tick_sell_cycle(&mut self) -> Vec<(u32, String)> {
        // Capture both values in a single borrow of active_camp
        let (in_downtime, seller_pid) = match &self.active_camp {
            Some(camp) => {
                let downtime = matches!(camp.state, CampState::Idle | CampState::Medding { .. });
                let pid = camp
                    .members
                    .iter()
                    .find(|m| m.role == Role::Dps)
                    .or(camp.members.first())
                    .map(|m| m.pid);
                (downtime, pid)
            }
            None => return Vec::new(),
        };

        if !in_downtime {
            return Vec::new();
        }

        let Some(sell_cycle) = &mut self.sell_cycle else {
            return Vec::new();
        };

        // Check if we need to start a sell cycle
        if sell_cycle.needs_sell(self.tick_count) {
            if sell_cycle.sell_queue.is_empty() && !self.vendor_inventory.is_empty() {
                sell_cycle.prepare_sell_plan(&self.vendor_inventory, chrono::Utc::now());
            }
            sell_cycle.start_sell(self.tick_count);
        }

        // Only tick if actually selling
        if sell_cycle.state == SellState::Idle {
            return Vec::new();
        }

        match seller_pid {
            Some(pid) => {
                let observation =
                    self.game_states
                        .get(&pid)
                        .map_or_else(VendorObservation::default, |state| VendorObservation {
                            nav_status: Some(state.nav_status.clone()),
                            // Merchant-busy state is not yet surfaced through
                            // `GameState`/shared-memory snapshots.
                            vendor_busy: false,
                        });
                sell_cycle.tick_with_observation(pid, self.tick_count, &observation)
            }
            None => Vec::new(),
        }
    }

    /// Check camp progression and set `suggested_camp` if the group has
    /// outleveled.
    fn check_camp_progression(&mut self) {
        let Some(camp) = &self.active_camp else {
            return;
        };
        let Some(db) = &self.camp_db else {
            return;
        };

        // Calculate average level from game states of camp members
        let mut level_sum = 0.0f32;
        let mut level_count = 0u32;

        for m in &camp.members {
            if let Some(level) = self
                .game_states
                .get(&m.pid)
                .and_then(|gs| gs.local_player.as_ref().map(|lp| f32::from(lp.level)))
            {
                level_sum += level;
                level_count += 1;
            }
        }

        if level_count == 0 {
            return;
        }

        let avg_level = level_sum / level_count as f32;

        match check_progression(&camp.config, avg_level, db) {
            Some(CampProgressionEvent::AdvanceToNext { to_camp, .. })
            | Some(CampProgressionEvent::FallbackToPrev { to_camp, .. }) => {
                if self.suggested_camp.as_deref() != Some(&to_camp) {
                    tracing::info!(
                        avg_level,
                        to_camp = %to_camp,
                        "Camp progression: suggesting move"
                    );
                    self.suggested_camp = Some(to_camp);
                }
            }
            Some(CampProgressionEvent::EndOfChain { .. }) => {
                // No suggestion — end of chain
            }
            None => {
                // In range — clear any stale suggestion
                self.suggested_camp = None;
            }
        }
    }

    /// Produce camp events by comparing current state to previous tick state.
    /// Detects charm breaks, new adds, and expiring CC.
    fn produce_camp_events(&mut self, _snapshot: &Option<CampSnapshot>) {
        let Some(camp) = &mut self.active_camp else {
            return;
        };

        // Only produce events during active combat phases
        if !matches!(
            camp.state,
            CampState::Fighting { .. } | CampState::Pulling { .. }
        ) {
            self.prev_cc_state.clear();
            self.prev_nearby_spawns.clear();
            return;
        }

        // --- Charm break detection ---
        let previous_cc = std::mem::take(&mut self.prev_cc_state);

        for (spawn_id, prev_cc) in &previous_cc {
            if *prev_cc != CcType::Charm {
                continue;
            }

            let still_charmed =
                camp.cc_tracker.targets.iter().any(|t| {
                    t.spawn_id == *spawn_id && matches!(t.cc_applied, Some(CcType::Charm))
                });

            if !still_charmed {
                // Charm was on this mob last tick but isn't now
                camp.push_event(CampEvent::CharmBreak {
                    spawn_id: *spawn_id,
                });
            }
        }

        for target in &camp.cc_tracker.targets {
            if let Some(cc) = target.cc_applied {
                self.prev_cc_state.insert(target.spawn_id, cc);
            }
        }

        // --- Add detection: new NPCs within camp radius ---
        // Use the tank's nearby_spawns as the source
        let tank = camp.members.iter().find(|m| m.role == Role::Tank);
        if let Some(tank) = tank
            && let Some(gs) = self.game_states.get(&tank.pid)
        {
            let previous_nearby = std::mem::take(&mut self.prev_nearby_spawns);

            for spawn in gs.nearby_spawns.iter().filter(|s| s.spawn_type == 1)
            // NPCs only
            {
                if !previous_nearby.contains_key(&spawn.spawn_id) {
                    camp.push_event(CampEvent::AddSpawned {
                        spawn_id: spawn.spawn_id,
                        name: spawn.name.clone(),
                        category: HateTargetCategory::default(),
                    });
                }

                self.prev_nearby_spawns
                    .insert(spawn.spawn_id, spawn.name.clone());
            }
        }

        // --- CC expiry detection ---
        let tick = camp.tick;
        let expiring: Vec<u32> = camp
            .cc_tracker
            .targets
            .iter()
            .filter(|t| {
                t.cc_applied.is_some()
                    && t.cc_expiry_tick > tick
                    && t.cc_expiry_tick.saturating_sub(tick) <= CC_EXPIRY_BUFFER
            })
            .map(|t| t.spawn_id)
            .collect();

        for spawn_id in expiring {
            camp.push_event(CampEvent::CcExpiring { spawn_id });
        }
    }

    /// Start a camp loop with the given config and members.
    pub fn start_camp(&mut self, config: CampConfig, members: Vec<CampMember>) {
        tracing::info!(
            camp = %config.name,
            members = members.len(),
            "Starting camp loop"
        );
        self.active_camp = Some(CampLoop::new(config, members));
    }

    /// Stop the current camp loop.
    pub fn stop_camp(&mut self) {
        if self.active_camp.is_some() {
            tracing::info!("Stopping camp loop");
            self.active_camp = None;
        }
    }

    /// Start a hunt loop with the given config and members.
    pub fn start_hunt(&mut self, config: CampConfig, members: Vec<CampMember>) {
        tracing::info!(
            config = %config.name,
            members = members.len(),
            "Starting hunt loop"
        );
        self.active_hunt = Some(HuntLoop::new(config, members));
        self.operating_mode = OperatingMode::Hunt;
    }

    /// Stop the current hunt loop.
    pub fn stop_hunt(&mut self) {
        if self.active_hunt.is_some() {
            tracing::info!("Stopping hunt loop");
            self.active_hunt = None;
            self.operating_mode = OperatingMode::Camp;
        }
    }

    /// Set the operating mode (Camp or Hunt).
    pub fn set_operating_mode(&mut self, mode: OperatingMode) {
        tracing::info!(?mode, "Switching operating mode");
        self.operating_mode = mode;
    }

    /// Activate a vendor sell cycle with the given config.
    pub fn start_sell_cycle(&mut self, config: VendorConfig) {
        tracing::info!(vendor = %config.vendor_name, "Configuring sell cycle");
        self.sell_cycle = Some(SellCycle::new(config));
    }

    /// Replace the inventory snapshot used for vendor plan generation.
    pub fn set_vendor_inventory(&mut self, inventory: Vec<VendorInventoryItem>) {
        self.vendor_inventory = inventory;
    }

    /// Load the camp progression database from disk.
    pub fn load_camp_database(&mut self) {
        match CampDatabase::load() {
            Ok(db) => {
                tracing::info!(camps = db.len(), "Loaded camp progression database");
                self.camp_db = Some(db);
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load camp database");
            }
        }
    }

    /// Return the current camp/hunt state for display.
    #[must_use]
    pub fn camp_status(&self) -> String {
        match self.operating_mode {
            OperatingMode::Hunt => match &self.active_hunt {
                None => "Hunt mode — no active hunt".into(),
                Some(hunt) => {
                    let state = match &hunt.state {
                        crate::camp::hunt::HuntState::Roaming => "Roaming",
                        crate::camp::hunt::HuntState::Engaging { .. } => "Engaging",
                        crate::camp::hunt::HuntState::Fighting { .. } => "Fighting",
                        crate::camp::hunt::HuntState::Looting { .. } => "Looting",
                    };
                    format!(
                        "Hunt '{}' — {} — tick {} — {} members",
                        hunt.config.name,
                        state,
                        hunt.tick,
                        hunt.members.len()
                    )
                }
            },
            OperatingMode::Camp => match &self.active_camp {
                None => "No active camp".into(),
                Some(camp) => {
                    let state = match &camp.state {
                        CampState::Idle => "Idle",
                        CampState::Pulling { .. } => "Pulling",
                        CampState::Fighting { .. } => "Fighting",
                        CampState::Looting { .. } => "Looting",
                        CampState::Medding { .. } => "Medding",
                        CampState::Buffing { .. } => "Buffing",
                        CampState::Recovery { .. } => "Recovery",
                    };
                    let mut status = format!(
                        "Camp '{}' — {} — tick {} — {} members",
                        camp.config.name,
                        state,
                        camp.tick,
                        camp.members.len()
                    );
                    if let Some(ref suggestion) = self.suggested_camp {
                        status.push_str(&format!(" [suggest: {suggestion}]"));
                    }
                    if let Some(ref sc) = self.sell_cycle
                        && sc.state != SellState::Idle
                    {
                        status.push_str(" [selling]");
                    }
                    status
                }
            },
        }
    }

    /// Get the session token for a client PID (if registered).
    #[must_use]
    pub fn session_tokens_get(&self, pid: u32) -> Option<&SessionToken> {
        self.session_tokens.get(&pid)
    }

    /// Register a client PID and generate a CSPRNG session token for it.
    /// Returns the token so the caller can pass it to the DLL during injection.
    #[allow(dead_code)]
    pub fn register_client(&mut self, pid: u32) -> SessionToken {
        let token = generate_session_token();
        self.session_tokens.insert(pid, token);
        if !self.client_pids.contains(&pid) {
            self.client_pids.push(pid);
        }
        self.session_controls
            .entry(pid)
            .or_insert_with(|| SessionControl::new(pid));
        self.persist_admin_session_inventory();
        token
    }

    /// Bind a stable managed session ID to a live PID for monitoring.
    pub fn bind_monitored_client(&mut self, client_id: ClientId, pid: u32) {
        self.monitoring.register_session(client_id, pid);
    }

    /// Record a memory sample for a managed session.
    pub fn record_memory_sample(&mut self, client_id: ClientId, pid: u32, memory_bytes: u64) {
        self.bind_monitored_client(client_id, pid);
        self.monitoring
            .record_memory_sample(client_id, memory_bytes);
        let character_name = self.metrics_character_name_for_pid(pid);
        self.metrics_collector
            .record_system_sample(&character_name, Some(memory_bytes), None);
    }

    /// Record an operational error against a managed session.
    pub fn record_session_error(&mut self, client_id: ClientId, kind: SessionErrorKind) {
        self.monitoring.record_error(client_id, kind);
    }

    /// Mark a managed session as exited while preserving retained samples.
    pub fn mark_session_exited(&mut self, client_id: ClientId) {
        self.monitoring.mark_session_exited(client_id);
    }

    /// Build a monitoring snapshot for a managed session using the current time.
    #[must_use]
    pub fn monitoring_snapshot(&self, client_id: ClientId) -> Option<SessionMonitoringSnapshot> {
        self.monitoring.snapshot(client_id, Instant::now())
    }

    /// Return the live metrics collector.
    #[must_use]
    pub fn metrics_collector(&self) -> &MetricsCollector {
        &self.metrics_collector
    }

    /// Return the live metrics collector mutably for source-specific hooks.
    pub fn metrics_collector_mut(&mut self) -> &mut MetricsCollector {
        &mut self.metrics_collector
    }

    fn metrics_character_name_for_pid(&self, pid: u32) -> String {
        self.client_names
            .get(&pid)
            .filter(|name| !name.trim().is_empty())
            .cloned()
            .or_else(|| {
                self.game_states.get(&pid).and_then(|state| {
                    state.local_player.as_ref().and_then(|player| {
                        let name = if player.displayed_name.trim().is_empty() {
                            &player.name
                        } else {
                            &player.displayed_name
                        };
                        (!name.trim().is_empty()).then(|| name.clone())
                    })
                })
            })
            .unwrap_or_else(|| format!("pid:{pid}"))
    }

    /// Cache the character name used by admin inventory and cross-group
    /// coordination.
    pub fn set_client_name(&mut self, pid: u32, name: impl Into<String>) {
        self.client_names.insert(pid, name.into());
        self.persist_admin_session_inventory();
    }

    /// Update the admin-facing identifying metadata for a registered client and
    /// persist one consolidated inventory snapshot.
    pub fn update_client_admin_metadata(
        &mut self,
        pid: u32,
        character_name: Option<String>,
        group_id: Option<u8>,
        class_name: Option<String>,
    ) {
        if let Some(name) = character_name {
            self.client_names.insert(pid, name);
        }

        if let Some(group_id) = group_id {
            self.client_groups.insert(pid, group_id);
            self.session_controls
                .entry(pid)
                .or_insert_with(|| SessionControl::new(pid))
                .apply_command(&SessionControlCommand::SetGroup { group_id });
        }

        if let Some(class_name) = class_name {
            self.client_class_names.insert(pid, class_name);
        }

        self.persist_admin_session_inventory();
    }

    /// Set or update the coordination group for a registered client.
    pub fn set_client_group(&mut self, pid: u32, group_id: u8) {
        self.client_groups.insert(pid, group_id);
        let changed = self
            .session_controls
            .entry(pid)
            .or_insert_with(|| SessionControl::new(pid))
            .apply_command(&SessionControlCommand::SetGroup { group_id });
        if changed {
            self.persist_admin_session_inventory();
        }
    }

    /// Return the current coordination group for a registered client.
    #[must_use]
    pub fn client_group(&self, pid: u32) -> Option<u8> {
        self.client_groups.get(&pid).copied().or_else(|| {
            self.session_controls
                .get(&pid)
                .map(|control| control.group_id)
        })
    }

    /// Cache the class name used by cross-group role classification.
    pub fn set_client_class_name(&mut self, pid: u32, class_name: impl Into<String>) {
        self.client_class_names.insert(pid, class_name.into());
        self.persist_admin_session_inventory();
    }

    /// Return the cached class name for a registered client.
    #[must_use]
    pub fn client_class_name(&self, pid: u32) -> Option<&str> {
        self.client_class_names.get(&pid).map(String::as_str)
    }

    /// Set the cross-group assist configuration for a client.
    pub fn set_xassist_config(&mut self, pid: u32, config: XAssistConfig) {
        self.xassist.set_config(pid, config);
    }

    /// Get the cross-group assist configuration for a client.
    #[must_use]
    pub fn get_xassist_config(&self, pid: u32) -> Option<XAssistConfig> {
        self.xassist.get_config(pid).cloned()
    }

    /// Remove client from XAssist tracking.
    pub fn remove_client_from_xassist(&mut self, pid: u32) {
        self.xassist.remove_client(pid);
    }

    /// Dispatch a `CampAction` to the appropriate client via IPC.
    fn dispatch_action(&mut self, pid: u32, action: &CampAction) {
        match action {
            CampAction::Slash(command) => {
                self.send_slash_command(pid, command);
            }
            CampAction::CombatEngage { target_id } => {
                self.send_ipc_command(
                    pid,
                    Command::CombatEngage {
                        target_id: *target_id,
                    },
                );
            }
            CampAction::CombatDisengage => {
                self.send_ipc_command(pid, Command::CombatDisengage);
            }
        }
    }

    /// Create a fresh pipe connection for a single command.
    /// Each call connects, authenticates, and returns an owned pipe that is
    /// dropped after the caller sends one command (matching the DLL's
    /// per-command disconnect model).
    fn get_pipe(&mut self, pid: u32) -> Result<CommandPipe, SessionErrorKind> {
        let name = self
            .client_names
            .get(&pid)
            .map_or("?", std::string::String::as_str);

        let token = if let Some(t) = self.session_tokens.get(&pid) {
            *t
        } else {
            tracing::warn!(pid, name, "No session token for client — skipping");
            self.record_ipc_error_for_pid(pid, SessionErrorKind::MissingSessionToken);
            return Err(SessionErrorKind::MissingSessionToken);
        };

        // Connect fresh for each command. The DLL's pipe server disconnects
        // after each command (connect → token → command → response → disconnect),
        // so persistent connections would fail on the second write.
        let session_id = textquest_common::ipc::session_id_from_token(&token);
        match CommandPipe::connect(pid, session_id) {
            Ok(pipe) => {
                if let Err(e) = pipe.send_raw_token(&token) {
                    tracing::warn!(pid, name, error = %e, "Failed to send token");
                    self.record_ipc_error_for_pid(pid, SessionErrorKind::PipeAuth);
                    return Err(SessionErrorKind::PipeAuth);
                }
                Ok(pipe)
            }
            Err(e) => {
                tracing::warn!(pid, name, error = %e, "Failed to connect pipe");
                self.record_ipc_error_for_pid(pid, SessionErrorKind::PipeConnect);
                Err(SessionErrorKind::PipeConnect)
            }
        }
    }

    fn record_ipc_latency_for_pid(&mut self, pid: u32, started_at: Instant) {
        let Some(client_id) = self.monitoring.client_id_for_pid(pid) else {
            return;
        };
        let finished_at = Instant::now();
        let latency_ms = finished_at
            .checked_duration_since(started_at)
            .unwrap_or_default()
            .as_millis()
            .max(1) as u64;
        self.monitoring
            .record_ipc_latency_at(client_id, latency_ms, finished_at);
        let character_name = self.metrics_character_name_for_pid(pid);
        self.metrics_collector
            .record_system_sample(&character_name, None, Some(latency_ms));
    }

    fn record_ipc_error_for_pid(&mut self, pid: u32, kind: SessionErrorKind) {
        let Some(client_id) = self.monitoring.client_id_for_pid(pid) else {
            return;
        };
        self.monitoring.record_error(client_id, kind);
    }

    /// Send a structured IPC command to a client via named pipe.
    /// Creates a fresh connection per command (connect → token → command →
    /// drop).
    ///
    /// Returns `true` on successful dispatch, `false` on any failure
    /// (pipe not ready, serialization error, remote-error response). Callers
    /// that cache "last sent" state must check this value — see
    /// `sync_reward_automation_configs` for the DLL-side reward-config sync
    /// that previously cached "sent" unconditionally and could starve reward
    /// automation when the pipe was briefly unavailable during client
    /// startup.
    pub(crate) fn send_ipc_command(&mut self, pid: u32, cmd: Command) -> bool {
        let name = self
            .client_names
            .get(&pid)
            .map_or("?", std::string::String::as_str)
            .to_string();
        let started_at = Instant::now();

        let Ok(pipe) = self.get_pipe(pid) else {
            return false;
        };
        match pipe.send(&cmd) {
            Ok(Response::CommandResult {
                success: false,
                message,
            }) => {
                self.record_ipc_latency_for_pid(pid, started_at);
                self.record_ipc_error_for_pid(pid, SessionErrorKind::IpcDispatch);
                tracing::warn!(
                    pid,
                    name = %name,
                    ?cmd,
                    %message,
                    "DLL rejected IPC command"
                );
                false
            }
            Ok(_response) => {
                self.record_ipc_latency_for_pid(pid, started_at);
                tracing::debug!(pid, name = %name, ?cmd, "Dispatched IPC command");
                true
            }
            Err(e) => {
                self.record_ipc_error_for_pid(pid, SessionErrorKind::IpcDispatch);
                tracing::warn!(pid, name = %name, ?cmd, error = %e, "Failed to send command");
                false
            }
        }
        // pipe is dropped here — DLL will disconnect its end too
    }

    /// Poll a client for accumulated packet events.
    /// Sends `PollPackets` and returns any `PacketEventInfo` entries.
    pub fn poll_packets(&mut self, pid: u32) -> Vec<textquest_common::ipc::PacketEventInfo> {
        let started_at = Instant::now();
        let Ok(pipe) = self.get_pipe(pid) else {
            return Vec::new();
        };
        match pipe.send(&Command::PollPackets) {
            Ok(Response::PacketBatch { events }) => {
                self.record_ipc_latency_for_pid(pid, started_at);
                events
            }
            Ok(_) => {
                self.record_ipc_latency_for_pid(pid, started_at);
                Vec::new()
            }
            Err(e) => {
                self.record_ipc_error_for_pid(pid, SessionErrorKind::IpcDispatch);
                tracing::debug!(pid, error = %e, "Failed to poll packets");
                Vec::new()
            }
        }
    }

    /// Poll a client for accumulated spawn list delta events.
    /// Sends `PollSpawnEvents` and returns any `SpawnEvent` delta entries.
    pub fn poll_spawn_events(&mut self, pid: u32) -> Vec<textquest_common::ipc::SpawnEvent> {
        let started_at = Instant::now();
        let Ok(pipe) = self.get_pipe(pid) else {
            return Vec::new();
        };
        match pipe.send(&Command::PollSpawnEvents) {
            Ok(Response::SpawnEventBatch { events }) => {
                self.record_ipc_latency_for_pid(pid, started_at);
                events
            }
            Ok(_) => {
                self.record_ipc_latency_for_pid(pid, started_at);
                Vec::new()
            }
            Err(e) => {
                self.record_ipc_error_for_pid(pid, SessionErrorKind::IpcDispatch);
                tracing::debug!(pid, error = %e, "Failed to poll spawn events");
                Vec::new()
            }
        }
    }

    /// Poll a client for accumulated chat messages captured by the DLL.
    pub fn poll_chat(&mut self, pid: u32) -> Vec<ChatMessageInfo> {
        let started_at = Instant::now();
        let Ok(pipe) = self.get_pipe(pid) else {
            return Vec::new();
        };
        match pipe.send(&Command::PollChat) {
            Ok(Response::ChatBatch { messages }) => {
                self.record_ipc_latency_for_pid(pid, started_at);
                messages
            }
            Ok(_) => {
                self.record_ipc_latency_for_pid(pid, started_at);
                Vec::new()
            }
            Err(e) => {
                self.record_ipc_error_for_pid(pid, SessionErrorKind::IpcDispatch);
                tracing::debug!(pid, error = %e, "Failed to poll chat");
                Vec::new()
            }
        }
    }

    /// Read raw bytes from the EQ process address space via IPC.
    /// Sends `MemoryRead` and returns `(address, bytes)` on success, or `None`
    /// if the pipe is unavailable or the DLL returns an unexpected response.
    pub fn read_memory(
        &mut self,
        pid: u32,
        address: usize,
        size: usize,
    ) -> Option<(usize, Vec<u8>)> {
        let started_at = Instant::now();
        let pipe = self.get_pipe(pid).ok()?;
        match pipe.send(&Command::MemoryRead {
            address,
            length: size,
        }) {
            Ok(Response::MemoryData { address, bytes }) => {
                self.record_ipc_latency_for_pid(pid, started_at);
                Some((address, bytes))
            }
            Ok(_) => {
                self.record_ipc_latency_for_pid(pid, started_at);
                None
            }
            Err(e) => {
                self.record_ipc_error_for_pid(pid, SessionErrorKind::IpcDispatch);
                tracing::debug!(pid, address, error = %e, "Failed to read memory");
                None
            }
        }
    }

    /// Read raw bytes relative to a known EQ global pointer via IPC.
    pub fn read_memory_relative(
        &mut self,
        pid: u32,
        global_name: impl Into<String>,
        offset: usize,
        size: usize,
    ) -> Option<(usize, Vec<u8>)> {
        let started_at = Instant::now();
        let global_name = global_name.into();
        let pipe = self.get_pipe(pid).ok()?;
        match pipe.send(&Command::MemoryReadRelative {
            global_name: global_name.clone(),
            offset,
            length: size,
        }) {
            Ok(Response::MemoryData { address, bytes }) => {
                self.record_ipc_latency_for_pid(pid, started_at);
                Some((address, bytes))
            }
            Ok(_) => {
                self.record_ipc_latency_for_pid(pid, started_at);
                None
            }
            Err(e) => {
                self.record_ipc_error_for_pid(pid, SessionErrorKind::IpcDispatch);
                tracing::debug!(
                    pid,
                    global = %global_name,
                    offset,
                    error = %e,
                    "Failed to read relative memory"
                );
                None
            }
        }
    }

    /// Eject the DLL from a client and clean up its tracked state.
    /// Sends an Eject IPC command, then removes the client from all maps.
    pub fn eject_client(&mut self, pid: u32) {
        let name = self
            .client_names
            .get(&pid)
            .map_or("?", std::string::String::as_str)
            .to_string();
        tracing::info!(pid, name = %name, "Ejecting client");

        // Best-effort eject command — pipe may already be dead
        self.send_ipc_command(pid, Command::Eject);
        self.remove_client(pid);
    }

    /// Remove a client from all tracked state (does NOT send any IPC).
    pub fn remove_client(&mut self, pid: u32) {
        self.client_pids.retain(|&p| p != pid);
        self.client_names.remove(&pid);
        self.client_groups.remove(&pid);
        self.client_class_names.remove(&pid);
        self.game_states.remove(&pid);
        self.state_readers.remove(&pid);
        self.session_tokens.remove(&pid);
        self.session_controls.remove(&pid);
        self.state_timestamps.remove(&pid);
        self.last_sent_reward_configs.remove(&pid);
        self.monitoring.detach_process(pid);
        self.xassist.remove_client(pid);
        self.persist_admin_session_inventory();
        tracing::info!(pid, "Client removed from orchestrator");
    }

    fn poll_trade_chat_if_due(&mut self) {
        if self
            .last_trade_chat_poll
            .is_some_and(|last| last.elapsed() < TRADE_CHAT_POLL_INTERVAL)
        {
            return;
        }

        let pids = self.client_pids.clone();
        if pids.is_empty() {
            return;
        }
        self.last_trade_chat_poll = Some(Instant::now());

        let mut recorded = 0usize;
        let mut monitor_available = self.trade_price_monitor.is_some();
        for pid in pids {
            let Some((zone, local_character_name)) = self.game_states.get(&pid).map(|state| {
                let zone = if !state.zone_short_name.is_empty() {
                    state.zone_short_name.clone()
                } else {
                    state.zone_long_name.clone()
                };
                let local_character_name = state
                    .local_player
                    .as_ref()
                    .map(|player| player.displayed_name.clone())
                    .or_else(|| self.client_names.get(&pid).cloned());
                (zone, local_character_name)
            }) else {
                continue;
            };

            let messages = self.poll_chat(pid);
            if messages.is_empty() {
                continue;
            }

            if !monitor_available {
                monitor_available = self.ensure_trade_price_monitor().is_some();
            }
            let Some(monitor) = self.trade_price_monitor.as_mut() else {
                continue;
            };

            for message in messages {
                let Some(chat) = textquest_common::chat::parse_chat_text(&message.text) else {
                    continue;
                };

                match monitor.record_chat(
                    pid,
                    &zone,
                    local_character_name.as_deref(),
                    &chat,
                    message.timestamp_ms as i64,
                ) {
                    Ok(true) => recorded += 1,
                    Ok(false) => {}
                    Err(error) => tracing::warn!(
                        pid,
                        zone = zone.as_str(),
                        error = %error,
                        "Failed to record passive trade chat"
                    ),
                }
            }
        }

        if recorded > 0 {
            tracing::info!(recorded, "Recorded passive trade-price observations");
        }
    }

    fn ensure_trade_price_monitor(&mut self) -> Option<&mut TradePriceMonitor> {
        if self.trade_price_monitor.is_none() {
            match TradePriceMonitor::open(Path::new(crate::TRADE_PRICE_DB_PATH)) {
                Ok(monitor) => {
                    self.trade_price_monitor = Some(monitor);
                    self.trade_price_monitor_open_failed = false;
                }
                Err(error) => {
                    if !self.trade_price_monitor_open_failed {
                        tracing::warn!(
                            error = %error,
                            path = crate::TRADE_PRICE_DB_PATH,
                            "Passive trade-price monitor disabled"
                        );
                    }
                    self.trade_price_monitor_open_failed = true;
                    return None;
                }
            }
        }

        self.trade_price_monitor.as_mut()
    }

    fn poll_chat_log_if_due(&mut self) {
        if self
            .last_chat_log_poll
            .is_some_and(|last| last.elapsed() < self.chat_log_poll_interval)
        {
            return;
        }

        let pids = self.client_pids.clone();
        if pids.is_empty() {
            return;
        }
        self.last_chat_log_poll = Some(Instant::now());

        for pid in pids {
            let Some((server, character_name)) = self.game_states.get(&pid).map(|state| {
                let server = if !state.zone_short_name.is_empty() {
                    state.zone_short_name.clone()
                } else {
                    state.zone_long_name.clone()
                };
                let character_name = state
                    .local_player
                    .as_ref()
                    .map(|player| player.displayed_name.clone())
                    .or_else(|| self.client_names.get(&pid).cloned());
                (server, character_name)
            }) else {
                continue;
            };

            let Some(character) = character_name else {
                continue;
            };

            let messages = self.poll_chat(pid);
            if messages.is_empty() {
                continue;
            }

            let Some(manager) = self.chat_log_manager.as_mut() else {
                continue;
            };

            let mut gm_tells: Vec<(String, String)> = Vec::new();

            for message in messages {
                let Some(chat) = textquest_common::chat::parse_chat_text(&message.text) else {
                    if manager.get_config().log_eq_chat {
                        let _ = manager.log_message(&server, &character, &message, None);
                    }
                    continue;
                };

                let channel: ChatChannel = chat.channel.into();

                if matches!(channel, ChatChannel::Tell)
                    && textquest_common::gm_detection::detect_gm_tell(&chat.sender, &chat.message)
                {
                    gm_tells.push((chat.sender.clone(), chat.message.clone()));
                }

                if let Err(error) =
                    manager.log_message(&server, &character, &message, Some(channel))
                {
                    tracing::warn!(
                        pid,
                        server = %server,
                        character = %character,
                        error = %error,
                        "Failed to log chat message"
                    );
                }
            }

            for (sender, msg_text) in gm_tells {
                self.emit_gm_alert(&character, &sender, &msg_text);
            }
        }
    }

    /// Initialize the chat log manager from config.
    pub fn init_chat_log_manager(&mut self, config: ChatLogConfig, log_dir: PathBuf) {
        match ChatLogManager::new(config, log_dir) {
            Ok(manager) => {
                tracing::info!("Chat log manager initialized");
                self.chat_log_manager = Some(manager);
            }
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "Failed to initialize chat log manager"
                );
            }
        }
    }

    /// Update chat log config at runtime.
    pub fn update_chat_log_config(&mut self, config: ChatLogConfig) {
        if let Some(manager) = self.chat_log_manager.as_mut() {
            manager.update_config(config);
        }
    }

    /// Configure say channel detection and alerting from application config.
    ///
    /// When `config.enabled` is `false`, the detector is cleared. When `enabled`
    /// is `true`, rules are loaded from the config.
    pub fn configure_say_detection(&mut self, config: &crate::config::SayDetectionConfig) {
        self.say_detection_config = config.clone();
        self.say_detector.clear_rules();
        self.say_detection_webhook = config
            .discord_webhook_url
            .clone()
            .filter(|url| !url.trim().is_empty())
            .map(crate::discord::webhook::WebhookSender::new);

        if !config.enabled {
            tracing::info!("Say detection disabled");
            return;
        }

        for rule_config in &config.rules {
            let pattern_type = match rule_config.pattern_type {
                crate::config::SayPatternType::Substring => SayPattern::Substring,
                crate::config::SayPatternType::Exact => SayPattern::Exact,
                crate::config::SayPatternType::Regex => SayPattern::Regex,
            };

            let action = match rule_config.action_type {
                crate::config::SayRuleAction::Alert => SayAction::Alert,
                crate::config::SayRuleAction::Broadcast => {
                    let Some(command) = rule_config
                        .action_value
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    else {
                        tracing::warn!(
                            rule = %rule_config.name,
                            "Skipping say-detection rule with empty broadcast command"
                        );
                        continue;
                    };
                    SayAction::Broadcast(command.to_string())
                }
                crate::config::SayRuleAction::Command => {
                    let Some(command) = rule_config
                        .action_value
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    else {
                        tracing::warn!(
                            rule = %rule_config.name,
                            "Skipping say-detection rule with empty local command"
                        );
                        continue;
                    };
                    SayAction::Command(command.to_string())
                }
            };

            let mut rule = SayRule::new(
                &rule_config.name,
                &rule_config.pattern,
                pattern_type,
                action,
            );
            rule.enabled = rule_config.enabled;
            self.say_detector.add_rule(rule);
        }

        tracing::info!(
            rules = self.say_detector.enabled_count(),
            "Say detection configured"
        );
    }

    /// Poll say-channel chat events and evaluate detection rules.
    ///
    /// Returns the number of matched rules (for telemetry).
    fn poll_and_evaluate_say_detection(&mut self) -> usize {
        if self.say_detector.enabled_count() == 0 {
            return 0;
        }

        let mut match_count = 0;
        let pids = self.client_pids.clone();

        for pid in pids {
            let messages = self.poll_chat(pid);
            for msg in messages {
                if let Some(event) = textquest_common::chat::parse_chat_text(&msg.text) {
                    if event.channel != textquest_common::chat::ChatChannel::Say {
                        continue;
                    }

                    let matches = self.say_detector.evaluate(&event);
                    if matches.is_empty() {
                        continue;
                    }

                    match_count += matches.len();
                    for matched in matches {
                        match matched.action {
                            SayAction::Alert => {
                                self.emit_say_alert(&matched.rule_name, &event);
                            }
                            SayAction::Broadcast(cmd) => {
                                self.broadcast_command(&cmd);
                                tracing::info!(
                                    rule = %matched.rule_name,
                                    cmd = %cmd,
                                    sender = %event.sender,
                                    "Say detection broadcast executed"
                                );
                            }
                            SayAction::Command(cmd) => {
                                self.send_slash_command(pid, &cmd);
                                tracing::info!(
                                    rule = %matched.rule_name,
                                    cmd = %cmd,
                                    sender = %event.sender,
                                    "Say detection command executed"
                                );
                            }
                        }
                    }
                }
            }
        }

        match_count
    }

    fn emit_say_alert(&mut self, rule_name: &str, event: &textquest_common::chat::ChatEvent) {
        tracing::warn!(
            rule = rule_name,
            sender = %event.sender,
            message = %event.message,
            sound_enabled = self.say_detection_config.sound_enabled,
            toast_enabled = self.say_detection_config.toast_enabled,
            "Say detection alert triggered"
        );

        if self.say_detection_config.broadcast_all_clients {
            self.broadcast_command(&format!(
                "/echo [Say Alert] {} :: {}",
                event.sender, event.message
            ));
        }

        if let Some(webhook) = &self.say_detection_webhook {
            webhook.send(
                crate::discord::webhook::DiscordAlert::simple(
                    format!("Say Alert: {rule_name}"),
                    format!("{} says: {}", event.sender, event.message),
                    crate::discord::webhook::AlertLevel::Warning,
                    crate::discord::webhook::EventCategory::Status,
                )
                .with_field("Rule", rule_name, true)
                .with_field("Speaker", &event.sender, true),
            );
        }
    }

    /// Emit a GM interaction alert and log the event.
    fn emit_gm_alert(&self, actor: &str, sender: &str, message: &str) {
        tracing::warn!(
            actor = %actor,
            sender = %sender,
            message = %message,
            "**SECURITY ALERT** GM/CSR interaction detected — operator review required"
        );
    }

    /// Send a slash command to all registered clients.
    fn broadcast_command(&mut self, command: &str) {
        let pids = self.client_pids.clone();
        for pid in pids {
            self.send_slash_command(pid, command);
        }
    }

    /// Send a single slash command to a client via named pipe.
    fn send_slash_command(&mut self, pid: u32, command: &str) {
        let name = self
            .client_names
            .get(&pid)
            .map_or("?", std::string::String::as_str)
            .to_string();

        match crate::nav::try_handle_local_slash_command(pid, command) {
            Ok(Some(message)) => {
                tracing::info!(pid, name = %name, %command, %message, "Handled local slash command");
                return;
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(pid, name = %name, %command, %error, "Failed to handle local slash command; falling back to IPC");
            }
        }

        let started_at = Instant::now();
        let Ok(pipe) = self.get_pipe(pid) else {
            return;
        };
        let cmd = Command::SlashCommand {
            command: command.to_string(),
        };
        match pipe.send(&cmd) {
            Ok(_response) => {
                self.record_ipc_latency_for_pid(pid, started_at);
                tracing::debug!(pid, name = %name, %command, "Dispatched command");
            }
            Err(e) => {
                self.record_ipc_error_for_pid(pid, SessionErrorKind::IpcDispatch);
                tracing::warn!(pid, name = %name, %command, error = %e, "Failed to send command");
            }
        }
        // pipe is dropped here
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camp::state::Role;
    #[cfg(not(windows))]
    use crate::ipc::pipe::{clear_test_ipc_responses, queue_test_ipc_response};
    use std::sync::{Mutex, OnceLock};
    use textquest_common::{
        combat::CombatStatus,
        nav::NavStatus,
        types::{GameState, SpawnData},
    };

    fn runtime_snapshot_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn test_config() -> CampConfig {
        CampConfig {
            name: "test".into(),
            zone: "crushbone".into(),
            camp_center: [0.0, 0.0, 0.0],
            pull_point: [10.0, 10.0, 0.0],
            pull_radius: 100.0,
            camp_radius: 20.0,
            leash_radius: 80.0,
            rest_mana_pct: 50,
            pull_mana_pct: 20,
            level_range: [1, 10],
            pull_mob_names: vec!["a_mob".into()],
            ignore_mob_names: Vec::new(),
            burn_mob_names: Vec::new(),
            return_no_aggro: false,
            next_camp: None,
            prev_camp: None,
        }
    }

    fn test_members() -> Vec<CampMember> {
        vec![
            CampMember::new(100, "Tank".into(), Role::Tank),
            CampMember::new(101, "Healer".into(), Role::Healer),
            CampMember::new(102, "DPS".into(), Role::Dps),
        ]
    }

    fn make_spawn_named(name: &str, class_id: u8, hp_current: i64, hp_max: i64) -> SpawnData {
        SpawnData {
            spawn_id: 1,
            name: name.into(),
            displayed_name: name.into(),
            spawn_type: 0,
            level: 60,
            class_id,
            race_id: 1,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current,
            hp_max,
            mana_current: 1000,
            mana_max: 1000,
            endurance_current: 100,
            endurance_max: 100,
            speed_run: 0.0,
            stand_state: if hp_current <= 0 { 111 } else { 0 },
            is_gm: false,
        }
    }

    fn make_game_state(
        client_id: u32,
        zone: &str,
        local_player: SpawnData,
        target: Option<SpawnData>,
    ) -> GameState {
        GameState {
            client_id,
            local_player: Some(local_player),
            target,
            nearby_spawns: vec![],
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: if client_id.is_multiple_of(10) {
                CombatStatus::Idle
            } else {
                CombatStatus::Engaging { target_id: 999 }
            },
            zone_short_name: zone.to_string(),
            zone_long_name: zone.to_string(),
            active_buffs: vec![],
            pet: None,
            actual_version: None,
        }
    }

    #[test]
    fn build_live_spawn_snapshot_exposes_observers_and_nearby_spawns() {
        let mut orch = Orchestrator::new();
        orch.client_pids = vec![100];
        orch.client_names.insert(100, "Tank".into());
        let mut state = make_game_state(100, "kael", make_spawn_named("Tank", 1, 900, 1000), None);
        state.target = Some(SpawnData {
            spawn_id: 999,
            name: "a frost giant".into(),
            displayed_name: "a frost giant".into(),
            spawn_type: 1,
            level: 61,
            class_id: 1,
            race_id: 10,
            x: 50.0,
            y: 75.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 500,
            hp_max: 1000,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 0,
            endurance_max: 0,
            speed_run: 0.0,
            stand_state: 0,
            is_gm: false,
        });
        state.zone_long_name = "Kael Drakkel".into();
        state.nearby_spawns = vec![SpawnData {
            spawn_id: 1234,
            name: "a fire giant".into(),
            displayed_name: "a fire giant".into(),
            spawn_type: 1,
            level: 61,
            class_id: 1,
            race_id: 10,
            x: 20.0,
            y: 40.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 800,
            hp_max: 1000,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 0,
            endurance_max: 0,
            speed_run: 0.0,
            stand_state: 0,
            is_gm: false,
        }];
        orch.game_states.insert(100, state);

        let snapshot = orch.build_live_spawn_snapshot();

        assert_eq!(snapshot.observers.len(), 1);
        assert_eq!(snapshot.observers[0].character_name, "Tank");
        assert_eq!(snapshot.observers[0].zone_short_name, "kael");
        assert_eq!(snapshot.observers[0].zone_long_name, "Kael Drakkel");
        assert_eq!(snapshot.observers[0].target_spawn_id, Some(999));
        assert_eq!(snapshot.observers[0].nearby_spawns[0].spawn_id, 1234);
    }

    #[test]
    fn persist_live_spawn_snapshot_to_path_writes_json_snapshot() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("runtime/live_spawns.json");
        let snapshot = LiveSpawnSnapshot {
            observers: vec![LiveSpawnObserver {
                client_id: 100,
                character_name: "Frostreaver".into(),
                zone_short_name: "kael".into(),
                zone_long_name: "Kael Drakkel".into(),
                local_player: make_spawn_named("Frostreaver", 2, 900, 1000),
                target_spawn_id: Some(999),
                nearby_spawns: vec![SpawnData {
                    spawn_id: 999,
                    name: "a frost giant".into(),
                    displayed_name: "a frost giant".into(),
                    spawn_type: 1,
                    level: 60,
                    class_id: 1,
                    race_id: 9,
                    x: 50.0,
                    y: 75.0,
                    z: 0.0,
                    heading: 0.0,
                    hp_current: 500,
                    hp_max: 1000,
                    mana_current: 0,
                    mana_max: 0,
                    endurance_current: 0,
                    endurance_max: 0,
                    speed_run: 0.0,
                    stand_state: 0,
                    is_gm: false,
                }],
            }],
        };

        persist_live_spawn_snapshot_to_path(&path, &snapshot).expect("snapshot write");

        let payload = std::fs::read_to_string(path).expect("snapshot exists");
        assert!(payload.contains("Frostreaver"));
        assert!(payload.contains("\"target_spawn_id\":999"));
        assert!(payload.contains("\"observers\""));
    }

    #[test]
    fn persist_shared_client_states_to_path_replaces_existing_file() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("runtime/live_sessions.json");

        let initial = vec![
            SharedClientState::from_game_state(
                Some("Tank"),
                &make_game_state(100, "kael", make_spawn_named("Tank", 1, 900, 1000), None),
                false,
            )
            .expect("initial shared state"),
        ];
        persist_shared_client_states_to_path(&path, &initial).expect("initial snapshot write");

        let replacement = vec![
            SharedClientState::from_game_state(
                Some("Healer"),
                &make_game_state(
                    101,
                    "soldungc",
                    make_spawn_named("Healer", 2, 850, 1000),
                    None,
                ),
                false,
            )
            .expect("replacement shared state"),
        ];
        persist_shared_client_states_to_path(&path, &replacement)
            .expect("replacement snapshot write");

        let payload = std::fs::read_to_string(path).expect("snapshot exists");
        assert!(payload.contains("Healer"));
        assert!(!payload.contains("\"Tank\""));
    }

    #[test]
    fn persist_live_spawn_snapshot_to_path_replaces_existing_file() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("runtime/live_spawns.json");

        let initial = LiveSpawnSnapshot {
            observers: vec![LiveSpawnObserver {
                client_id: 100,
                character_name: "Frostreaver".into(),
                zone_short_name: "kael".into(),
                zone_long_name: "Kael Drakkel".into(),
                local_player: make_spawn_named("Frostreaver", 2, 900, 1000),
                target_spawn_id: None,
                nearby_spawns: vec![make_spawn_named("a frost giant", 1, 900, 1000)],
            }],
        };
        persist_live_spawn_snapshot_to_path(&path, &initial).expect("initial snapshot write");

        let replacement = LiveSpawnSnapshot {
            observers: vec![LiveSpawnObserver {
                client_id: 101,
                character_name: "Leafbinder".into(),
                zone_short_name: "soldungc".into(),
                zone_long_name: "Solusek's Lair".into(),
                local_player: make_spawn_named("Leafbinder", 2, 700, 900),
                target_spawn_id: None,
                nearby_spawns: vec![make_spawn_named("a lava walker", 1, 600, 1000)],
            }],
        };
        persist_live_spawn_snapshot_to_path(&path, &replacement)
            .expect("replacement snapshot write");

        let payload = std::fs::read_to_string(path).expect("snapshot exists");
        assert!(payload.contains("Leafbinder"));
        assert!(!payload.contains("Frostreaver"));
    }

    #[test]
    fn sync_runtime_snapshots_overwrites_stale_live_spawn_snapshot_with_empty_state() {
        let _guard = runtime_snapshot_test_lock()
            .lock()
            .expect("live spawn snapshot test lock");
        let path = live_spawn_snapshot_path();
        let previous = std::fs::read(&path).ok();

        let stale_snapshot = LiveSpawnSnapshot {
            observers: vec![LiveSpawnObserver {
                client_id: 100,
                character_name: "Frostreaver".into(),
                zone_short_name: "kael".into(),
                zone_long_name: "Kael Drakkel".into(),
                local_player: make_spawn_named("Frostreaver", 2, 900, 1000),
                target_spawn_id: Some(999),
                nearby_spawns: vec![SpawnData {
                    spawn_id: 999,
                    name: "a frost giant".into(),
                    displayed_name: "a frost giant".into(),
                    spawn_type: 1,
                    level: 60,
                    class_id: 1,
                    race_id: 9,
                    x: 50.0,
                    y: 75.0,
                    z: 0.0,
                    heading: 0.0,
                    hp_current: 500,
                    hp_max: 1000,
                    mana_current: 0,
                    mana_max: 0,
                    endurance_current: 0,
                    endurance_max: 0,
                    speed_run: 0.0,
                    stand_state: 0,
                    is_gm: false,
                }],
            }],
        };
        persist_live_spawn_snapshot_to_path(&path, &stale_snapshot).expect("seed stale snapshot");

        let mut orch = Orchestrator::new();
        orch.sync_runtime_snapshots();

        let payload = std::fs::read(&path).expect("live spawn snapshot exists");
        let snapshot =
            serde_json::from_slice::<LiveSpawnSnapshot>(&payload).expect("parse live snapshot");

        if let Some(contents) = previous {
            std::fs::write(&path, contents).expect("restore prior snapshot");
        } else {
            let _ = std::fs::remove_file(&path);
        }

        assert!(snapshot.observers.is_empty());
    }

    #[test]
    fn sync_runtime_snapshots_retries_after_persist_failure() {
        let _guard = runtime_snapshot_test_lock()
            .lock()
            .expect("runtime snapshot test lock");
        let session_path = live_session_snapshot_path();
        let spawn_path = live_spawn_snapshot_path();
        let runtime_dir = session_path.parent().expect("runtime dir").to_path_buf();
        let data_dir = runtime_dir.parent().expect("data dir").to_path_buf();
        let runtime_dir_existed = runtime_dir.exists();
        let data_dir_existed = data_dir.exists();
        let backup_dir = runtime_dir.with_extension(format!(
            "bak-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));

        if runtime_dir_existed {
            std::fs::rename(&runtime_dir, &backup_dir).expect("backup runtime dir");
        } else if !data_dir_existed {
            std::fs::create_dir_all(&data_dir).expect("create data dir");
        }
        std::fs::write(&runtime_dir, b"blocked").expect("block runtime dir");

        let mut orch = Orchestrator::new();
        orch.sync_runtime_snapshots();

        assert!(!orch.persisted_shared_client_states);
        assert!(!orch.persisted_live_spawn_snapshot);

        std::fs::remove_file(&runtime_dir).expect("remove runtime dir blocker");
        orch.sync_runtime_snapshots();

        assert!(orch.persisted_shared_client_states);
        assert!(orch.persisted_live_spawn_snapshot);
        assert!(session_path.exists());
        assert!(spawn_path.exists());

        let _ = std::fs::remove_file(&session_path);
        let _ = std::fs::remove_file(&spawn_path);
        let _ = std::fs::remove_dir(&runtime_dir);
        if runtime_dir_existed {
            std::fs::rename(&backup_dir, &runtime_dir).expect("restore runtime dir");
        } else if !data_dir_existed {
            let _ = std::fs::remove_dir(&data_dir);
        }
    }

    fn mark_states_fresh(orch: &mut Orchestrator, pids: &[u32]) {
        for &pid in pids {
            orch.state_timestamps.insert(pid, orch.tick_count);
        }
    }

    #[test]
    fn test_new_orchestrator() {
        let orch = Orchestrator::new();
        assert!(orch.active_camp.is_none());
        assert_eq!(orch.tick_count, 0);
    }

    #[test]
    fn test_tick_no_camp() {
        let mut orch = Orchestrator::new();
        assert_eq!(orch.tick(), 0);
        assert_eq!(orch.tick_count, 1);
    }

    #[test]
    fn test_start_and_stop_camp() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        assert!(orch.active_camp.is_some());
        assert!(orch.camp_status().contains("test"));

        orch.stop_camp();
        assert!(orch.active_camp.is_none());
        assert_eq!(orch.camp_status(), "No active camp");
    }

    #[test]
    fn test_tick_with_camp_generates_commands() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());

        // First tick transitions Idle -> Pulling, which generates commands
        let count = orch.tick();
        assert!(count > 0);
        assert!(!orch.last_dispatched.is_empty());
    }

    #[test]
    fn test_camp_status_shows_state() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        orch.tick(); // Idle -> Pulling
        let status = orch.camp_status();
        assert!(status.contains("Pulling"));
        assert!(status.contains("3 members"));
    }

    #[test]
    fn test_get_client_state_none_without_data() {
        let orch = Orchestrator::new();
        assert!(orch.get_client_state(100).is_none());
    }

    #[test]
    fn test_game_states_initialized_empty() {
        let orch = Orchestrator::new();
        assert!(orch.game_states.is_empty());
        assert!(orch.state_readers.is_empty());
    }

    #[test]
    fn sync_shared_client_states_retries_after_send_failure() {
        let mut orch = Orchestrator::new();
        let player = make_spawn_named("Tank", 1, 100, 100);
        orch.client_pids.push(100);
        orch.client_names.insert(100, "Tank".into());
        orch.game_states
            .insert(100, make_game_state(100, "crushbone", player, None));

        orch.sync_shared_client_states();

        assert!(
            orch.last_shared_client_states.is_empty(),
            "failed IPC sends should not advance the shared-state cache"
        );
    }

    #[test]
    fn test_build_camp_snapshot_none_without_camp() {
        let orch = Orchestrator::new();
        assert!(orch.build_camp_snapshot().is_none());
    }

    #[test]
    fn test_build_camp_snapshot_none_without_game_state() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        // No game state inserted — snapshot should be None
        assert!(orch.build_camp_snapshot().is_none());
    }

    #[test]
    fn test_build_camp_snapshot_with_game_state() {
        use textquest_common::{
            combat::CombatStatus,
            nav::NavStatus,
            types::{GameState, SpawnData},
        };

        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());

        fn make_spawn(hp: i64, hp_max: i64, mana: i32, mana_max: i32) -> SpawnData {
            SpawnData {
                spawn_id: 1,
                name: "Test".into(),
                displayed_name: "Test".into(),
                spawn_type: 0,
                level: 60,
                class_id: 1,
                race_id: 1,
                x: 0.0,
                y: 0.0,
                z: 0.0,
                heading: 0.0,
                hp_current: hp,
                hp_max,
                mana_current: mana,
                mana_max,
                endurance_current: 100,
                endurance_max: 100,
                speed_run: 0.0,
                stand_state: 0,
                is_gm: false,
            }
        }

        // Set tick_count and timestamps so staleness check passes
        orch.tick_count = 1;
        orch.state_timestamps.insert(100, 1);
        orch.state_timestamps.insert(101, 1);

        // Tank at 80% HP
        orch.game_states.insert(
            100,
            GameState {
                client_id: 100,
                local_player: Some(make_spawn(800, 1000, 0, 0)),
                target: Some(make_spawn(500, 1000, 0, 0)),
                nearby_spawns: vec![],
                timestamp_ms: 0,
                nav_status: NavStatus::Idle,
                combat_status: CombatStatus::Idle,
                zone_short_name: String::new(),
                zone_long_name: String::new(),
                active_buffs: vec![],
                pet: None,
                actual_version: None,
            },
        );

        // Healer at 60% mana
        orch.game_states.insert(
            101,
            GameState {
                client_id: 101,
                local_player: Some(make_spawn(1000, 1000, 600, 1000)),
                target: None,
                nearby_spawns: vec![],
                timestamp_ms: 0,
                nav_status: NavStatus::Idle,
                combat_status: CombatStatus::Idle,
                zone_short_name: String::new(),
                zone_long_name: String::new(),
                active_buffs: vec![],
                pet: None,
                actual_version: None,
            },
        );

        let snap = orch.build_camp_snapshot().expect("should build snapshot");
        assert!((snap.tank_hp_pct - 80.0).abs() < 0.1);
        assert!((snap.healer_mana_pct - 60.0).abs() < 0.1);
        assert!((snap.target_hp_pct.unwrap() - 50.0).abs() < 0.1);
        assert!(!snap.target_is_dead);
    }

    #[test]
    fn test_register_client_generates_unique_tokens() {
        let mut orch = Orchestrator::new();
        let token_a = orch.register_client(100);
        let token_b = orch.register_client(101);
        // CSPRNG tokens should be different
        assert_ne!(token_a, token_b);
        // Client PID should be tracked
        assert!(orch.client_pids.contains(&100));
        assert!(orch.client_pids.contains(&101));
        // Token should be stored
        assert_eq!(orch.session_tokens[&100], token_a);
    }

    #[cfg(not(windows))]
    #[test]
    fn failed_ipc_dispatch_records_error_for_monitored_session() {
        let mut orch = Orchestrator::new();
        orch.bind_monitored_client(7, 100);
        let _token = orch.register_client(100);
        orch.client_names.insert(100, "Test".to_string());

        orch.send_ipc_command(100, textquest_common::ipc::Command::CombatDisengage);

        let snapshot = orch
            .monitoring_snapshot(7)
            .expect("monitoring snapshot should exist");
        assert_eq!(
            snapshot.state,
            crate::metrics::MonitoredSessionState::Active
        );
        assert_eq!(snapshot.errors.total_errors, 1);
        assert_eq!(
            snapshot.errors.last_error_kind,
            Some(crate::metrics::SessionErrorKind::IpcDispatch)
        );
        assert_eq!(snapshot.ipc_latency.sample_count, 0);
    }

    #[test]
    fn test_stale_state_returns_none_snapshot() {
        use textquest_common::{
            combat::CombatStatus,
            nav::NavStatus,
            types::{GameState, SpawnData},
        };

        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());

        fn make_spawn(hp: i64, hp_max: i64, mana: i32, mana_max: i32) -> SpawnData {
            SpawnData {
                spawn_id: 1,
                name: "Test".into(),
                displayed_name: "Test".into(),
                spawn_type: 0,
                level: 60,
                class_id: 1,
                race_id: 1,
                x: 0.0,
                y: 0.0,
                z: 0.0,
                heading: 0.0,
                hp_current: hp,
                hp_max,
                mana_current: mana,
                mana_max,
                endurance_current: 100,
                endurance_max: 100,
                speed_run: 0.0,
                stand_state: 0,
                is_gm: false,
            }
        }

        // Insert game states
        orch.game_states.insert(
            100,
            GameState {
                client_id: 100,
                local_player: Some(make_spawn(1000, 1000, 0, 0)),
                target: None,
                nearby_spawns: vec![],
                timestamp_ms: 0,
                nav_status: NavStatus::Idle,
                combat_status: CombatStatus::Idle,
                zone_short_name: String::new(),
                zone_long_name: String::new(),
                active_buffs: vec![],
                pet: None,
                actual_version: None,
            },
        );
        orch.game_states.insert(
            101,
            GameState {
                client_id: 101,
                local_player: Some(make_spawn(1000, 1000, 1000, 1000)),
                target: None,
                nearby_spawns: vec![],
                timestamp_ms: 0,
                nav_status: NavStatus::Idle,
                combat_status: CombatStatus::Idle,
                zone_short_name: String::new(),
                zone_long_name: String::new(),
                active_buffs: vec![],
                pet: None,
                actual_version: None,
            },
        );

        // State was updated at tick 1, current tick is 10 — stale by 9 ticks
        orch.state_timestamps.insert(100, 1);
        orch.state_timestamps.insert(101, 1);
        orch.tick_count = 10;

        assert!(
            orch.build_camp_snapshot().is_none(),
            "stale state should return None"
        );

        // Update timestamps to be fresh — snapshot should work
        orch.state_timestamps.insert(100, 9);
        orch.state_timestamps.insert(101, 9);
        assert!(
            orch.build_camp_snapshot().is_some(),
            "fresh state should return Some"
        );
    }

    // --- Task 1: Hunt mode toggle ---

    #[test]
    fn test_hunt_mode_toggle() {
        let mut orch = Orchestrator::new();
        assert_eq!(orch.operating_mode, OperatingMode::Camp);

        orch.set_operating_mode(OperatingMode::Hunt);
        assert_eq!(orch.operating_mode, OperatingMode::Hunt);

        // Tick in hunt mode with no active hunt returns 0
        assert_eq!(orch.tick(), 0);
    }

    #[test]
    fn test_start_hunt_sets_mode() {
        let mut orch = Orchestrator::new();
        orch.start_hunt(test_config(), test_members());
        assert_eq!(orch.operating_mode, OperatingMode::Hunt);
        assert!(orch.active_hunt.is_some());
    }

    #[test]
    fn test_hunt_tick_generates_commands() {
        let mut orch = Orchestrator::new();
        orch.start_hunt(test_config(), test_members());
        // First tick transitions Roaming -> Engaging
        let count = orch.tick();
        assert!(count > 0);
    }

    #[test]
    fn test_camp_status_hunt_mode() {
        let mut orch = Orchestrator::new();
        orch.start_hunt(test_config(), test_members());
        let status = orch.camp_status();
        assert!(status.contains("Hunt"));
    }

    #[test]
    fn test_camp_mode_tick_still_works() {
        let mut orch = Orchestrator::new();
        orch.set_operating_mode(OperatingMode::Camp);
        orch.start_camp(test_config(), test_members());
        let count = orch.tick();
        assert!(count > 0, "Camp mode tick should still produce commands");
    }

    // --- Task 2: Vendor sell cycle ---

    #[test]
    fn test_sell_cycle_integration() {
        let mut orch = Orchestrator::new();
        let vendor_config = VendorConfig {
            vendor_name: "Merchant_Leah".into(),
            sell_interval_ticks: 10,
            keep_items: vec![],
            travel_ticks: 2,
            sellable_items: vec![],
            sell_step_delay: 1,
            return_spell: None,
            navigation_timeout_ticks: 10,
            vendor_retry_ticks: 1,
            max_busy_retries: 3,
            backlog_days: 30,
            watch_items: vec![],
        };
        orch.start_sell_cycle(vendor_config);
        assert!(orch.sell_cycle.is_some());
    }

    #[test]
    fn test_sell_cycle_only_during_downtime() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        let vendor_config = VendorConfig {
            vendor_name: "Merchant_Leah".into(),
            sell_interval_ticks: 1, // trigger immediately
            keep_items: vec![],
            travel_ticks: 1,
            sellable_items: vec![],
            sell_step_delay: 1,
            return_spell: None,
            navigation_timeout_ticks: 10,
            vendor_retry_ticks: 1,
            max_busy_retries: 3,
            backlog_days: 30,
            watch_items: vec![],
        };
        orch.start_sell_cycle(vendor_config);

        // First tick goes Idle -> Pulling, sell cycle should not interfere
        orch.tick();
        // Camp should be in Pulling state, sell cycle should still be NotNeeded
        // (it only triggers during Idle/Medding)
        assert!(orch.active_camp.is_some());
    }

    #[test]
    fn test_sell_cycle_status_display() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        let vendor_config = VendorConfig {
            vendor_name: "Merchant_Leah".into(),
            sell_interval_ticks: 10,
            keep_items: vec![],
            travel_ticks: 2,
            sellable_items: vec![],
            sell_step_delay: 1,
            return_spell: None,
            navigation_timeout_ticks: 10,
            vendor_retry_ticks: 1,
            max_busy_retries: 3,
            backlog_days: 30,
            watch_items: vec![],
        };
        orch.start_sell_cycle(vendor_config);

        // Manually trigger selling state to check display
        if let Some(ref mut sc) = orch.sell_cycle {
            sc.start_sell(0);
        }
        let status = orch.camp_status();
        assert!(
            status.contains("[selling]"),
            "Status should show selling indicator"
        );
    }

    // --- Task 3: Buff rebuffing (tested via state.rs, verify integration) ---

    #[test]
    fn test_camp_loop_has_buff_tracker() {
        let camp = CampLoop::new(test_config(), test_members());
        assert!(camp.buff_tracker.last_cast.is_empty());
        assert!(camp.class_configs.is_empty());
    }

    // --- Task 4: Camp progression ---

    #[test]
    fn test_progression_suggested_camp() {
        let mut orch = Orchestrator::new();
        // No camp DB loaded — no suggestion
        orch.check_camp_progression();
        assert!(orch.suggested_camp.is_none());
    }

    #[test]
    fn test_progression_status_display() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        orch.suggested_camp = Some("unrest_yard".into());
        let status = orch.camp_status();
        assert!(status.contains("[suggest: unrest_yard]"));
    }

    // --- Task 5: Event production ---

    #[test]
    fn test_event_production_only_during_combat() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        // Camp starts in Idle — no events should be produced
        orch.produce_camp_events(&None);
        assert!(
            orch.active_camp.as_ref().unwrap().pending_events.is_empty(),
            "No events during Idle"
        );
    }

    #[test]
    fn test_charm_break_detection() {
        use crate::camp::cc::{CcTarget, CcType};
        use textquest_common::combat::HateTargetCategory;

        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());

        // Set camp to Fighting state
        if let Some(ref mut camp) = orch.active_camp {
            camp.state = CampState::Fighting { started_tick: 1 };
            // Add a charmed mob to CC tracker
            camp.cc_tracker.targets.push(CcTarget {
                spawn_id: 42,
                name: "charmed pet".into(),
                cc_applied: Some(CcType::Charm),
                cc_expiry_tick: 100,
                assigned_to_pid: Some(102),
                debuffed: false,
                category: HateTargetCategory::default(),
            });
        }

        // Record previous state with charm active
        orch.prev_cc_state.insert(42, CcType::Charm);

        // Now remove the charm (simulating a charm break)
        if let Some(ref mut camp) = orch.active_camp {
            camp.cc_tracker.targets[0].cc_applied = None;
        }

        orch.produce_camp_events(&None);

        // Should have pushed a CharmBreak event
        let events = &orch.active_camp.as_ref().unwrap().pending_events;
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CampEvent::CharmBreak { spawn_id: 42 })),
            "Should detect charm break"
        );
    }

    #[test]
    fn test_add_detection() {
        use textquest_common::{
            combat::CombatStatus,
            nav::NavStatus,
            types::{GameState, SpawnData},
        };

        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());

        // Set camp to Fighting state
        if let Some(ref mut camp) = orch.active_camp {
            camp.state = CampState::Fighting { started_tick: 1 };
        }

        // Insert game state with nearby spawns for the tank (pid 100)
        let new_npc = SpawnData {
            spawn_id: 99,
            name: "an orc centurion".into(),
            displayed_name: "an orc centurion".into(),
            spawn_type: 1, // NPC
            level: 10,
            class_id: 1,
            race_id: 1,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 1000,
            hp_max: 1000,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 100,
            endurance_max: 100,
            speed_run: 0.0,
            stand_state: 0,
            is_gm: false,
        };
        orch.game_states.insert(
            100,
            GameState {
                client_id: 100,
                local_player: Some(SpawnData {
                    spawn_id: 1,
                    name: "Tank".into(),
                    displayed_name: "Tank".into(),
                    spawn_type: 0,
                    level: 60,
                    class_id: 1,
                    race_id: 1,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    heading: 0.0,
                    hp_current: 1000,
                    hp_max: 1000,
                    mana_current: 0,
                    mana_max: 0,
                    endurance_current: 100,
                    endurance_max: 100,
                    speed_run: 0.0,
                    stand_state: 0,
                    is_gm: false,
                }),
                target: None,
                nearby_spawns: vec![new_npc],
                timestamp_ms: 0,
                nav_status: NavStatus::Idle,
                combat_status: CombatStatus::Idle,
                zone_short_name: String::new(),
                zone_long_name: String::new(),
                active_buffs: vec![],
                pet: None,
                actual_version: None,
            },
        );

        // First call: records spawns as prev
        orch.produce_camp_events(&None);
        // Clear any events from first detection (first time seeing spawn 99)
        let events = &orch.active_camp.as_ref().unwrap().pending_events;
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CampEvent::AddSpawned { spawn_id: 99, .. })),
            "Should detect new add"
        );
    }

    #[test]
    fn test_cc_expiry_detection() {
        use crate::camp::cc::CcTarget;
        use textquest_common::combat::HateTargetCategory;

        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());

        // Set camp to Fighting state with a mezzed mob about to expire
        if let Some(ref mut camp) = orch.active_camp {
            camp.state = CampState::Fighting { started_tick: 1 };
            camp.tick = 18; // Current tick
            camp.cc_tracker.targets.push(CcTarget {
                spawn_id: 55,
                name: "mezzed orc".into(),
                cc_applied: Some(CcType::Mez),
                cc_expiry_tick: 20, // 2 ticks away, within CC_EXPIRY_BUFFER (3)
                assigned_to_pid: Some(102),
                debuffed: false,
                category: HateTargetCategory::default(),
            });
        }

        orch.produce_camp_events(&None);

        let events = &orch.active_camp.as_ref().unwrap().pending_events;
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CampEvent::CcExpiring { spawn_id: 55 })),
            "Should detect CC about to expire"
        );
    }

    // ── M8 routing scope tests ─────────────────────────────────────────────

    #[test]
    fn pids_in_scope_all_session_returns_all_clients() {
        let mut orch = Orchestrator::new();
        orch.client_pids = vec![1, 2, 3];
        orch.routing_scope = RoutingScope::AllSession;
        assert_eq!(orch.pids_in_scope(), vec![1, 2, 3]);
    }

    #[test]
    fn pids_in_scope_narrowed_uses_scope_pids() {
        let mut orch = Orchestrator::new();
        orch.client_pids = vec![1, 2, 3];
        orch.routing_scope = RoutingScope::OneToon {
            name: "Cleric".to_string(),
        };
        orch.scope_pids = vec![2];
        assert_eq!(orch.pids_in_scope(), vec![2]);
    }

    #[test]
    fn pids_in_scope_empty_scope_pids_falls_back_to_all() {
        let mut orch = Orchestrator::new();
        orch.client_pids = vec![1, 2, 3];
        orch.routing_scope = RoutingScope::OneToon {
            name: "Ghost".to_string(),
        };
        // scope_pids is empty (unknown toon) → fall back to all clients
        orch.scope_pids = Vec::new();
        assert_eq!(orch.pids_in_scope(), vec![1, 2, 3]);
    }

    #[test]
    fn default_routing_scope_is_all_session() {
        let orch = Orchestrator::new();
        assert_eq!(orch.routing_scope, RoutingScope::AllSession);
        assert!(orch.scope_pids.is_empty());
    }

    #[test]
    fn camp_integration_vendor_cycle_uses_inventory_plan() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        orch.set_vendor_inventory(vec![crate::loot::vendor_cycle::VendorInventoryItem::trash(
            "Torn Cloth Sandal",
            1,
            9,
        )]);
        orch.start_sell_cycle(VendorConfig {
            vendor_name: "Merchant_Leah".into(),
            sell_interval_ticks: 1,
            keep_items: vec![],
            travel_ticks: 2,
            sellable_items: vec![],
            sell_step_delay: 1,
            return_spell: None,
            navigation_timeout_ticks: 10,
            vendor_retry_ticks: 1,
            max_busy_retries: 3,
            backlog_days: 30,
            watch_items: vec![],
        });
        orch.tick_count = 1;

        let cmds = orch.tick_sell_cycle();

        assert!(
            cmds.iter().any(|(_, cmd)| cmd.contains("/nav target")),
            "integration path should issue vendor navigation commands"
        );
    }

    #[test]
    fn camp_integration_vendor_cycle_preserves_timer_fallback_without_inventory() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        orch.start_sell_cycle(VendorConfig {
            vendor_name: "Merchant_Leah".into(),
            sell_interval_ticks: 1,
            keep_items: vec![],
            travel_ticks: 2,
            sellable_items: vec![],
            sell_step_delay: 1,
            return_spell: None,
            navigation_timeout_ticks: 10,
            vendor_retry_ticks: 1,
            max_busy_retries: 3,
            backlog_days: 30,
            watch_items: vec![],
        });
        orch.tick_count = 1;

        let cmds = orch.tick_sell_cycle();

        assert!(
            cmds.iter().any(|(_, cmd)| cmd.contains("/nav target")),
            "timer fallback should still enter the vendor cycle when inventory snapshots are absent"
        );
    }

    #[test]
    fn cross_group_rez_dispatches_same_zone_cleric_once() {
        let mut orch = Orchestrator::new();

        for pid in [100, 101, 200, 201] {
            orch.register_client(pid);
        }

        orch.client_names.insert(100, "Warrior01".into());
        orch.client_names.insert(101, "Cleric01".into());
        orch.client_names.insert(200, "Warrior02".into());
        orch.client_names.insert(201, "Cleric02".into());

        orch.set_client_group(100, 1);
        orch.set_client_group(101, 1);
        orch.set_client_group(200, 2);
        orch.set_client_group(201, 2);

        orch.set_client_class_name(100, "Warrior");
        orch.set_client_class_name(101, "Cleric");
        orch.set_client_class_name(200, "Warrior");
        orch.set_client_class_name(201, "Cleric");

        orch.game_states.insert(
            100,
            make_game_state(
                100,
                "crushbone",
                make_spawn_named("Warrior01", 1, 1800, 2000),
                None,
            ),
        );
        orch.game_states.insert(
            101,
            make_game_state(
                101,
                "crushbone",
                make_spawn_named("Cleric01", 2, 1500, 2000),
                None,
            ),
        );
        orch.game_states.insert(
            200,
            make_game_state(
                200,
                "crushbone",
                make_spawn_named("Warrior02", 1, 0, 2000),
                None,
            ),
        );
        orch.game_states.insert(
            201,
            make_game_state(
                201,
                "crushbone",
                make_spawn_named("Cleric02", 2, 0, 2000),
                None,
            ),
        );
        mark_states_fresh(&mut orch, &[100, 101, 200, 201]);

        let first_tick = orch.tick();
        assert_eq!(first_tick, 2, "expected rez target + cast commands");
        assert!(
            orch.last_dispatched
                .iter()
                .any(|(pid, action)| *pid == 101 && action == "/target Cleric02")
        );
        assert!(
            orch.last_dispatched
                .iter()
                .any(|(pid, action)| *pid == 101 && action == "/cast 5")
        );

        let second_tick = orch.tick();
        assert_eq!(second_tick, 0, "rez request should not spam every tick");
    }

    #[test]
    fn cross_group_assist_dispatches_same_zone_attackers_once() {
        let mut orch = Orchestrator::new();

        for pid in [110, 111, 210, 211] {
            orch.register_client(pid);
        }

        orch.client_names.insert(110, "Warrior10".into());
        orch.client_names.insert(111, "Cleric10".into());
        orch.client_names.insert(210, "Warrior20".into());
        orch.client_names.insert(211, "Ranger20".into());

        orch.set_client_group(110, 1);
        orch.set_client_group(111, 1);
        orch.set_client_group(210, 2);
        orch.set_client_group(211, 2);

        orch.set_client_class_name(110, "Warrior");
        orch.set_client_class_name(111, "Cleric");
        orch.set_client_class_name(210, "Warrior");
        orch.set_client_class_name(211, "Ranger");

        let rescue_target = Some(make_spawn_named("an_orc_centurion", 1, 900, 1000));
        orch.game_states.insert(
            110,
            make_game_state(
                110,
                "crushbone",
                make_spawn_named("Warrior10", 1, 300, 2000),
                rescue_target.clone(),
            ),
        );
        orch.game_states.insert(
            111,
            make_game_state(
                111,
                "crushbone",
                make_spawn_named("Cleric10", 2, 500, 2000),
                None,
            ),
        );
        orch.game_states.insert(
            210,
            make_game_state(
                210,
                "crushbone",
                make_spawn_named("Warrior20", 1, 1800, 2000),
                None,
            ),
        );
        orch.game_states.insert(
            211,
            make_game_state(
                211,
                "crushbone",
                make_spawn_named("Ranger20", 4, 1600, 2000),
                None,
            ),
        );
        mark_states_fresh(&mut orch, &[110, 111, 210, 211]);

        let first_tick = orch.tick();
        assert_eq!(first_tick, 4, "expected assist + attack for each responder");
        assert!(
            orch.last_dispatched
                .iter()
                .any(|(pid, action)| *pid == 210 && action == "/assist Warrior10")
        );
        assert!(
            orch.last_dispatched
                .iter()
                .any(|(pid, action)| *pid == 211 && action == "/assist Warrior10")
        );

        let second_tick = orch.tick();
        assert_eq!(second_tick, 0, "assist request should not spam every tick");
    }

    #[test]
    fn cross_group_coordination_skips_other_zones() {
        let mut orch = Orchestrator::new();

        for pid in [120, 121, 220] {
            orch.register_client(pid);
        }

        orch.client_names.insert(120, "Warrior12".into());
        orch.client_names.insert(121, "Cleric12".into());
        orch.client_names.insert(220, "Cleric22".into());

        orch.set_client_group(120, 1);
        orch.set_client_group(121, 1);
        orch.set_client_group(220, 2);

        orch.set_client_class_name(120, "Warrior");
        orch.set_client_class_name(121, "Cleric");
        orch.set_client_class_name(220, "Cleric");

        orch.game_states.insert(
            120,
            make_game_state(
                120,
                "mistmoore",
                make_spawn_named("Warrior12", 1, 0, 2000),
                None,
            ),
        );
        orch.game_states.insert(
            121,
            make_game_state(
                121,
                "mistmoore",
                make_spawn_named("Cleric12", 2, 0, 2000),
                None,
            ),
        );
        orch.game_states.insert(
            220,
            make_game_state(
                220,
                "guktop",
                make_spawn_named("Cleric22", 2, 1700, 2000),
                None,
            ),
        );
        mark_states_fresh(&mut orch, &[120, 121, 220]);

        assert_eq!(orch.tick(), 0, "different-zone groups must not coordinate");
        assert!(orch.last_dispatched.is_empty());
    }

    #[test]
    fn cross_group_priority_reserves_responder_for_rez_before_assist() {
        let mut orch = Orchestrator::new();

        for pid in [130, 131, 230, 231, 330, 331] {
            orch.register_client(pid);
        }

        orch.client_names.insert(130, "Warrior13".into());
        orch.client_names.insert(131, "Cleric13".into());
        orch.client_names.insert(230, "Warrior23".into());
        orch.client_names.insert(231, "Cleric23".into());
        orch.client_names.insert(330, "Warrior33".into());
        orch.client_names.insert(331, "Cleric33".into());

        orch.set_client_group(130, 1);
        orch.set_client_group(131, 1);
        orch.set_client_group(230, 2);
        orch.set_client_group(231, 2);
        orch.set_client_group(330, 3);
        orch.set_client_group(331, 3);

        orch.set_client_class_name(130, "Warrior");
        orch.set_client_class_name(131, "Cleric");
        orch.set_client_class_name(230, "Warrior");
        orch.set_client_class_name(231, "Cleric");
        orch.set_client_class_name(330, "Warrior");
        orch.set_client_class_name(331, "Cleric");

        orch.game_states.insert(
            130,
            make_game_state(
                130,
                "crushbone",
                make_spawn_named("Warrior13", 1, 0, 2000),
                None,
            ),
        );
        orch.game_states.insert(
            131,
            make_game_state(
                131,
                "crushbone",
                make_spawn_named("Cleric13", 2, 0, 2000),
                None,
            ),
        );

        let mut add_target = make_spawn_named("an_orc_legionnaire", 1, 900, 1000);
        add_target.spawn_type = 1;
        orch.game_states.insert(
            230,
            make_game_state(
                230,
                "crushbone",
                make_spawn_named("Warrior23", 1, 400, 2000),
                Some(add_target.clone()),
            ),
        );
        orch.game_states.insert(
            231,
            make_game_state(
                231,
                "crushbone",
                make_spawn_named("Cleric23", 2, 500, 2000),
                None,
            ),
        );
        orch.game_states.insert(
            330,
            make_game_state(
                330,
                "crushbone",
                make_spawn_named("Warrior33", 1, 1800, 2000),
                None,
            ),
        );
        orch.game_states.insert(
            331,
            make_game_state(
                331,
                "crushbone",
                make_spawn_named("Cleric33", 2, 1700, 2000),
                None,
            ),
        );
        mark_states_fresh(&mut orch, &[130, 131, 230, 231, 330, 331]);

        let first_tick = orch.tick();
        assert_eq!(
            first_tick, 2,
            "single responder group should be reserved for the higher-priority rez"
        );
        assert!(
            orch.last_dispatched
                .iter()
                .any(|(pid, action)| *pid == 331 && action == "/target Cleric13")
        );
        assert!(
            orch.last_dispatched
                .iter()
                .all(|(_, action)| !action.starts_with("/assist "))
        );
    }

    #[test]
    fn cross_group_ignores_stale_group_state() {
        let mut orch = Orchestrator::new();

        for pid in [140, 141, 240, 241] {
            orch.register_client(pid);
        }

        orch.client_names.insert(140, "Warrior14".into());
        orch.client_names.insert(141, "Cleric14".into());
        orch.client_names.insert(240, "Warrior24".into());
        orch.client_names.insert(241, "Cleric24".into());

        orch.set_client_group(140, 1);
        orch.set_client_group(141, 1);
        orch.set_client_group(240, 2);
        orch.set_client_group(241, 2);

        orch.set_client_class_name(140, "Warrior");
        orch.set_client_class_name(141, "Cleric");
        orch.set_client_class_name(240, "Warrior");
        orch.set_client_class_name(241, "Cleric");

        orch.game_states.insert(
            140,
            make_game_state(
                140,
                "crushbone",
                make_spawn_named("Warrior14", 1, 0, 2000),
                None,
            ),
        );
        orch.game_states.insert(
            141,
            make_game_state(
                141,
                "crushbone",
                make_spawn_named("Cleric14", 2, 0, 2000),
                None,
            ),
        );
        orch.game_states.insert(
            240,
            make_game_state(
                240,
                "crushbone",
                make_spawn_named("Warrior24", 1, 1800, 2000),
                None,
            ),
        );
        orch.game_states.insert(
            241,
            make_game_state(
                241,
                "crushbone",
                make_spawn_named("Cleric24", 2, 1700, 2000),
                None,
            ),
        );

        orch.tick_count = STALE_TICK_THRESHOLD + 5;
        orch.state_timestamps.insert(140, 1);
        orch.state_timestamps.insert(141, 1);
        orch.state_timestamps.insert(240, orch.tick_count);
        orch.state_timestamps.insert(241, orch.tick_count);

        assert_eq!(orch.tick(), 0, "stale groups must not request rescue");
        assert!(orch.last_dispatched.is_empty());
    }

    #[cfg(not(windows))]
    #[test]
    fn poll_chat_log_if_due_forwards_ipc_messages_to_disk() {
        clear_test_ipc_responses();

        let pid = 1000u32;
        let mut orch = Orchestrator::new();
        orch.register_client(pid);
        orch.client_names.insert(pid, "ChatSink".into());
        orch.game_states.insert(
            pid,
            make_game_state(
                pid,
                "qeynos",
                make_spawn_named("ChatSink", 1, 1000, 1000),
                None,
            ),
        );

        let tempdir = tempfile::tempdir().expect("tempdir");
        let mut chat_config = ChatLogConfig::default();
        chat_config.enabled = true;
        chat_config.channels = vec![ChatChannel::Say];
        orch.init_chat_log_manager(chat_config, tempdir.path().to_path_buf());

        queue_test_ipc_response(
            pid,
            1,
            Response::ChatBatch {
                messages: vec![ChatMessageInfo {
                    text: "ChatSink says, 'hello from test'".to_string(),
                    color: 273,
                    timestamp_ms: 1_700_000_000_000,
                }],
            },
        );

        orch.poll_chat_log_if_due();

        {
            let manager = orch
                .chat_log_manager
                .as_mut()
                .expect("chat log manager exists");
            manager.close_writer("qeynos", "ChatSink");
        }

        let log_file = tempdir.path().join("qeynos_ChatSink.log");
        let log_contents =
            std::fs::read_to_string(&log_file).expect("chat log file should be written");

        assert!(log_contents.contains("hello from test"));
    }
}
