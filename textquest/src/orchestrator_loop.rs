//! Top-level orchestrator event loop — wires ClientManager, LaunchCoordinator,
//! and Orchestrator into a single async tick loop with health checks, crash
//! recovery, and graceful shutdown.

use crate::{
    client::{
        death_camp::{AutoCampOnDeathSettings, DeathCampAction, DeathCampTracker},
        discovery::PeerDiscoveryEvent,
        healing::ClientHealth,
        manager::ClientManager,
        session::SlotLifecycle,
    },
    config::{AppConfig, OrchestratorConfig},
    credentials::store::CredentialStore,
    discord::webhook::WebhookSender,
    launcher::coordinator::{CoordinatorEvent, LaunchCoordinator},
    orchestrator::Orchestrator,
};
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use textquest_common::{combat::CombatStatus, ipc::Command, login::RelogConfig};
use tokio::sync::watch;
use zeroize::Zeroizing;

const DEAD_STAND_STATE: u8 = 111;

enum RelogDispatchOutcome {
    Handled,
    RetryableFailure,
}

/// Events emitted by the orchestrator loop for external consumers (TUI,
/// logging).
#[derive(Debug, Clone)]
pub enum LoopEvent {
    /// A client was detected as unhealthy and sent `/camp desktop`.
    ClientCamped { pid: u32 },
    /// A crashed client was re-enqueued for relaunch.
    ClientRequeued { pid: u32 },
    /// A newly-launched client was registered with the orchestrator.
    ClientRegistered { pid: u32 },
    /// A remote multicast peer became visible.
    PeerDiscovered { node_name: String, sessions: usize },
    /// A previously-visible multicast peer expired.
    PeerExpired { node_name: String },
    /// Health check completed for all clients.
    HealthCheckDone { healthy: usize, unhealthy: usize },
    /// The loop is shutting down.
    ShuttingDown,
}

/// The top-level event loop that ties all subsystems together.
pub struct OrchestratorLoop {
    pub client_manager: ClientManager,
    pub launch_coordinator: LaunchCoordinator,
    pub orchestrator: Orchestrator,
    config: OrchestratorConfig,
    auto_camp_settings: HashMap<String, AutoCampOnDeathSettings>,
    default_server_name: String,
    death_camp_tracker: DeathCampTracker,
    credential_store: Option<CredentialStore>,
    shared_password: Option<Zeroizing<String>>,
    status_webhook: Option<WebhookSender>,
    timing_correction_enabled: bool,
    shutdown_rx: watch::Receiver<bool>,
    timestamp_runtime: crate::timestamp_runtime::TimestampRuntime,
    window_title_runtime: crate::window_title_runtime::WindowTitleRuntime,
}

impl OrchestratorLoop {
    fn is_missing_account_lookup(error: &anyhow::Error) -> bool {
        error
            .chain()
            .any(|cause| cause.to_string().contains("not found"))
    }

    /// Create a new orchestrator loop.
    ///
    /// `shutdown_rx` receives `true` when the loop should stop.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        client_manager: ClientManager,
        launch_coordinator: LaunchCoordinator,
        orchestrator: Orchestrator,
        config: OrchestratorConfig,
        auto_camp_settings: HashMap<String, AutoCampOnDeathSettings>,
        default_server_name: String,
        credential_store: Option<CredentialStore>,
        shared_password: Option<Zeroizing<String>>,
        status_webhook: Option<WebhookSender>,
        timing_correction_enabled: bool,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            client_manager,
            launch_coordinator,
            orchestrator,
            config,
            auto_camp_settings,
            default_server_name,
            death_camp_tracker: DeathCampTracker::new(),
            credential_store,
            shared_password,
            status_webhook,
            timing_correction_enabled,
            shutdown_rx,
            timestamp_runtime: crate::timestamp_runtime::TimestampRuntime::new(),
            window_title_runtime: crate::window_title_runtime::WindowTitleRuntime::new(),
        }
    }

    /// Build from an `AppConfig` and a shutdown channel.
    pub fn from_config(app_config: &AppConfig, shutdown_rx: watch::Receiver<bool>) -> Self {
        let client_manager = ClientManager::new(&app_config.process_name)
            .with_discovery_config(&app_config.discovery);
        let launch_coordinator = LaunchCoordinator::new(
            app_config.launch.clone(),
            app_config.retry.clone(),
            app_config.server.clone(),
        );
        let mut orchestrator = Orchestrator::new();
        let log_dir = crate::paths::resolve_log_dir().join("chat");
        orchestrator.init_chat_log_manager(app_config.chat_log.clone(), log_dir);
        orchestrator.configure_say_detection(&app_config.say_detection);
        let auto_camp_settings = app_config
            .group
            .iter()
            .flat_map(|group| group.toon.iter())
            .map(|toon| {
                (toon.name.to_ascii_lowercase(), toon.auto_camp_on_death.clone())
            })
            .collect::<HashMap<_, _>>();
        let credential_store = std::env::var("TEXTQUEST_MASTER_PASSWORD")
            .ok()
            .filter(|password| !password.trim().is_empty())
            .and_then(|password| match CredentialStore::open_default(&password) {
                Ok(store) => Some(store),
                Err(error) => {
                    tracing::error!(
                        error = %error,
                        "Failed to initialize unattended relog credential store"
                    );
                    None
                }
            });
        let shared_password = std::env::var("TEXTQUEST_PASSWORD")
            .ok()
            .filter(|password| !password.trim().is_empty())
            .map(Zeroizing::new);
        let status_webhook = (app_config.discord.alert_status
            && !app_config.discord.webhook_url.trim().is_empty())
        .then(|| {
            WebhookSender::with_channels(
                app_config.discord.webhook_url.clone(),
                app_config.discord.channels.clone(),
            )
        });

        Self::new(
            client_manager,
            launch_coordinator,
            orchestrator,
            app_config.orchestrator.clone(),
            auto_camp_settings,
            app_config.server.name.clone(),
            credential_store,
            shared_password,
            status_webhook,
            app_config.timing_correction,
            shutdown_rx,
        )
    }

    /// Run the event loop until shutdown is signaled.
    ///
    /// Returns accumulated events from the final tick (or empty on clean
    /// shutdown).
    pub async fn run(&mut self) -> Vec<LoopEvent> {
        tracing::info!(
            health_check_ms = self.config.health_check_interval_ms,
            launch_tick_ms = self.config.launch_tick_interval_ms,
            orchestrator_tick_ms = self.config.orchestrator_tick_interval_ms,
            "Orchestrator loop starting"
        );

        let mut health_interval =
            tokio::time::interval(Duration::from_millis(self.config.health_check_interval_ms));
        let mut launch_interval =
            tokio::time::interval(Duration::from_millis(self.config.launch_tick_interval_ms));
        let mut orch_interval = tokio::time::interval(Duration::from_millis(
            self.config.orchestrator_tick_interval_ms,
        ));

        // Don't burst-fire missed ticks
        health_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        launch_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        orch_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = health_interval.tick() => {
                    let events = self.tick_health_checks();
                    for event in &events {
                        tracing::debug!(?event, "orchestrator loop event");
                    }
                    self.sync_box_chat_runtime();
                    self.sync_timestamp_runtime();
                    self.sync_window_title_runtime();
                }
                _ = launch_interval.tick() => {
                    let events = self.tick_launch_coordinator();
                    for event in &events {
                        tracing::debug!(?event, "orchestrator loop event");
                    }
                    self.sync_box_chat_runtime();
                    self.sync_timestamp_runtime();
                    self.sync_window_title_runtime();
                }
                _ = orch_interval.tick() => {
                    let events = self.tick_peer_discovery();
                    for event in &events {
                        tracing::debug!(?event, "orchestrator loop event");
                    }
                    self.orchestrator.tick();
                    self.tick_death_camp();
                    self.sync_box_chat_runtime();
                    self.sync_timestamp_runtime();
                    self.sync_window_title_runtime();
                }
                Ok(()) = self.shutdown_rx.changed() => {
                    if *self.shutdown_rx.borrow() {
                        tracing::info!("Shutdown signal received — stopping orchestrator loop");
                        return vec![LoopEvent::ShuttingDown];
                    }
                }
            }
        }
    }

    fn tick_peer_discovery(&mut self) -> Vec<LoopEvent> {
        self.client_manager
            .tick_peer_discovery()
            .into_iter()
            .map(|event| match event {
                PeerDiscoveryEvent::PeerDiscovered {
                    node_name,
                    session_count,
                    ..
                } => LoopEvent::PeerDiscovered {
                    node_name,
                    sessions: session_count,
                },
                PeerDiscoveryEvent::PeerExpired { node_name, .. } => {
                    LoopEvent::PeerExpired { node_name }
                }
            })
            .collect()
    }

    fn tick_death_camp(&mut self) {
        #[derive(Debug, Clone)]
        struct DeathCampObservation {
            client_id: u32,
            pid: u32,
            character_name: String,
            account_name: Option<String>,
            server_name: Option<String>,
            is_dead: bool,
            settings: AutoCampOnDeathSettings,
        }

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        let observations =
            self.client_manager
                .all_sessions()
                .filter_map(|session| {
                    let character_name = session.character_name.clone().or_else(|| {
                        session
                            .bound_toon
                            .as_ref()
                            .map(|toon| toon.character_name.clone())
                    })?;
                    let settings = self
                        .auto_camp_settings
                        .get(&character_name.to_ascii_lowercase())
                        .cloned()
                        .unwrap_or(AutoCampOnDeathSettings {
                            enabled: false,
                            camp_delay_secs: 30,
                            relog_wait_secs: 900,
                        });
                    let is_dead =
                        self.orchestrator
                            .game_states
                            .get(&session.pid)
                            .is_some_and(|state| {
                                matches!(state.combat_status, CombatStatus::Dead)
                                    || state.local_player.as_ref().is_some_and(|player| {
                                        player.stand_state == DEAD_STAND_STATE
                                    })
                            });

                    Some(DeathCampObservation {
                        client_id: session.client_id,
                        pid: session.pid,
                        character_name,
                        account_name: session
                            .bound_toon
                            .as_ref()
                            .map(|toon| toon.account_name.clone())
                            .or_else(|| session.account_name.clone()),
                        server_name: session
                            .bound_toon
                            .as_ref()
                            .map(|toon| toon.server_name.clone()),
                        is_dead,
                        settings,
                    })
                })
                .collect::<Vec<_>>();

        let active_client_ids = observations
            .iter()
            .map(|observation| observation.client_id)
            .collect::<HashSet<_>>();
        self.death_camp_tracker.retain_clients(&active_client_ids);

        for observation in observations {
            let actions = self.death_camp_tracker.observe(
                observation.client_id,
                &observation.character_name,
                observation.is_dead,
                &observation.settings,
                now_secs,
            );
            for action in actions {
                match action {
                    DeathCampAction::SendAlert {
                        character_name,
                        camp_delay_secs,
                        relog_wait_secs,
                    } => self.send_death_camp_alert(
                        &character_name,
                        camp_delay_secs,
                        relog_wait_secs,
                    ),
                    DeathCampAction::TriggerRelog {
                        character_name,
                        relog_wait_secs,
                    } => {
                        if matches!(
                            self.trigger_death_relog(
                                observation.pid,
                                &character_name,
                                observation.account_name.as_deref(),
                                observation.server_name.as_deref(),
                                relog_wait_secs,
                            ),
                            RelogDispatchOutcome::Handled
                        ) {
                            self.death_camp_tracker
                                .mark_relog_handled(observation.client_id);
                        }
                    }
                }
            }
        }
    }

    fn sync_box_chat_runtime(&self) {
        crate::box_chat::update_local_clients(
            self.orchestrator
                .client_names
                .iter()
                .map(|(pid, name)| (*pid, name.clone())),
        );
        match crate::box_chat::reload_from_disk() {
            Ok(Some(config)) => {
                tracing::info!(?config, "Reloaded box-chat config from disk");
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%error, "Failed to reload box-chat config from disk");
            }
        }
    }

    fn send_death_camp_alert(
        &self,
        character_name: &str,
        camp_delay_secs: u64,
        relog_wait_secs: u64,
    ) {
        if let Some(webhook) = &self.status_webhook {
            webhook.warn(
                "Character death detected",
                &format!(
                    "{character_name} died. Camping to desktop in {camp_delay_secs}s and re-login \
                     will wait {relog_wait_secs}s."
                ),
            );
        }
    }

    fn trigger_death_relog(
        &mut self,
        pid: u32,
        character_name: &str,
        account_name: Option<&str>,
        server_name: Option<&str>,
        relog_wait_secs: u64,
    ) -> RelogDispatchOutcome {
        let Some(account_name) = account_name.filter(|name| !name.trim().is_empty()) else {
            tracing::warn!(
                pid,
                character_name,
                "Cannot auto-relog after death without an account name"
            );
            return RelogDispatchOutcome::Handled;
        };

        let Some(password) = self.resolve_password(account_name) else {
            tracing::warn!(
                pid,
                character_name,
                account_name,
                "Cannot auto-relog after death without credentials"
            );
            if let Some(webhook) = &self.status_webhook {
                webhook.critical(
                    "Death auto-relog skipped",
                    &format!(
                        "{character_name} could not auto-relog because credentials for account \
                         '{account_name}' are unavailable."
                    ),
                );
            }
            return RelogDispatchOutcome::Handled;
        };

        let mut relog_config = RelogConfig::default();
        relog_config.retry_policy.initial_delay = Duration::from_secs(relog_wait_secs);
        relog_config.retry_policy.max_delay = relog_config
            .retry_policy
            .max_delay
            .max(Duration::from_secs(relog_wait_secs));

        let server_name = server_name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(&self.default_server_name)
            .to_string();

        if self.orchestrator.send_ipc_command(
            pid,
            Command::Relog {
                account_name: account_name.to_string(),
                password,
                server_name,
                character_name: character_name.to_string(),
                config: relog_config,
            },
        ) {
            RelogDispatchOutcome::Handled
        } else {
            RelogDispatchOutcome::RetryableFailure
        }
    }

    fn resolve_password(&self, account_name: &str) -> Option<String> {
        if let Some(store) = &self.credential_store {
            match store.get_password(account_name) {
                Ok(password) => return Some(password.to_string()),
                Err(error) => {
                    if !Self::is_missing_account_lookup(&error) {
                        tracing::warn!(
                            account_name,
                            error = %error,
                            "Credential store lookup failed; shared password fallback disabled"
                        );
                        return None;
                    }
                    tracing::debug!(
                        account_name,
                        error = %error,
                        "Credential store lookup failed; falling back to shared password"
                    );
                }
            }
        }

        self.shared_password
            .as_ref()
            .map(|p| p.as_str().to_string())
    }

    fn sync_timestamp_runtime(&mut self) {
        let updated_configs = self.timestamp_runtime.tick();
        if updated_configs.is_some() {
            tracing::debug!("Timestamp config changed, applying to clients");
        }
        let clients: Vec<_> = self
            .orchestrator
            .client_names
            .iter()
            .map(|(&pid, name)| (pid, name.clone()))
            .collect();
        for (pid, name) in clients {
            self.timestamp_runtime
                .apply_to_client(&mut self.orchestrator, pid, &name);
        }
    }

    fn sync_window_title_runtime(&mut self) {
        let updated_configs = self.window_title_runtime.tick();
        if updated_configs.is_some() {
            tracing::debug!("Window title config changed, applying to clients");
        }

        let clients = self
            .client_manager
            .all_sessions()
            .filter_map(|session| {
                let character_name = session.character_name.clone().or_else(|| {
                    session
                        .bound_toon
                        .as_ref()
                        .map(|toon| toon.character_name.clone())
                })?;
                let server_name = session
                    .bound_toon
                    .as_ref()
                    .map(|toon| toon.server_name.clone())
                    .unwrap_or_else(|| self.default_server_name.clone());
                Some((session.pid, character_name, server_name))
            })
            .collect::<Vec<_>>();

        for (pid, character_name, server_name) in clients {
            self.window_title_runtime.apply_to_client(
                &mut self.orchestrator,
                pid,
                &character_name,
                &server_name,
            );
        }
    }

    /// Run health checks on all managed clients.
    ///
    /// - Unhealthy but alive clients get `/camp desktop`.
    /// - Crashed clients are re-enqueued for relaunch.
    fn tick_health_checks(&mut self) -> Vec<LoopEvent> {
        let mut events = Vec::new();

        let needs_restart = self.client_manager.check_health();
        let total = self.client_manager.session_count();
        let unhealthy_count = needs_restart.len();
        let healthy_count = total.saturating_sub(unhealthy_count);

        for client_id in needs_restart {
            let Some(session) = self.client_manager.get_mut(client_id) else {
                continue;
            };

            let pid = session.pid;

            match session.health_monitor.current_health() {
                ClientHealth::Crashed => {
                    tracing::warn!(pid, client_id, "Client crashed — marking for relaunch");
                    session.slot_lifecycle = SlotLifecycle::Recovering;
                    session.health_monitor.record_restart();

                    // Re-enqueue for relaunch if we have account info
                    if let Some(account) = session.bound_toon.clone() {
                        self.launch_coordinator.enqueue(client_id, account);
                        events.push(LoopEvent::ClientRequeued { pid });
                    }

                    // Remove from orchestrator's active client list
                    self.orchestrator.remove_client(pid);
                }
                ClientHealth::Unresponsive { .. } => {
                    tracing::warn!(
                        pid,
                        client_id,
                        "Client unresponsive — sending /camp desktop"
                    );
                    session.slot_lifecycle = SlotLifecycle::Recovering;

                    // Best-effort: tell the client to camp out
                    self.send_camp_desktop(pid);
                    events.push(LoopEvent::ClientCamped { pid });
                }
                _ => {}
            }
        }

        if total > 0 {
            tracing::debug!(
                healthy = healthy_count,
                unhealthy = unhealthy_count,
                total,
                "Health check complete"
            );
            events.push(LoopEvent::HealthCheckDone {
                healthy: healthy_count,
                unhealthy: unhealthy_count,
            });
        }

        events
    }

    /// Tick the launch coordinator and register newly-ready clients.
    fn tick_launch_coordinator(&mut self) -> Vec<LoopEvent> {
        let mut events = Vec::new();
        let coordinator_events = self.launch_coordinator.tick();

        for event in coordinator_events {
            match event {
                CoordinatorEvent::ClientLaunched { client_id, pid } => {
                    self.client_manager.track_client(client_id, pid);
                    tracing::info!(client_id, pid, "Client launched");
                    if let Some(session) = self.client_manager.get_mut(client_id) {
                        session.slot_lifecycle = SlotLifecycle::Launching;
                    }
                }
                CoordinatorEvent::ClientReady { client_id } => {
                    if let Some(session) = self.client_manager.get_mut(client_id) {
                        let pid = session.pid;
                        session.slot_lifecycle = SlotLifecycle::Live;
                        self.orchestrator.register_client(pid);

                        let mut ready_group_id = None;
                        let mut ready_class_name = None;
                        if let Some(account) = session.bound_toon.as_ref() {
                            if let Ok(group_id) = u8::try_from(account.group_id) {
                                ready_group_id = Some(group_id);
                            } else {
                                tracing::warn!(
                                    client_id,
                                    pid,
                                    group_id = account.group_id,
                                    "Skipping out-of-range group assignment for coordination"
                                );
                            }
                            ready_class_name = Some(account.class_name.clone());
                        }

                        self.orchestrator.update_client_admin_metadata(
                            pid,
                            session.character_name.clone(),
                            ready_group_id,
                            ready_class_name,
                        );

                        if self.timing_correction_enabled {
                            self.orchestrator.send_ipc_command(
                                pid,
                                textquest_common::ipc::Command::SetTimingCorrection {
                                    enabled: true,
                                },
                            );
                        }

                        tracing::info!(
                            client_id,
                            pid,
                            "Client ready — registered with orchestrator"
                        );
                        events.push(LoopEvent::ClientRegistered { pid });
                    }
                }
                CoordinatorEvent::ClientFailed { client_id, error } => {
                    tracing::error!(client_id, ?error, "Client login failed");
                    if let Some(session) = self.client_manager.get_mut(client_id) {
                        session.slot_lifecycle = SlotLifecycle::Blocked;
                    }
                }
                CoordinatorEvent::AllPaused { reason } => {
                    tracing::error!(%reason, "Launch coordinator paused — mass failure detected");
                }
                CoordinatorEvent::AllReady => {
                    tracing::info!("All queued clients are ready");
                }
            }
        }

        events
    }

    /// Best-effort send `/camp desktop` to a client via IPC.
    fn send_camp_desktop(&mut self, pid: u32) {
        // Re-use orchestrator's existing IPC machinery
        self.orchestrator
            .client_names
            .entry(pid)
            .or_insert_with(|| "?".to_string());
        // Ensure the orchestrator knows about this PID for pipe connection
        if !self.orchestrator.client_pids.contains(&pid) {
            self.orchestrator.client_pids.push(pid);
        }
        // The send_slash_command is private, so we use the public send path
        // by temporarily dispatching through the orchestrator's IPC
        let cmd = textquest_common::ipc::Command::SlashCommand {
            command: "/camp desktop".to_string(),
        };
        // Use the pipe directly
        let session_id = self
            .orchestrator
            .session_tokens_get(pid)
            .map(textquest_common::ipc::session_id_from_token);
        if let Some(session_id) = session_id {
            match crate::ipc::pipe::CommandPipe::connect(pid, session_id) {
                Ok(pipe) => {
                    if let Some(token) = self.orchestrator.session_tokens_get(pid)
                        && let Err(e) = pipe.send_raw_token(token)
                    {
                        tracing::warn!(pid, error = %e, "Failed to auth for /camp desktop");
                        return;
                    }
                    if let Err(e) = pipe.send(&cmd) {
                        tracing::warn!(pid, error = %e, "Failed to send /camp desktop");
                    } else {
                        tracing::info!(pid, "Sent /camp desktop to unhealthy client");
                    }
                }
                Err(e) => {
                    tracing::warn!(pid, error = %e, "Failed to connect pipe for /camp desktop");
                }
            }
        } else {
            tracing::warn!(pid, "No session token — cannot send /camp desktop");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OrchestratorConfig;
    use textquest_common::login::AccountInfo;

    fn test_config() -> OrchestratorConfig {
        OrchestratorConfig {
            health_check_interval_ms: 100,
            launch_tick_interval_ms: 50,
            orchestrator_tick_interval_ms: 50,
            state_poll_interval_ms: 50,
        }
    }

    fn make_app_config() -> AppConfig {
        let mut config = AppConfig::default_config();
        config.orchestrator = test_config();
        config
    }

    #[test]
    fn orchestrator_loop_from_config() {
        let config = make_app_config();
        let (_tx, rx) = watch::channel(false);
        let oloop = OrchestratorLoop::from_config(&config, rx);
        assert_eq!(oloop.config.health_check_interval_ms, 100);
        assert_eq!(oloop.config.launch_tick_interval_ms, 50);
        assert_eq!(oloop.client_manager.session_count(), 0);
    }

    #[tokio::test]
    async fn shutdown_signal_stops_loop() {
        let config = make_app_config();
        let (tx, rx) = watch::channel(false);
        let mut oloop = OrchestratorLoop::from_config(&config, rx);

        // Signal shutdown immediately
        tx.send(true).unwrap();

        let events = oloop.run().await;
        assert!(events.iter().any(|e| matches!(e, LoopEvent::ShuttingDown)));
    }

    #[tokio::test]
    async fn loop_ticks_before_shutdown() {
        let config = make_app_config();
        let (tx, rx) = watch::channel(false);
        let mut oloop = OrchestratorLoop::from_config(&config, rx);

        // Let it run briefly then signal shutdown
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            tx.send(true).unwrap();
        });

        let events = oloop.run().await;
        handle.await.unwrap();

        // Should have at least the shutdown event
        assert!(events.iter().any(|e| matches!(e, LoopEvent::ShuttingDown)));
    }

    #[test]
    fn health_check_empty_manager() {
        let config = make_app_config();
        let (_tx, rx) = watch::channel(false);
        let mut oloop = OrchestratorLoop::from_config(&config, rx);
        let events = oloop.tick_health_checks();
        // No clients = no events
        assert!(events.is_empty());
    }

    #[test]
    fn launch_tick_empty_coordinator() {
        let config = make_app_config();
        let (_tx, rx) = watch::channel(false);
        let mut oloop = OrchestratorLoop::from_config(&config, rx);
        let events = oloop.tick_launch_coordinator();
        // No queued clients = no events
        assert!(events.is_empty());
    }

    #[test]
    fn client_ready_caches_group_and_class_metadata() {
        let config = make_app_config();
        let (_tx, rx) = watch::channel(false);
        let mut oloop = OrchestratorLoop::from_config(&config, rx);

        oloop.client_manager.track_client(7, 4242);
        let account = AccountInfo {
            account_name: "acct".into(),
            character_name: "Cleric42".into(),
            class_name: "Cleric".into(),
            level: 60,
            group_id: 2,
            server_name: "Frostreaver".into(),
        };
        {
            let session = oloop.client_manager.get_mut(7).expect("session");
            session.bound_toon = Some(account.clone());
            session.character_name = Some("Cleric42".into());
        }

        oloop
            .launch_coordinator
            .seed_ready_login_for_test(7, account);

        let events = oloop.tick_launch_coordinator();
        assert!(
            events
                .iter()
                .any(|event| matches!(event, LoopEvent::ClientRegistered { pid: 4242 }))
        );
        assert_eq!(oloop.orchestrator.client_group(4242), Some(2));
        assert_eq!(oloop.orchestrator.client_class_name(4242), Some("Cleric"));
    }

    #[test]
    fn loop_event_debug_format() {
        let e = LoopEvent::ClientCamped { pid: 123 };
        let debug = format!("{e:?}");
        assert!(debug.contains("ClientCamped"));
        assert!(debug.contains("123"));
    }

    #[test]
    fn loop_event_variants() {
        let events = vec![
            LoopEvent::ClientCamped { pid: 1 },
            LoopEvent::ClientRequeued { pid: 2 },
            LoopEvent::ClientRegistered { pid: 3 },
            LoopEvent::PeerDiscovered {
                node_name: "peer-box".to_string(),
                sessions: 2,
            },
            LoopEvent::PeerExpired {
                node_name: "peer-box".to_string(),
            },
            LoopEvent::HealthCheckDone {
                healthy: 5,
                unhealthy: 1,
            },
            LoopEvent::ShuttingDown,
        ];
        for e in &events {
            let _ = format!("{e:?}");
        }
        assert_eq!(events.len(), 7);
    }

    #[test]
    fn config_defaults_are_sensible() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.health_check_interval_ms, 5000);
        assert_eq!(config.launch_tick_interval_ms, 1000);
        assert_eq!(config.orchestrator_tick_interval_ms, 250);
        assert_eq!(config.state_poll_interval_ms, 100);
    }
}
