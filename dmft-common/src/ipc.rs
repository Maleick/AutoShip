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
    // Navigation
    /// Follow a sequence of waypoints.
    NavigateTo { waypoints: Vec<crate::nav::Waypoint> },
    /// Move to a camp spot and face heading.
    SetCamp { spot: crate::nav::CampSpot },
    /// Stop navigating, stay where you are.
    StopNavigation,
    // Login automation
    LoginPhaseQuery,
    // Post-login
    JoinGroup { group_id: u32 },
    ApplyBuffs,
    ReportReady,
    // Combat
    CombatEngage { target_id: u32 },
    CombatDisengage,
    CombatSetAssistTarget { spawn_id: u32 },
    CombatForceAbility { ability_id: u32 },
    CombatEmergencyHeal { target_id: u32 },
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
    /// Navigation status push notification from the DLL's nav state machine.
    /// Note: NavStatus is also available in `GameState.nav_status` (shared memory).
    /// `GameState.nav_status` is authoritative — it is updated every tick.
    /// `NavUpdate` is sent only on state transitions (Idle→Moving, Moving→Arrived, etc.)
    /// for low-latency notification without polling shared memory.
    NavUpdate {
        status: crate::nav::NavStatus,
    },
    LoginPhaseUpdate {
        phase: crate::login::LoginPhase,
    },
    PostLoginComplete {
        client_id: crate::types::ClientId,
    },
    CombatUpdate {
        status: crate::combat::CombatStatus,
    },
}

/// Random session token generated at injection time for IPC authentication.
/// The orchestrator writes this to shared memory; the DLL reads it and
/// validates it on every pipe connection.
pub type SessionToken = [u8; 32];

/// Size of shared memory region allocated per client (64 KB)
pub const SHARED_MEMORY_SIZE: usize = 64 * 1024;

/// Named pipe prefix for per-client IPC channels
pub const PIPE_NAME_PREFIX: &str = r"\\.\pipe\dmft_";

/// Shared memory name prefix for per-client game state regions
pub const SHARED_MEMORY_NAME_PREFIX: &str = "dmft_state_";
