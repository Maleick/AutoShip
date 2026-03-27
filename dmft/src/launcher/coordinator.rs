use crate::config::{LaunchConfig, RetryConfig, ServerConfig};
use crate::launcher::login_sm::{LoginAction, LoginEvent, LoginStateMachine};
use crate::launcher::spawner;
use dmft_common::login::{AccountInfo, LoginError};
use dmft_common::types::ClientId;
use rand::Rng;
use std::collections::VecDeque;
use std::path::Path;
use std::time::{Duration, Instant};

pub struct LaunchCoordinator {
    config: LaunchConfig,
    retry_config: RetryConfig,
    server_config: ServerConfig,
    launch_queue: VecDeque<(ClientId, AccountInfo)>,
    active_logins: Vec<LoginStateMachine>,
    failure_window: VecDeque<(Instant, ClientId)>,
    paused: bool,
    last_launch: Option<Instant>,
    next_stagger: Duration,
}

#[derive(Debug)]
pub enum CoordinatorEvent {
    ClientLaunched { client_id: ClientId, pid: u32 },
    ClientReady { client_id: ClientId },
    ClientFailed { client_id: ClientId, error: LoginError },
    AllPaused { reason: String },
    AllReady,
}

impl LaunchCoordinator {
    pub fn new(config: LaunchConfig, retry: RetryConfig, server: ServerConfig) -> Self {
        let next_stagger = compute_stagger_between(config.stagger_min_secs, config.stagger_max_secs);
        Self {
            config,
            retry_config: retry,
            server_config: server,
            launch_queue: VecDeque::new(),
            active_logins: Vec::new(),
            failure_window: VecDeque::new(),
            paused: false,
            last_launch: None,
            next_stagger,
        }
    }

    pub fn enqueue(&mut self, client_id: ClientId, account: AccountInfo) {
        self.launch_queue.push_back((client_id, account));
    }

    pub fn tick(&mut self) -> Vec<CoordinatorEvent> {
        let mut events = Vec::new();

        // 1. If paused, return empty
        if self.paused {
            return events;
        }

        // 2-3. Launch next client if allowed
        if self.should_launch_next()
            && let Some((client_id, account)) = self.launch_queue.pop_front()
        {
            let eq_path = Path::new(&self.config.eq_path);
            match spawner::spawn_eq_client(
                eq_path,
                &account.account_name,
                &self.server_config.name,
                &self.config.launch_args,
            ) {
                Ok(spawned) => {
                    let mut sm = LoginStateMachine::new(client_id, account);
                    sm.advance(LoginEvent::ProcessStarted { pid: spawned.pid });
                    self.active_logins.push(sm);
                    self.last_launch = Some(Instant::now());
                    self.next_stagger = compute_stagger_between(
                        self.config.stagger_min_secs,
                        self.config.stagger_max_secs,
                    );
                    events.push(CoordinatorEvent::ClientLaunched {
                        client_id,
                        pid: spawned.pid,
                    });
                }
                Err(err) => {
                    tracing::error!(client_id, %err, "Failed to spawn EQ client");
                    let error = LoginError::Timeout {
                        phase: format!("spawn failed: {err}"),
                    };
                    if self.detect_mass_failure(client_id) {
                        self.paused = true;
                        events.push(CoordinatorEvent::AllPaused {
                            reason: "Mass failure threshold reached during spawn".to_string(),
                        });
                    }
                    events.push(CoordinatorEvent::ClientFailed {
                        client_id,
                        error,
                    });
                }
            }
        }

        // 4-5. Tick all active state machines and collect actions
        let mut retry_queue: Vec<(ClientId, AccountInfo)> = Vec::new();
        let mut i = 0;
        while i < self.active_logins.len() {
            if let Some(action) = self.active_logins[i].tick() {
                let client_id = self.active_logins[i].client_id;
                match action {
                    LoginAction::Retry { after: _ } => {
                        let sm = self.active_logins.remove(i);
                        retry_queue.push((sm.client_id, sm.account_info));
                        // Don't increment i since we removed the element
                        continue;
                    }
                    LoginAction::Abort { reason } => {
                        if self.detect_mass_failure(client_id) {
                            self.paused = true;
                            events.push(CoordinatorEvent::AllPaused {
                                reason: "Mass failure threshold reached".to_string(),
                            });
                        }
                        events.push(CoordinatorEvent::ClientFailed {
                            client_id,
                            error: reason,
                        });
                    }
                    LoginAction::PauseAll => {
                        self.paused = true;
                        events.push(CoordinatorEvent::AllPaused {
                            reason: "Login state machine requested pause".to_string(),
                        });
                    }
                    _ => {}
                }
            }
            i += 1;
        }

        // Re-enqueue retries at the front
        for (client_id, account) in retry_queue.into_iter().rev() {
            self.launch_queue.push_front((client_id, account));
        }

        // 6. Mass failure window check is done inline above

        // 7. If all active logins are terminal and queue is empty, emit AllReady
        if self.launch_queue.is_empty()
            && !self.active_logins.is_empty()
            && self.active_logins.iter().all(|sm| sm.is_terminal())
        {
            // Only emit AllReady if all finished successfully (Ready state)
            let all_ready = self.active_logins.iter().all(|sm| {
                matches!(sm.phase, dmft_common::login::LoginPhase::Ready)
            });
            if all_ready {
                events.push(CoordinatorEvent::AllReady);
            }
        }

        events
    }

    pub fn report_login_event(&mut self, client_id: ClientId, event: LoginEvent) {
        if let Some(sm) = self.active_logins.iter_mut().find(|sm| sm.client_id == client_id) {
            let action = sm.advance(event);
            match action {
                LoginAction::Retry { after: _ } => {
                    // Find and remove this SM, re-enqueue
                    if let Some(idx) = self.active_logins.iter().position(|s| s.client_id == client_id) {
                        let sm = self.active_logins.remove(idx);
                        self.launch_queue.push_front((sm.client_id, sm.account_info));
                    }
                }
                LoginAction::Abort { reason } => {
                    if self.detect_mass_failure(client_id) {
                        self.paused = true;
                    }
                    tracing::warn!(client_id, ?reason, "Client login aborted");
                }
                LoginAction::PauseAll => {
                    self.paused = true;
                    tracing::warn!(client_id, "Pause-all requested by login state machine");
                }
                _ => {}
            }
        } else {
            tracing::warn!(client_id, "report_login_event for unknown client");
        }
    }

    pub fn resume(&mut self) {
        self.paused = false;
        tracing::info!("Launch coordinator resumed");
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn pending_count(&self) -> usize {
        self.launch_queue.len()
    }

    pub fn active_count(&self) -> usize {
        self.active_logins.iter().filter(|sm| !sm.is_terminal()).count()
    }

    fn should_launch_next(&self) -> bool {
        if self.launch_queue.is_empty() {
            return false;
        }

        // Check max concurrent (non-terminal active logins)
        let active = self.active_count();
        if active >= self.config.max_concurrent_launches {
            return false;
        }

        // Check stagger timing
        if let Some(last) = self.last_launch && last.elapsed() < self.next_stagger {
            return false;
        }

        true
    }

    fn detect_mass_failure(&mut self, client_id: ClientId) -> bool {
        let now = Instant::now();
        let window = Duration::from_secs(self.retry_config.mass_failure_window_secs);

        // Add this failure
        self.failure_window.push_back((now, client_id));

        // Prune old entries outside the window
        while let Some(&(ts, _)) = self.failure_window.front() {
            if now.duration_since(ts) > window {
                self.failure_window.pop_front();
            } else {
                break;
            }
        }

        self.failure_window.len() as u32 >= self.retry_config.mass_failure_threshold
    }
}

fn compute_stagger_between(min_secs: u64, max_secs: u64) -> Duration {
    if min_secs >= max_secs {
        return Duration::from_secs(min_secs);
    }
    let secs = rand::thread_rng().gen_range(min_secs..=max_secs);
    Duration::from_secs(secs)
}
