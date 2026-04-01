use dmft_common::login::{AccountInfo, LoginError, LoginPhase};
use dmft_common::types::ClientId;
use std::time::{Duration, Instant};

pub struct LoginStateMachine {
    pub client_id: ClientId,
    pub account_info: AccountInfo,
    pub phase: LoginPhase,
    pub attempts: u32,
    pub last_transition: Instant,
    pub phase_timeout: Duration,
}

pub enum LoginEvent {
    ProcessStarted { pid: u32 },
    LoginScreenDetected,
    CredentialsSent,
    ServerSelected,
    CharacterSelected,
    ZoneInComplete,
    PlayerDataConfirmed { name: String, class_name: String },
    ErrorDetected { error: LoginError },
    DllReported { phase: LoginPhase },
}

pub enum LoginAction {
    None,
    SendCredentials,
    SelectServer { name: String },
    SelectCharacter { name: String },
    WaitForZone,
    BeginPostLogin,
    Retry { after: Duration },
    Abort { reason: LoginError },
    PauseAll,
}

const MAX_ATTEMPTS: u32 = 3;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const RETRY_DELAY: Duration = Duration::from_secs(5);

impl LoginStateMachine {
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
                if name == self.account_info.character_name
                    && class_name == self.account_info.class_name
                {
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

    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self.phase, LoginPhase::Ready | LoginPhase::Failed { .. })
    }

    fn transition_to(&mut self, phase: LoginPhase) {
        self.phase = phase;
        self.last_transition = Instant::now();
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
}
