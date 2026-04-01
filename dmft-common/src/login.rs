use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LoginPhase {
    NotStarted,
    ProcessLaunching,
    AtLoginScreen,
    EnteringCredentials,
    ServerSelecting,
    CharacterSelecting,
    Zoning,
    InWorld,
    PostLoginSetup,
    Ready,
    Failed { reason: LoginError },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LoginError {
    WrongPassword,
    AccountLocked,
    ServerDown,
    ServerFull,
    CharacterNotFound { expected: String, found: String },
    Timeout { phase: String },
    MassFailure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub account_name: String,
    pub character_name: String,
    pub class_name: String,
    pub level: u8,
    pub group_id: u32,
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
        assert_eq!(errors.len(), 7);
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
        assert_ne!(LoginError::WrongPassword, LoginError::AccountLocked);
        assert_ne!(LoginError::ServerDown, LoginError::ServerFull);
    }

    #[test]
    fn login_error_serialization_roundtrip() {
        let errors = vec![
            LoginError::WrongPassword,
            LoginError::AccountLocked,
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
        assert_eq!(cloned.account_name, info.account_name);
        assert_eq!(cloned.group_id, info.group_id);
    }
}
