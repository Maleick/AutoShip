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
                    LoginPhase::Ready => LoginAction::None,
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
                Some(LoginAction::Retry {
                    after: RETRY_DELAY,
                })
            }
        } else {
            None
        }
    }

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
                    LoginAction::Retry {
                        after: RETRY_DELAY,
                    }
                }
            }

            // Fatal errors — abort immediately
            LoginError::WrongPassword | LoginError::AccountLocked => {
                self.transition_to(LoginPhase::Failed {
                    reason: error.clone(),
                });
                LoginAction::Abort { reason: error }
            }

            // Character mismatch — abort
            LoginError::CharacterNotFound { .. } => {
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
