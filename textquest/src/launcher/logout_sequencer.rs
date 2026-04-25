use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Context;
use textquest_common::{
    ipc::{Command, Response},
    login::{LoginError, LoginPhase},
    types::{ClientId, GameState},
};
use thiserror::Error;

use crate::ipc::pipe::CommandPipe;

const DEFAULT_EXIT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const DEFAULT_COMMAND_ATTEMPTS: u32 = 3;
const GROUP_LEAVE_COMMAND: &str = "/disband";
const QUIT_COMMAND: &str = "/quit";

/// Sends logout commands to a client.
pub trait LogoutCommandSender: Send {
    /// Send a command and return the IPC response.
    ///
    /// # Errors
    ///
    /// Returns an error when the IPC layer cannot deliver the command.
    fn send(&mut self, client_id: ClientId, command: &Command) -> anyhow::Result<Response>;
}

/// Observes and terminates the EQ process for a client.
pub trait LogoutProcessControl: Send {
    /// Returns whether the process is still running.
    ///
    /// # Errors
    ///
    /// Returns an error when process state cannot be queried.
    fn is_running(&mut self, client_id: ClientId) -> anyhow::Result<bool>;

    /// Forcefully terminate the process.
    ///
    /// # Errors
    ///
    /// Returns an error when process termination fails.
    fn force_kill(&mut self, client_id: ClientId) -> anyhow::Result<()>;
}

/// IPC-backed logout command sender.
#[derive(Debug, Clone, Copy)]
pub struct CommandPipeLogoutSender {
    session_id: u64,
}

impl CommandPipeLogoutSender {
    /// Create a command sender for the current orchestration session.
    #[must_use]
    pub const fn new(session_id: u64) -> Self {
        Self { session_id }
    }
}

impl LogoutCommandSender for CommandPipeLogoutSender {
    fn send(&mut self, client_id: ClientId, command: &Command) -> anyhow::Result<Response> {
        let pipe = CommandPipe::connect(client_id, self.session_id)?;
        pipe.send(command)
    }
}

/// OS-backed process control used by the production logout path.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemProcessControl;

impl LogoutProcessControl for SystemProcessControl {
    fn is_running(&mut self, client_id: ClientId) -> anyhow::Result<bool> {
        #[cfg(windows)]
        {
            use windows::Win32::System::Threading::{
                OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            };

            Ok(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, client_id).is_ok() })
        }

        #[cfg(not(windows))]
        {
            let output = std::process::Command::new("kill")
                .args(["-0", &client_id.to_string()])
                .output()
                .context("failed to query process with kill -0")?;
            Ok(output.status.success())
        }
    }

    fn force_kill(&mut self, client_id: ClientId) -> anyhow::Result<()> {
        #[cfg(windows)]
        {
            use windows::Win32::System::Threading::{
                OpenProcess, TerminateProcess, PROCESS_TERMINATE,
            };

            unsafe {
                let handle = OpenProcess(PROCESS_TERMINATE, false, client_id)
                    .context("failed to open process for termination")?;
                TerminateProcess(handle, 0).context("failed to terminate process")?;
            }
            Ok(())
        }

        #[cfg(not(windows))]
        {
            let output = std::process::Command::new("kill")
                .args(["-KILL", &client_id.to_string()])
                .output()
                .context("failed to send SIGKILL")?;
            if output.status.success() {
                Ok(())
            } else {
                anyhow::bail!("kill -KILL exited with {}", output.status);
            }
        }
    }
}

/// Result of the process-exit portion of logout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoutOutcome {
    /// The EQ process exited before the timeout.
    Exited,
    /// The timeout expired and the sequencer force-killed the process.
    ForceKilled,
}

/// Errors raised while coordinating logout.
#[derive(Debug, Error)]
pub enum LogoutError {
    /// An IPC command was rejected after all retry attempts.
    #[error("logout command `{command}` failed for client {client_id} after {attempts} attempts: {message}")]
    CommandFailed {
        client_id: ClientId,
        command: String,
        attempts: u32,
        message: String,
    },
    /// An IPC command could not be delivered after all retry attempts.
    #[error("logout command `{command}` could not be delivered to client {client_id} after {attempts} attempts")]
    CommandDelivery {
        client_id: ClientId,
        command: String,
        attempts: u32,
        #[source]
        source: anyhow::Error,
    },
    /// Process status could not be queried.
    #[error("failed to query logout process state for client {client_id}")]
    ProcessStatus {
        client_id: ClientId,
        #[source]
        source: anyhow::Error,
    },
    /// Force-kill fallback failed.
    #[error("failed to force-kill logout process for client {client_id}")]
    ForceKill {
        client_id: ClientId,
        #[source]
        source: anyhow::Error,
    },
}

/// Tracks logout phases using the shared launcher login phase vocabulary.
#[derive(Debug, Clone)]
pub struct LogoutStateMachine {
    phase: LoginPhase,
    last_transition: Instant,
    exit_timeout: Duration,
}

impl Default for LogoutStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl LogoutStateMachine {
    /// Create a logout state machine starting from an in-world client.
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: LoginPhase::InWorld,
            last_transition: Instant::now(),
            exit_timeout: DEFAULT_EXIT_TIMEOUT,
        }
    }

    /// Current logout phase.
    #[must_use]
    pub fn phase(&self) -> &LoginPhase {
        &self.phase
    }

    /// Exit timeout for the `/quit` process-exit wait.
    #[must_use]
    pub const fn exit_timeout(&self) -> Duration {
        self.exit_timeout
    }

    /// Override the process-exit timeout.
    pub fn set_exit_timeout(&mut self, timeout: Duration) {
        self.exit_timeout = timeout;
    }

    /// Elapsed time since the last phase transition.
    #[must_use]
    pub fn elapsed_in_phase(&self) -> Duration {
        self.last_transition.elapsed()
    }

    fn begin_group_leave(&mut self) {
        self.transition_to(LoginPhase::LeavingGroup);
    }

    fn group_leave_acknowledged(&mut self) {
        self.transition_to(LoginPhase::QuittingGame);
    }

    fn quit_command_sent(&mut self) {
        self.transition_to(LoginPhase::ProcessExiting);
    }

    fn process_exited(&mut self) {
        self.transition_to(LoginPhase::Exited);
    }

    fn fail_timeout(&mut self) {
        self.transition_to(LoginPhase::Failed {
            reason: LoginError::Timeout {
                phase: "ProcessExiting".to_string(),
            },
        });
    }

    fn transition_to(&mut self, phase: LoginPhase) {
        self.phase = phase;
        self.last_transition = Instant::now();
    }
}

/// Coordinates graceful logout for one EQ client.
pub struct LogoutSequencer {
    client_id: ClientId,
    state_machine: LogoutStateMachine,
    game_state: Arc<GameState>,
    command_sender: Box<dyn LogoutCommandSender>,
    process_control: Box<dyn LogoutProcessControl>,
    command_attempts: u32,
    poll_interval: Duration,
}

impl LogoutSequencer {
    /// Create a production logout sequencer backed by named-pipe IPC and OS
    /// process control.
    #[must_use]
    pub fn new(client_id: ClientId, session_id: u64, game_state: Arc<GameState>) -> Self {
        Self::with_adapters(
            client_id,
            game_state,
            CommandPipeLogoutSender::new(session_id),
            SystemProcessControl,
        )
    }

    /// Create a sequencer with explicit adapters.
    #[must_use]
    pub fn with_adapters<C, P>(
        client_id: ClientId,
        game_state: Arc<GameState>,
        command_sender: C,
        process_control: P,
    ) -> Self
    where
        C: LogoutCommandSender + 'static,
        P: LogoutProcessControl + 'static,
    {
        Self {
            client_id,
            state_machine: LogoutStateMachine::new(),
            game_state,
            command_sender: Box::new(command_sender),
            process_control: Box::new(process_control),
            command_attempts: DEFAULT_COMMAND_ATTEMPTS,
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }

    /// Run the full logout sequence.
    ///
    /// # Errors
    ///
    /// Returns an error when command delivery, process polling, or force-kill
    /// fails.
    pub async fn logout(&mut self) -> Result<(), LogoutError> {
        self.logout_with_outcome().await.map(|_| ())
    }

    /// Run logout and report whether the process exited naturally or was
    /// force-killed after timeout.
    ///
    /// # Errors
    ///
    /// Returns an error when command delivery, process polling, or force-kill
    /// fails.
    pub async fn logout_with_outcome(&mut self) -> Result<LogoutOutcome, LogoutError> {
        self.warn_on_state_mismatch();

        self.state_machine.begin_group_leave();
        self.send_slash_command(GROUP_LEAVE_COMMAND)?;
        self.state_machine.group_leave_acknowledged();

        self.send_slash_command(QUIT_COMMAND)?;
        self.state_machine.quit_command_sent();

        let outcome = self.wait_for_process_exit().await?;
        self.state_machine.process_exited();
        Ok(outcome)
    }

    /// Current logout state machine.
    #[must_use]
    pub const fn state_machine(&self) -> &LogoutStateMachine {
        &self.state_machine
    }

    /// Current shared launcher phase for the logout state machine.
    #[must_use]
    pub fn phase(&self) -> &LoginPhase {
        self.state_machine.phase()
    }

    /// Client ID managed by this sequencer.
    #[must_use]
    pub const fn client_id(&self) -> ClientId {
        self.client_id
    }

    /// Override the process-exit timeout.
    pub fn set_exit_timeout(&mut self, timeout: Duration) {
        self.state_machine.set_exit_timeout(timeout);
    }

    /// Override command retry attempts.
    pub fn set_command_attempts(&mut self, attempts: u32) {
        self.command_attempts = attempts.max(1);
    }

    /// Override process polling interval.
    pub fn set_poll_interval(&mut self, interval: Duration) {
        self.poll_interval = interval;
    }

    fn warn_on_state_mismatch(&self) {
        if self.game_state.client_id != self.client_id {
            tracing::warn!(
                sequencer_client_id = self.client_id,
                game_state_client_id = self.game_state.client_id,
                "logout sequencer game state belongs to a different client"
            );
        }
    }

    fn send_slash_command(&mut self, slash_command: &str) -> Result<(), LogoutError> {
        let command = Command::SlashCommand {
            command: slash_command.to_string(),
        };
        self.send_with_retry(slash_command, &command)
    }

    fn send_with_retry(
        &mut self,
        command_label: &str,
        command: &Command,
    ) -> Result<(), LogoutError> {
        let mut last_delivery_error = None;
        let mut last_rejection = None;

        for _ in 0..self.command_attempts {
            match self.command_sender.send(self.client_id, command) {
                Ok(response) => {
                    if let Err(message) = accepted_response(response) {
                        last_rejection = Some(message);
                        continue;
                    }
                    return Ok(());
                }
                Err(error) => {
                    last_delivery_error = Some(error);
                }
            }
        }

        if let Some(message) = last_rejection {
            return Err(LogoutError::CommandFailed {
                client_id: self.client_id,
                command: command_label.to_string(),
                attempts: self.command_attempts,
                message,
            });
        }

        Err(LogoutError::CommandDelivery {
            client_id: self.client_id,
            command: command_label.to_string(),
            attempts: self.command_attempts,
            source: last_delivery_error
                .unwrap_or_else(|| anyhow::anyhow!("command sender made no attempts")),
        })
    }

    async fn wait_for_process_exit(&mut self) -> Result<LogoutOutcome, LogoutError> {
        let started = Instant::now();
        let timeout = self.state_machine.exit_timeout();

        loop {
            let running = self
                .process_control
                .is_running(self.client_id)
                .map_err(|source| LogoutError::ProcessStatus {
                    client_id: self.client_id,
                    source,
                })?;
            if !running {
                return Ok(LogoutOutcome::Exited);
            }

            if started.elapsed() >= timeout {
                self.state_machine.fail_timeout();
                self.process_control.force_kill(self.client_id).map_err(|source| {
                    LogoutError::ForceKill {
                        client_id: self.client_id,
                        source,
                    }
                })?;
                return Ok(LogoutOutcome::ForceKilled);
            }

            tokio::time::sleep(self.poll_interval).await;
        }
    }
}

fn accepted_response(response: Response) -> Result<(), String> {
    match response {
        Response::CommandResult {
            success: true,
            message: _,
        } => Ok(()),
        Response::CommandResult {
            success: false,
            message,
        } => Err(message),
        Response::Error { message } => Err(message),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, sync::Mutex};

    enum FakeSend {
        Ok,
        Err(&'static str),
    }

    struct FakeCommandSender {
        sent: Arc<Mutex<Vec<Command>>>,
        responses: VecDeque<FakeSend>,
    }

    impl FakeCommandSender {
        fn new(sent: Arc<Mutex<Vec<Command>>>, responses: Vec<FakeSend>) -> Self {
            Self {
                sent,
                responses: responses.into(),
            }
        }
    }

    impl LogoutCommandSender for FakeCommandSender {
        fn send(&mut self, _client_id: ClientId, command: &Command) -> anyhow::Result<Response> {
            self.sent
                .lock()
                .expect("sent command log should lock")
                .push(command.clone());
            match self.responses.pop_front().unwrap_or(FakeSend::Ok) {
                FakeSend::Ok => Ok(Response::CommandResult {
                    success: true,
                    message: "ok".to_string(),
                }),
                FakeSend::Err(message) => Err(anyhow::anyhow!("{message}")),
            }
        }
    }

    struct FakeProcessControl {
        running: VecDeque<bool>,
        killed: Arc<Mutex<Vec<ClientId>>>,
    }

    impl FakeProcessControl {
        fn new(killed: Arc<Mutex<Vec<ClientId>>>, running: Vec<bool>) -> Self {
            Self {
                running: running.into(),
                killed,
            }
        }
    }

    impl LogoutProcessControl for FakeProcessControl {
        fn is_running(&mut self, _client_id: ClientId) -> anyhow::Result<bool> {
            Ok(self.running.pop_front().unwrap_or(false))
        }

        fn force_kill(&mut self, client_id: ClientId) -> anyhow::Result<()> {
            self.killed
                .lock()
                .expect("killed process log should lock")
                .push(client_id);
            Ok(())
        }
    }

    fn test_state(client_id: ClientId) -> Arc<GameState> {
        Arc::new(GameState {
            client_id,
            local_player: None,
            target: None,
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: textquest_common::nav::NavStatus::Idle,
            combat_status: textquest_common::combat::CombatStatus::Idle,
            zone_short_name: String::new(),
            zone_long_name: String::new(),
            active_buffs: vec![],
            pet: None,
            actual_version: None,
            is_zone_changing: false,
        })
    }

    fn slash_commands(sent: &Arc<Mutex<Vec<Command>>>) -> Vec<String> {
        sent.lock()
            .expect("sent command log should lock")
            .iter()
            .filter_map(|command| match command {
                Command::SlashCommand { command } => Some(command.clone()),
                _ => None,
            })
            .collect()
    }

    #[tokio::test]
    async fn logout_sends_group_leave_quit_and_observes_exit() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let killed = Arc::new(Mutex::new(Vec::new()));
        let mut sequencer = LogoutSequencer::with_adapters(
            42,
            test_state(42),
            FakeCommandSender::new(sent.clone(), vec![FakeSend::Ok, FakeSend::Ok]),
            FakeProcessControl::new(killed.clone(), vec![false]),
        );

        let outcome = sequencer.logout_with_outcome().await.unwrap();

        assert_eq!(outcome, LogoutOutcome::Exited);
        assert_eq!(slash_commands(&sent), vec!["/disband", "/quit"]);
        assert!(killed.lock().expect("killed process log should lock").is_empty());
        assert!(matches!(sequencer.phase(), LoginPhase::Exited));
    }

    #[tokio::test]
    async fn timeout_force_kills_process() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let killed = Arc::new(Mutex::new(Vec::new()));
        let mut sequencer = LogoutSequencer::with_adapters(
            77,
            test_state(77),
            FakeCommandSender::new(sent, vec![FakeSend::Ok, FakeSend::Ok]),
            FakeProcessControl::new(killed.clone(), vec![true]),
        );
        sequencer.set_exit_timeout(Duration::ZERO);
        sequencer.set_poll_interval(Duration::ZERO);

        let outcome = sequencer.logout_with_outcome().await.unwrap();

        assert_eq!(outcome, LogoutOutcome::ForceKilled);
        assert_eq!(*killed.lock().expect("killed process log should lock"), vec![77]);
        assert!(matches!(sequencer.phase(), LoginPhase::Exited));
    }

    #[tokio::test]
    async fn ipc_command_delivery_retries_before_quit() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let killed = Arc::new(Mutex::new(Vec::new()));
        let mut sequencer = LogoutSequencer::with_adapters(
            91,
            test_state(91),
            FakeCommandSender::new(
                sent.clone(),
                vec![FakeSend::Err("pipe busy"), FakeSend::Ok, FakeSend::Ok],
            ),
            FakeProcessControl::new(killed, vec![false]),
        );

        let outcome = sequencer.logout_with_outcome().await.unwrap();

        assert_eq!(outcome, LogoutOutcome::Exited);
        assert_eq!(
            slash_commands(&sent),
            vec!["/disband", "/disband", "/quit"]
        );
    }
}
