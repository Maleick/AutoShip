//! Top-level orchestrator event loop — wires ClientManager, LaunchCoordinator,
//! and Orchestrator into a single async tick loop with health checks, crash
//! recovery, and graceful shutdown.

use crate::client::healing::ClientHealth;
use crate::client::manager::ClientManager;
use crate::client::session::SlotLifecycle;
use crate::config::{AppConfig, OrchestratorConfig};
use crate::launcher::coordinator::{CoordinatorEvent, LaunchCoordinator};
use crate::orchestrator::Orchestrator;
use std::time::Duration;
use tokio::sync::watch;

/// Events emitted by the orchestrator loop for external consumers (TUI, logging).
#[derive(Debug, Clone)]
pub enum LoopEvent {
    /// A client was detected as unhealthy and sent `/camp desktop`.
    ClientCamped { pid: u32 },
    /// A crashed client was re-enqueued for relaunch.
    ClientRequeued { pid: u32 },
    /// A newly-launched client was registered with the orchestrator.
    ClientRegistered { pid: u32 },
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
    shutdown_rx: watch::Receiver<bool>,
}

impl OrchestratorLoop {
    /// Create a new orchestrator loop.
    ///
    /// `shutdown_rx` receives `true` when the loop should stop.
    pub fn new(
        client_manager: ClientManager,
        launch_coordinator: LaunchCoordinator,
        orchestrator: Orchestrator,
        config: OrchestratorConfig,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            client_manager,
            launch_coordinator,
            orchestrator,
            config,
            shutdown_rx,
        }
    }

    /// Build from an `AppConfig` and a shutdown channel.
    pub fn from_config(app_config: &AppConfig, shutdown_rx: watch::Receiver<bool>) -> Self {
        let client_manager = ClientManager::new(&app_config.process_name);
        let launch_coordinator = LaunchCoordinator::new(
            app_config.launch.clone(),
            app_config.retry.clone(),
            app_config.server.clone(),
        );
        let orchestrator = Orchestrator::new();

        Self::new(
            client_manager,
            launch_coordinator,
            orchestrator,
            app_config.orchestrator.clone(),
            shutdown_rx,
        )
    }

    /// Run the event loop until shutdown is signaled.
    ///
    /// Returns accumulated events from the final tick (or empty on clean shutdown).
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
                }
                _ = launch_interval.tick() => {
                    let events = self.tick_launch_coordinator();
                    for event in &events {
                        tracing::debug!(?event, "orchestrator loop event");
                    }
                }
                _ = orch_interval.tick() => {
                    self.orchestrator.tick();
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

                        // Store the session token for the DLL
                        if let Some(name) = &session.character_name {
                            self.orchestrator.client_names.insert(pid, name.clone());
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
            LoopEvent::HealthCheckDone {
                healthy: 5,
                unhealthy: 1,
            },
            LoopEvent::ShuttingDown,
        ];
        for e in &events {
            let _ = format!("{e:?}");
        }
        assert_eq!(events.len(), 5);
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
