use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Configurable retry policy with exponential backoff for login failures.
///
/// Delays escalate as: `initial_delay * backoff_multiplier^attempt`, capped at `max_delay`.
/// Optional jitter adds up to ±25% randomization to prevent thundering-herd retries
/// across multiple clients.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts before giving up.
    pub max_retries: u32,
    /// Delay before the first retry.
    #[serde(with = "serde_duration_secs")]
    pub initial_delay: Duration,
    /// Upper bound on any single retry delay.
    #[serde(with = "serde_duration_secs")]
    pub max_delay: Duration,
    /// Multiplier applied to the delay after each attempt.
    pub backoff_multiplier: f64,
    /// Whether to add ±25% random jitter to each delay.
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryPolicy {
    /// Returns the next retry delay if retries remain, or `None` if exhausted.
    ///
    /// The returned duration includes jitter (if enabled) and is clamped to `max_delay`.
    pub fn next_delay(&self, state: &RetryState) -> Option<Duration> {
        if state.attempt_count >= self.max_retries {
            return None;
        }

        let base_secs = self.initial_delay.as_secs_f64()
            * self.backoff_multiplier.powi(state.attempt_count as i32);
        let clamped_secs = base_secs.min(self.max_delay.as_secs_f64());

        let final_secs = if self.jitter {
            apply_jitter(clamped_secs)
        } else {
            clamped_secs
        };

        Some(Duration::from_secs_f64(final_secs.max(0.0)))
    }
}

/// Tracks retry progress for a single login attempt sequence.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetryState {
    /// Number of retry attempts made so far.
    pub attempt_count: u32,
    /// The last error that triggered a retry.
    pub last_error: Option<LoginError>,
}

impl RetryState {
    /// Record a failed attempt.
    pub fn record_failure(&mut self, error: LoginError) {
        self.attempt_count += 1;
        self.last_error = Some(error);
    }

    /// Reset state for a fresh retry sequence.
    pub fn reset(&mut self) {
        self.attempt_count = 0;
        self.last_error = None;
    }
}

/// Apply ±25% jitter to a delay value.
fn apply_jitter(secs: f64) -> f64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .subsec_nanos();
    let jitter_factor = (nanos as f64 / u32::MAX as f64) * 0.5 - 0.25;
    secs * (1.0 + jitter_factor)
}

/// Serde helper to serialize `Duration` as fractional seconds (f64) for TOML/JSON.
mod serde_duration_secs {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(dur: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_f64(dur.as_secs_f64())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let secs = f64::deserialize(d)?;
        if secs < 0.0 {
            return Err(serde::de::Error::custom("duration cannot be negative"));
        }
        Ok(Duration::from_secs_f64(secs))
    }
}

/// Current phase of the automated login state machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LoginPhase {
    /// Login has not been initiated.
    NotStarted,
    /// EQ process is being launched.
    ProcessLaunching,
    /// At the EQ login screen (eqmain.dll loaded).
    AtLoginScreen,
    /// Typing account name and password into login fields.
    EnteringCredentials,
    /// Navigating the server selection screen.
    ServerSelecting,
    /// At the character select screen, picking a character.
    CharacterSelecting,
    /// Character is zoning into the game world.
    Zoning,
    /// Character is fully in-world.
    InWorld,
    /// Running post-login setup (buffs, group join, camp positioning).
    PostLoginSetup,
    /// Login complete, client is ready for orchestration.
    Ready,
    /// Login failed with an error.
    Failed {
        /// The error that caused login to fail.
        reason: LoginError,
    },
}

/// Errors that can occur during the automated login process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LoginError {
    /// Authentication failed (bad password).
    WrongPassword,
    /// Account is locked or suspended.
    AccountLocked,
    /// Character is already logged in and needs an explicit kick.
    CharacterAlreadyLoggedIn,
    /// Character is in offline trader mode and cannot log in automatically.
    OfflineTrader,
    /// Target server is down.
    ServerDown,
    /// Target server is at capacity.
    ServerFull,
    /// Expected character was not found at character select.
    CharacterNotFound {
        /// Character name we were looking for.
        expected: String,
        /// What was actually found (may be empty or a different name).
        found: String,
    },
    /// A phase exceeded its timeout.
    Timeout {
        /// Name of the phase that timed out.
        phase: String,
    },
    /// Multiple clients failed simultaneously (circuit breaker triggered).
    MassFailure,
}

/// Per-character account and server metadata for login orchestration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountInfo {
    /// Login account name.
    pub account_name: String,
    /// Character name to select at character select.
    pub character_name: String,
    /// EQ class name (e.g. "Warrior", "Cleric").
    pub class_name: String,
    /// Character level.
    pub level: u8,
    /// Logical group ID for post-login grouping.
    pub group_id: u32,
    /// Target server name (e.g. "Teek", "FV").
    pub server_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_phase_all_variants_constructible() {
        let phases = [
            LoginPhase::NotStarted,
            LoginPhase::ProcessLaunching,
            LoginPhase::AtLoginScreen,
            LoginPhase::EnteringCredentials,
            LoginPhase::ServerSelecting,
            LoginPhase::CharacterSelecting,
            LoginPhase::Zoning,
            LoginPhase::InWorld,
            LoginPhase::PostLoginSetup,
            LoginPhase::Ready,
            LoginPhase::Failed {
                reason: LoginError::WrongPassword,
            },
        ];
        assert_eq!(phases.len(), 11);
    }

    #[test]
    fn login_phase_equality() {
        assert_eq!(LoginPhase::NotStarted, LoginPhase::NotStarted);
        assert_ne!(LoginPhase::NotStarted, LoginPhase::Ready);
        assert_ne!(LoginPhase::InWorld, LoginPhase::Zoning);
    }

    #[test]
    fn login_error_all_variants_constructible() {
        let errors = [
            LoginError::WrongPassword,
            LoginError::AccountLocked,
            LoginError::CharacterAlreadyLoggedIn,
            LoginError::OfflineTrader,
            LoginError::ServerDown,
            LoginError::ServerFull,
            LoginError::CharacterNotFound {
                expected: "Legolas".into(),
                found: "Gimli".into(),
            },
            LoginError::Timeout {
                phase: "ServerSelecting".into(),
            },
            LoginError::MassFailure,
        ];
        assert_eq!(errors.len(), 9);
    }

    #[test]
    fn login_error_character_not_found_stores_names() {
        let err = LoginError::CharacterNotFound {
            expected: "Legolas".into(),
            found: "Gimli".into(),
        };
        if let LoginError::CharacterNotFound { expected, found } = err {
            assert_eq!(expected, "Legolas");
            assert_eq!(found, "Gimli");
        } else {
            panic!("expected CharacterNotFound");
        }
    }

    #[test]
    fn login_error_timeout_stores_phase() {
        let err = LoginError::Timeout {
            phase: "CharacterSelecting".into(),
        };
        if let LoginError::Timeout { phase } = err {
            assert_eq!(phase, "CharacterSelecting");
        } else {
            panic!("expected Timeout");
        }
    }

    #[test]
    fn login_phase_failed_carries_error() {
        let phase = LoginPhase::Failed {
            reason: LoginError::ServerFull,
        };
        if let LoginPhase::Failed { reason } = phase {
            assert_eq!(reason, LoginError::ServerFull);
        } else {
            panic!("expected Failed");
        }
    }

    #[test]
    fn login_phase_serialization_roundtrip() {
        let phases = vec![
            LoginPhase::NotStarted,
            LoginPhase::Ready,
            LoginPhase::Failed {
                reason: LoginError::WrongPassword,
            },
            LoginPhase::Failed {
                reason: LoginError::CharacterNotFound {
                    expected: "A".into(),
                    found: "B".into(),
                },
            },
        ];
        for phase in &phases {
            let json = serde_json::to_string(phase).expect("serialize");
            let restored: LoginPhase = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*phase, restored);
        }
    }

    #[test]
    fn account_info_construction() {
        let info = AccountInfo {
            account_name: "testuser".into(),
            character_name: "Legolas".into(),
            class_name: "Ranger".into(),
            level: 65,
            group_id: 1,
            server_name: "Teek".into(),
        };
        assert_eq!(info.account_name, "testuser");
        assert_eq!(info.character_name, "Legolas");
        assert_eq!(info.class_name, "Ranger");
        assert_eq!(info.level, 65);
        assert_eq!(info.group_id, 1);
        assert_eq!(info.server_name, "Teek");
    }

    #[test]
    fn account_info_serialization_roundtrip() {
        let info = AccountInfo {
            account_name: "user".into(),
            character_name: "Char".into(),
            class_name: "Warrior".into(),
            level: 50,
            group_id: 0,
            server_name: "FV".into(),
        };
        let json = serde_json::to_string(&info).expect("serialize");
        let restored: AccountInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.account_name, info.account_name);
        assert_eq!(restored.level, info.level);
    }

    #[test]
    fn login_error_equality() {
        assert_eq!(LoginError::WrongPassword, LoginError::WrongPassword);
        assert_eq!(LoginError::ServerDown, LoginError::ServerDown);
        assert_eq!(
            LoginError::CharacterAlreadyLoggedIn,
            LoginError::CharacterAlreadyLoggedIn
        );
        assert_eq!(LoginError::OfflineTrader, LoginError::OfflineTrader);
        assert_ne!(LoginError::WrongPassword, LoginError::AccountLocked);
        assert_ne!(LoginError::ServerDown, LoginError::ServerFull);
    }

    #[test]
    fn login_error_serialization_roundtrip() {
        let errors = vec![
            LoginError::WrongPassword,
            LoginError::AccountLocked,
            LoginError::CharacterAlreadyLoggedIn,
            LoginError::OfflineTrader,
            LoginError::ServerDown,
            LoginError::ServerFull,
            LoginError::MassFailure,
            LoginError::CharacterNotFound {
                expected: "A".into(),
                found: "B".into(),
            },
            LoginError::Timeout {
                phase: "test".into(),
            },
        ];
        for error in &errors {
            let json = serde_json::to_string(error).expect("serialize");
            let restored: LoginError = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*error, restored);
        }
    }

    #[test]
    fn login_phase_failed_with_different_errors_not_equal() {
        let p1 = LoginPhase::Failed {
            reason: LoginError::WrongPassword,
        };
        let p2 = LoginPhase::Failed {
            reason: LoginError::AccountLocked,
        };
        assert_ne!(p1, p2);
    }

    #[test]
    fn login_phase_debug_format() {
        let phase = LoginPhase::AtLoginScreen;
        let debug = format!("{:?}", phase);
        assert!(debug.contains("AtLoginScreen"));
    }

    #[test]
    fn login_error_debug_format() {
        let err = LoginError::CharacterNotFound {
            expected: "Foo".into(),
            found: "Bar".into(),
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("Foo"));
        assert!(debug.contains("Bar"));
    }

    #[test]
    fn account_info_clone() {
        let info = AccountInfo {
            account_name: "user".into(),
            character_name: "Char".into(),
            class_name: "Warrior".into(),
            level: 50,
            group_id: 1,
            server_name: "FV".into(),
        };
        let cloned = info.clone();
        assert_eq!(cloned, info);
    }

    // ── RetryPolicy / RetryState tests ──────────────────────────────────

    #[test]
    fn retry_policy_default_values() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 5);
        assert_eq!(policy.initial_delay, Duration::from_secs(2));
        assert_eq!(policy.max_delay, Duration::from_secs(60));
        assert!((policy.backoff_multiplier - 2.0).abs() < f64::EPSILON);
        assert!(policy.jitter);
    }

    #[test]
    fn retry_state_default_values() {
        let state = RetryState::default();
        assert_eq!(state.attempt_count, 0);
        assert_eq!(state.last_error, None);
    }

    #[test]
    fn retry_state_record_failure() {
        let mut state = RetryState::default();
        state.record_failure(LoginError::ServerDown);
        assert_eq!(state.attempt_count, 1);
        assert_eq!(state.last_error, Some(LoginError::ServerDown));

        state.record_failure(LoginError::Timeout {
            phase: "test".into(),
        });
        assert_eq!(state.attempt_count, 2);
        assert_eq!(
            state.last_error,
            Some(LoginError::Timeout {
                phase: "test".into()
            })
        );
    }

    #[test]
    fn retry_state_reset() {
        let mut state = RetryState::default();
        state.record_failure(LoginError::ServerDown);
        state.record_failure(LoginError::ServerFull);
        state.reset();
        assert_eq!(state.attempt_count, 0);
        assert_eq!(state.last_error, None);
    }

    #[test]
    fn retry_policy_exponential_backoff_without_jitter() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let expected = [2.0, 4.0, 8.0, 16.0, 32.0];
        for (i, &exp) in expected.iter().enumerate() {
            let state = RetryState {
                attempt_count: i as u32,
                last_error: None,
            };
            let delay = policy.next_delay(&state).unwrap();
            assert!(
                (delay.as_secs_f64() - exp).abs() < 0.001,
                "attempt {i}: expected {exp}, got {}",
                delay.as_secs_f64()
            );
        }
    }

    #[test]
    fn retry_policy_respects_max_delay() {
        let policy = RetryPolicy {
            max_retries: 10,
            initial_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 3.0,
            jitter: false,
        };

        let state = RetryState {
            attempt_count: 3,
            last_error: None,
        };
        let delay = policy.next_delay(&state).unwrap();
        assert!((delay.as_secs_f64() - 10.0).abs() < 0.001);
    }

    #[test]
    fn retry_policy_exhausted_returns_none() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let state = RetryState {
            attempt_count: 3,
            last_error: Some(LoginError::ServerDown),
        };
        assert!(policy.next_delay(&state).is_none());

        let state = RetryState {
            attempt_count: 10,
            last_error: None,
        };
        assert!(policy.next_delay(&state).is_none());
    }

    #[test]
    fn retry_policy_zero_retries_always_exhausted() {
        let policy = RetryPolicy {
            max_retries: 0,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: false,
        };
        let state = RetryState::default();
        assert!(policy.next_delay(&state).is_none());
    }

    #[test]
    fn retry_policy_jitter_stays_within_bounds() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_delay: Duration::from_secs(10),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 1.0,
            jitter: true,
        };
        let state = RetryState::default();

        for _ in 0..20 {
            let delay = policy.next_delay(&state).unwrap();
            let secs = delay.as_secs_f64();
            assert!(secs >= 7.4 && secs <= 12.6, "jitter out of bounds: {secs}");
        }
    }

    #[test]
    fn retry_policy_serialization_roundtrip() {
        let policy = RetryPolicy::default();
        let json = serde_json::to_string(&policy).expect("serialize");
        let restored: RetryPolicy = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.max_retries, policy.max_retries);
        assert_eq!(restored.initial_delay, policy.initial_delay);
        assert_eq!(restored.max_delay, policy.max_delay);
        assert!((restored.backoff_multiplier - policy.backoff_multiplier).abs() < f64::EPSILON);
        assert_eq!(restored.jitter, policy.jitter);
    }

    #[test]
    fn retry_state_serialization_roundtrip() {
        let mut state = RetryState::default();
        state.record_failure(LoginError::ServerFull);

        let json = serde_json::to_string(&state).expect("serialize");
        let restored: RetryState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.attempt_count, 1);
        assert_eq!(restored.last_error, Some(LoginError::ServerFull));
    }

    #[test]
    fn retry_policy_custom_multiplier() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(100),
            backoff_multiplier: 3.0,
            jitter: false,
        };

        let expected = [1.0, 3.0, 9.0];
        let delays: Vec<f64> = (0..3)
            .map(|i| {
                let state = RetryState {
                    attempt_count: i,
                    last_error: None,
                };
                policy.next_delay(&state).unwrap().as_secs_f64()
            })
            .collect();
        for (i, (&got, &exp)) in delays.iter().zip(expected.iter()).enumerate() {
            assert!(
                (got - exp).abs() < 0.001,
                "attempt {i}: expected {exp}, got {got}"
            );
        }
    }

    #[test]
    fn serde_duration_rejects_negative() {
        let json = r#"{"max_retries":5,"initial_delay":-1.0,"max_delay":60.0,"backoff_multiplier":2.0,"jitter":false}"#;
        let result: Result<RetryPolicy, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
