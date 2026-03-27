use crate::types::ClientId;

/// Commands sent from the manager to an injected DLL
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Command {
    // Movement
    MoveTo { x: f32, y: f32, z: f32 },
    StopMovement,
    // Combat
    CastSpell { spell_slot: u8, target_id: u32 },
    Attack { target_id: u32 },
    StopAttack,
    // Targeting
    SetTarget { spawn_id: u32 },
    ClearTarget,
    // Utility
    Sit,
    Stand,
    // System
    Ping,
    Eject,
    SetHookState { enabled: bool },
}

/// Responses sent from the DLL back to the manager
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Response {
    Pong {
        client_id: ClientId,
        timestamp_ms: u64,
    },
    CommandResult {
        success: bool,
        message: String,
    },
    Error {
        message: String,
    },
}

/// Size of shared memory region allocated per client (64 KB)
pub const SHARED_MEMORY_SIZE: usize = 64 * 1024;

/// Named pipe prefix for per-client IPC channels
pub const PIPE_NAME_PREFIX: &str = r"\\.\pipe\dmft_";
