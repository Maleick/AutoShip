use std::time::{Duration, Instant};
use textquest_common::{
    login::{AccountInfo, LoginError, LoginPhase},
    types::ClientId,
};

/// State machine driving a single client through the EQ login flow.
pub struct LoginStateMachine {
    /// Unique identifier for this client session.
    pub client_id: ClientId,
    /// Account credentials and target character/server.
    pub account_info: AccountInfo,
    /// Current phase of the login process.
    pub phase: LoginPhase,
    /// Number of retry attempts so far.
    pub attempts: u32,
    /// Timestamp of the last phase transition (for timeout detection).
    pub last_transition: Instant,
    /// Maximum time allowed in the current phase before timeout.
    pub phase_timeout: Duration,
}

/// Events that drive login state transitions.
pub enum LoginEvent {
    /// EQ process has been spawned with the given PID.
    ProcessStarted {
        /// OS process ID.
        pid: u32,
    },
    /// Login screen UI is visible and ready for input.
    LoginScreenDetected,
    /// Account/password have been entered.
    CredentialsSent,
    /// Server has been chosen from the server list.
    ServerSelected,
    /// Character has been selected from the character list.
    CharacterSelected,
    /// Client has finished zoning into the world.
    ZoneInComplete,
    /// Player data confirmed from in-game memory.
    PlayerDataConfirmed {
        /// Character name read from memory.
        name: String,
        /// Class name read from memory.
        class_name: String,
    },
    /// An error occurred during the login process.
    ErrorDetected {
        /// The specific login error.
        error: LoginError,
    },
    /// The injected DLL reported a phase change.
    DllReported {
        /// Phase reported by the DLL.
        phase: LoginPhase,
    },
}

/// Actions the login coordinator should perform in response to state
/// transitions.
pub enum LoginAction {
    /// No action needed.
    None,
    /// Enter account credentials into the login screen.
    SendCredentials,
    /// Select the target server by name.
    SelectServer {
        /// Server name to select.
        name: String,
    },
    /// Select the target character by name.
    SelectCharacter {
        /// Character name to select.
        name: String,
    },
    /// Wait for the client to finish zoning in.
    WaitForZone,
    /// Start the post-login sequence (group invites, buffs, etc.).
    BeginPostLogin,
    /// Retry the current phase after a delay.
    Retry {
        /// How long to wait before retrying.
        after: Duration,
    },
    /// Abort the login attempt with an error.
    Abort {
        /// Reason for the abort.
        reason: LoginError,
    },
    /// Pause all login operations (mass failure detected).
    PauseAll,
}

const MAX_ATTEMPTS: u32 = 3;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const RETRY_DELAY: Duration = Duration::from_secs(5);

impl LoginStateMachine {
    /// Creates a new login state machine for the given client and account.
    #[must_use]
    pub fn new(client_id: ClientId, account_info: AccountInfo) -> Self {
        Self {
            client_id,
            account_info,
            phase: LoginPhase::NotStarted,
            attempts: 0,
            last_transition: Instant::now(),
            phase_timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Processes an event and returns the action the coordinator should take.
    pub fn advance(&mut self, event: LoginEvent) -> LoginAction {
        match event {
            LoginEvent::ProcessStarted { pid: _ } => {
                self.transition_to(LoginPhase::ProcessLaunching);
                LoginAction::None
            }

            LoginEvent::LoginScreenDetected => {
                self.transition_to(LoginPhase::AtLoginScreen);
                LoginAction::SendCredentials
            }

            LoginEvent::CredentialsSent => {
                self.transition_to(LoginPhase::EnteringCredentials);
                LoginAction::SelectServer {
                    name: self.account_info.server_name.clone(),
                }
            }

            LoginEvent::ServerSelected => {
                self.transition_to(LoginPhase::ServerSelecting);
                LoginAction::SelectCharacter {
                    name: self.account_info.character_name.clone(),
                }
            }

            LoginEvent::CharacterSelected => {
                self.transition_to(LoginPhase::CharacterSelecting);
                LoginAction::WaitForZone
            }

            LoginEvent::ZoneInComplete => {
                self.transition_to(LoginPhase::InWorld);
                LoginAction::BeginPostLogin
            }

            LoginEvent::PlayerDataConfirmed { name, class_name } => {
                let class_matches =
                    self.account_class_is_unset() || class_name == self.account_info.class_name;
                if name == self.account_info.character_name && class_matches {
                    self.transition_to(LoginPhase::Ready);
                    LoginAction::None
                } else {
                    let error = LoginError::CharacterNotFound {
                        expected: self.account_info.character_name.clone(),
                        found: name,
                    };
                    self.transition_to(LoginPhase::Failed {
                        reason: error.clone(),
                    });
                    LoginAction::Abort { reason: error }
                }
            }

            LoginEvent::ErrorDetected { error } => self.handle_error(error),

            LoginEvent::DllReported { phase } => {
                self.transition_to(phase.clone());
                match phase {
                    LoginPhase::InWorld => LoginAction::BeginPostLogin,
                    LoginPhase::Failed { reason } => LoginAction::Abort { reason },
                    _ => LoginAction::None,
                }
            }
        }
    }

    /// Checks for phase timeout and returns a retry or abort action if needed.
    pub fn tick(&mut self) -> Option<LoginAction> {
        // No timeout checks for terminal or not-yet-started states
        if self.is_terminal() {
            return None;
        }
        if matches!(self.phase, LoginPhase::NotStarted) {
            return None;
        }

        if self.last_transition.elapsed() >= self.phase_timeout {
            let phase_name = format!("{:?}", self.phase);
            self.attempts += 1;

            if self.attempts >= MAX_ATTEMPTS {
                let error = LoginError::Timeout { phase: phase_name };
                self.transition_to(LoginPhase::Failed {
                    reason: error.clone(),
                });
                Some(LoginAction::Abort { reason: error })
            } else {
                // Reset the timer for the retry
                self.last_transition = Instant::now();
                Some(LoginAction::Retry { after: RETRY_DELAY })
            }
        } else {
            None
        }
    }

    /// Returns `true` if the login is in a terminal state (Ready, Failed, or
    /// ProcessExiting).
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.phase,
            LoginPhase::Ready | LoginPhase::Failed { .. } | LoginPhase::ProcessExiting
        )
    }

    fn transition_to(&mut self, phase: LoginPhase) {
        self.phase = phase;
        self.last_transition = Instant::now();
    }

    fn account_class_is_unset(&self) -> bool {
        self.account_info.class_name.eq_ignore_ascii_case("UNK")
    }

    fn handle_error(&mut self, error: LoginError) -> LoginAction {
        match &error {
            // Retryable errors
            LoginError::ServerFull | LoginError::ServerDown => {
                self.attempts += 1;
                if self.attempts >= MAX_ATTEMPTS {
                    self.transition_to(LoginPhase::Failed {
                        reason: error.clone(),
                    });
                    LoginAction::Abort { reason: error }
                } else {
                    LoginAction::Retry { after: RETRY_DELAY }
                }
            }

            // Fatal errors — abort immediately
            LoginError::WrongPassword
            | LoginError::AccountLocked
            | LoginError::CharacterAlreadyLoggedIn
            | LoginError::OfflineTrader
            | LoginError::CharacterNotFound { .. } => {
                self.transition_to(LoginPhase::Failed {
                    reason: error.clone(),
                });
                LoginAction::Abort { reason: error }
            }

            // Timeout — let tick() handle retry logic, but record the error
            LoginError::Timeout { .. } => {
                self.transition_to(LoginPhase::Failed {
                    reason: error.clone(),
                });
                LoginAction::Abort { reason: error }
            }

            // Mass failure — pause everything
            LoginError::MassFailure => {
                self.transition_to(LoginPhase::Failed {
                    reason: error.clone(),
                });
                LoginAction::PauseAll
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_account() -> AccountInfo {
        AccountInfo {
            account_name: "test_acct".to_string(),
            character_name: "Frostreaver".to_string(),
            class_name: "Warrior".to_string(),
            level: 60,
            group_id: 1,
            server_name: "TestServer".to_string(),
        }
    }

    fn new_sm() -> LoginStateMachine {
        LoginStateMachine::new(1, test_account())
    }

    #[test]
    fn initial_state_is_not_started() {
        let sm = new_sm();
        assert!(matches!(sm.phase, LoginPhase::NotStarted));
        assert_eq!(sm.attempts, 0);
        assert!(!sm.is_terminal());
    }

    #[test]
    fn process_started_transitions_to_launching() {
        let mut sm = new_sm();
        let action = sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
        assert!(matches!(sm.phase, LoginPhase::ProcessLaunching));
        assert!(matches!(action, LoginAction::None));
    }

    #[test]
    fn login_screen_detected_transitions_and_sends_credentials() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
        let action = sm.advance(LoginEvent::LoginScreenDetected);
        assert!(matches!(sm.phase, LoginPhase::AtLoginScreen));
        assert!(matches!(action, LoginAction::SendCredentials));
    }

    #[test]
    fn full_happy_path_reaches_ready() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
        sm.advance(LoginEvent::LoginScreenDetected);
        sm.advance(LoginEvent::CredentialsSent);
        assert!(matches!(sm.phase, LoginPhase::EnteringCredentials));

        sm.advance(LoginEvent::ServerSelected);
        assert!(matches!(sm.phase, LoginPhase::ServerSelecting));

        sm.advance(LoginEvent::CharacterSelected);
        assert!(matches!(sm.phase, LoginPhase::CharacterSelecting));

        let action = sm.advance(LoginEvent::ZoneInComplete);
        assert!(matches!(sm.phase, LoginPhase::InWorld));
        assert!(matches!(action, LoginAction::BeginPostLogin));

        let action = sm.advance(LoginEvent::PlayerDataConfirmed {
            name: "Frostreaver".to_string(),
            class_name: "Warrior".to_string(),
        });
        assert!(matches!(sm.phase, LoginPhase::Ready));
        assert!(matches!(action, LoginAction::None));
        assert!(sm.is_terminal());
    }

    #[test]
    fn wrong_password_aborts_immediately() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
        let action = sm.advance(LoginEvent::ErrorDetected {
            error: LoginError::WrongPassword,
        });
        assert!(matches!(sm.phase, LoginPhase::Failed { .. }));
        assert!(matches!(action, LoginAction::Abort { .. }));
        assert!(sm.is_terminal());
    }

    #[test]
    fn account_locked_aborts_immediately() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
        let action = sm.advance(LoginEvent::ErrorDetected {
            error: LoginError::AccountLocked,
        });
        assert!(matches!(sm.phase, LoginPhase::Failed { .. }));
        assert!(matches!(action, LoginAction::Abort { .. }));
    }

    #[test]
    fn server_full_retries_then_aborts() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });

        // First two retries should produce Retry actions
        let action = sm.advance(LoginEvent::ErrorDetected {
            error: LoginError::ServerFull,
        });
        assert!(matches!(action, LoginAction::Retry { .. }));
        assert_eq!(sm.attempts, 1);

        let action = sm.advance(LoginEvent::ErrorDetected {
            error: LoginError::ServerFull,
        });
        assert!(matches!(action, LoginAction::Retry { .. }));
        assert_eq!(sm.attempts, 2);

        // Third attempt (MAX_ATTEMPTS=3) should abort
        let action = sm.advance(LoginEvent::ErrorDetected {
            error: LoginError::ServerFull,
        });
        assert!(matches!(action, LoginAction::Abort { .. }));
        assert!(sm.is_terminal());
    }

    #[test]
    fn mass_failure_triggers_pause_all() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
        let action = sm.advance(LoginEvent::ErrorDetected {
            error: LoginError::MassFailure,
        });
        assert!(matches!(action, LoginAction::PauseAll));
        assert!(sm.is_terminal());
    }

    #[test]
    fn player_data_mismatch_aborts() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
        sm.advance(LoginEvent::LoginScreenDetected);
        sm.advance(LoginEvent::CredentialsSent);
        sm.advance(LoginEvent::ServerSelected);
        sm.advance(LoginEvent::CharacterSelected);
        sm.advance(LoginEvent::ZoneInComplete);

        let action = sm.advance(LoginEvent::PlayerDataConfirmed {
            name: "WrongCharacter".to_string(),
            class_name: "Warrior".to_string(),
        });
        assert!(matches!(sm.phase, LoginPhase::Failed { .. }));
        assert!(matches!(action, LoginAction::Abort { .. }));
    }

    #[test]
    fn tick_returns_none_when_not_started() {
        let mut sm = new_sm();
        assert!(sm.tick().is_none());
    }

    #[test]
    fn tick_returns_none_when_terminal() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ErrorDetected {
            error: LoginError::WrongPassword,
        });
        assert!(sm.is_terminal());
        assert!(sm.tick().is_none());
    }

    #[test]
    fn dll_reported_in_world_triggers_post_login() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
        let action = sm.advance(LoginEvent::DllReported {
            phase: LoginPhase::InWorld,
        });
        assert!(matches!(sm.phase, LoginPhase::InWorld));
        assert!(matches!(action, LoginAction::BeginPostLogin));
    }

    #[test]
    fn credentials_sent_produces_select_server_with_correct_name() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
        sm.advance(LoginEvent::LoginScreenDetected);
        let action = sm.advance(LoginEvent::CredentialsSent);
        match action {
            LoginAction::SelectServer { name } => {
                assert_eq!(name, "TestServer");
            }
            other => panic!(
                "expected SelectServer, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn server_selected_produces_select_character_with_correct_name() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
        sm.advance(LoginEvent::LoginScreenDetected);
        sm.advance(LoginEvent::CredentialsSent);
        let action = sm.advance(LoginEvent::ServerSelected);
        match action {
            LoginAction::SelectCharacter { name } => {
                assert_eq!(name, "Frostreaver");
            }
            other => panic!(
                "expected SelectCharacter, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn character_selected_produces_wait_for_zone() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
        sm.advance(LoginEvent::LoginScreenDetected);
        sm.advance(LoginEvent::CredentialsSent);
        sm.advance(LoginEvent::ServerSelected);
        let action = sm.advance(LoginEvent::CharacterSelected);
        assert!(matches!(action, LoginAction::WaitForZone));
    }

    #[test]
    fn server_down_retries_then_aborts() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });

        for i in 0..2 {
            let action = sm.advance(LoginEvent::ErrorDetected {
                error: LoginError::ServerDown,
            });
            assert!(matches!(action, LoginAction::Retry { .. }), "attempt {i}");
        }

        let action = sm.advance(LoginEvent::ErrorDetected {
            error: LoginError::ServerDown,
        });
        assert!(matches!(action, LoginAction::Abort { .. }));
    }

    #[test]
    fn character_not_found_error_aborts() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
        let action = sm.advance(LoginEvent::ErrorDetected {
            error: LoginError::CharacterNotFound {
                expected: "Frostreaver".into(),
                found: "WrongChar".into(),
            },
        });
        assert!(matches!(action, LoginAction::Abort { .. }));
        assert!(sm.is_terminal());
    }

    #[test]
    fn timeout_error_aborts() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
        let action = sm.advance(LoginEvent::ErrorDetected {
            error: LoginError::Timeout {
                phase: "test".into(),
            },
        });
        assert!(matches!(action, LoginAction::Abort { .. }));
        assert!(sm.is_terminal());
    }

    #[test]
    fn dll_reported_ready_is_terminal() {
        let mut sm = new_sm();
        let action = sm.advance(LoginEvent::DllReported {
            phase: LoginPhase::Ready,
        });
        assert!(matches!(action, LoginAction::None));
        assert!(sm.is_terminal());
    }

    #[test]
    fn dll_reported_failed_aborts() {
        let mut sm = new_sm();
        let action = sm.advance(LoginEvent::DllReported {
            phase: LoginPhase::Failed {
                reason: LoginError::WrongPassword,
            },
        });
        assert!(matches!(action, LoginAction::Abort { .. }));
        assert!(sm.is_terminal());
    }

    #[test]
    fn dll_reported_intermediate_phase_is_none() {
        let mut sm = new_sm();
        let action = sm.advance(LoginEvent::DllReported {
            phase: LoginPhase::AtLoginScreen,
        });
        assert!(matches!(action, LoginAction::None));
        assert!(!sm.is_terminal());
    }

    #[test]
    fn class_mismatch_in_player_data_aborts() {
        let mut sm = new_sm();
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
        sm.advance(LoginEvent::ZoneInComplete);
        let action = sm.advance(LoginEvent::PlayerDataConfirmed {
            name: "Frostreaver".to_string(),
            class_name: "Cleric".to_string(), // Wrong class
        });
        assert!(matches!(action, LoginAction::Abort { .. }));
    }

    #[test]
    fn unset_account_class_accepts_confirmed_player_data() {
        let mut account = test_account();
        account.class_name = "UNK".to_string();

        let mut sm = LoginStateMachine::new(1, account);
        sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
        sm.advance(LoginEvent::ZoneInComplete);
        let action = sm.advance(LoginEvent::PlayerDataConfirmed {
            name: "Frostreaver".to_string(),
            class_name: "Cleric".to_string(),
        });

        assert!(matches!(action, LoginAction::None));
        assert!(matches!(sm.phase, LoginPhase::Ready));
    }

    #[test]
    fn client_id_preserved() {
        let sm = LoginStateMachine::new(42, test_account());
        assert_eq!(sm.client_id, 42);
    }
}
