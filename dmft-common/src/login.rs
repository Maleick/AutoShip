use serde::{Deserialize, Serialize};

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
}
