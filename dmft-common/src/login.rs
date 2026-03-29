use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
