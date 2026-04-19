use thiserror::Error;

#[derive(Debug, Error)]
pub enum LuaApiError {
    #[error("Player not found")]
    PlayerNotFound,

    #[error("Target not found")]
    TargetNotFound,

    #[error("Group not found")]
    GroupNotFound,

    #[error("Spawn not found: {0}")]
    SpawnNotFound(String),

    #[error("Invalid spawn filter: {0}")]
    InvalidFilter(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Navigation error: {0}")]
    NavError(String),

    #[error("Combat error: {0}")]
    CombatError(String),

    #[error("Spell not found: {0}")]
    SpellNotFound(String),

    #[error("Event subscription error: {0}")]
    EventError(String),

    #[error("Lua binding error: {0}")]
    BindingError(String),
}

impl std::fmt::Display for LuaApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}