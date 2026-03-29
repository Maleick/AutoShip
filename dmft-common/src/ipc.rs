use crate::types::ClientId;

/// Commands sent from the manager to an injected DLL
#[derive(Clone, serde::Serialize, serde::Deserialize)]
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
    /// Dump all login-related pointer addresses to the DLL log for calibration.
    /// Used to validate offsets on the live client before attempting auto-login.
    CalibrateLogin,
    /// Start the automated login sequence. The DLL handles all UI steps
    /// autonomously and reports progress via LoginPhaseUpdate responses.
    /// Password is zeroized in DLL memory immediately after use.
    StartLogin {
        account_name: String,
        password: String,
        server_name: String,
        character_name: String,
    },
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
    // Soul Engine
    /// Send a chat message in-game.
    Say {
        channel: crate::soul::SayChannel,
        message: String,
        target: Option<String>,
    },
    /// Perform an emote animation.
    Emote { emote: String },
    /// Execute a soul action (idle behavior, etc.).
    SoulAction { action: crate::soul::SoulAction },
    /// Execute a slash command as if typed in the chat window.
    /// Uses EQ's InterpretCmd internally (e.g. "/target Camrene", "/follow").
    SlashCommand { command: String },
    // System
    Ping,
    Eject,
    SetHookState { enabled: bool },
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StartLogin { account_name, server_name, character_name, .. } => {
                f.debug_struct("StartLogin")
                    .field("account_name", account_name)
                    .field("password", &"[REDACTED]")
                    .field("server_name", server_name)
                    .field("character_name", character_name)
                    .finish()
            }
            other => write!(f, "{}", {
                // Fall through to derived-style output for all other variants.
                // This uses serde_json as a quick Debug proxy since we removed derive(Debug).
                serde_json::to_string(other).unwrap_or_else(|_| "Command(?)".to_string())
            }),
        }
    }
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

/// Legacy named pipe prefix — prefer `pipe_name()` with a session ID.
pub const PIPE_NAME_PREFIX: &str = r"\\.\pipe\dmft_";

/// Legacy shared memory name prefix — prefer `shared_memory_name()` with a session ID.
pub const SHARED_MEMORY_NAME_PREFIX: &str = "dmft_state_";

/// Build a per-client pipe name incorporating a random session ID.
/// Format: `\\.\pipe\{session_id:x}_cmd_{client_id}`
pub fn pipe_name(session_id: u64, client_id: u32) -> String {
    format!(r"\\.\pipe\{:x}_cmd_{}", session_id, client_id)
}

/// Build a per-client shared memory name incorporating a random session ID.
/// Format: `{session_id:x}_state_{client_id}`
pub fn shared_memory_name(session_id: u64, client_id: u32) -> String {
    format!("{:x}_state_{}", session_id, client_id)
}
