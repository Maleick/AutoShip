use crate::types::ClientId;

/// Commands sent from the manager to an injected DLL
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum Command {
    // Movement
    MoveTo {
        x: f32,
        y: f32,
        z: f32,
    },
    StopMovement,
    // Combat
    CastSpell {
        spell_slot: u8,
        target_id: u32,
    },
    Attack {
        target_id: u32,
    },
    StopAttack,
    // Targeting
    SetTarget {
        spawn_id: u32,
    },
    ClearTarget,
    // Utility
    Sit,
    Stand,
    // Navigation
    /// Follow a sequence of waypoints.
    NavigateTo {
        waypoints: Vec<crate::nav::Waypoint>,
    },
    /// Move to a camp spot and face heading.
    SetCamp {
        spot: crate::nav::CampSpot,
    },
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
    JoinGroup {
        group_id: u32,
    },
    ApplyBuffs,
    ReportReady,
    // Combat
    CombatEngage {
        target_id: u32,
    },
    CombatDisengage,
    CombatSetAssistTarget {
        spawn_id: u32,
    },
    CombatForceAbility {
        ability_id: u32,
    },
    CombatEmergencyHeal {
        target_id: u32,
    },
    /// Loot the nearest corpse.
    LootCorpse,
    /// Loot all items from the currently open loot window.
    LootAll,
    // Soul Engine
    /// Send a chat message in-game.
    Say {
        channel: crate::soul::SayChannel,
        message: String,
        target: Option<String>,
    },
    /// Perform an emote animation.
    Emote {
        emote: String,
    },
    /// Execute a soul action (idle behavior, etc.).
    SoulAction {
        action: crate::soul::SoulAction,
    },
    /// Execute a slash command as if typed in the chat window.
    /// Uses EQ's InterpretCmd internally (e.g. "/target Camrene", "/follow").
    SlashCommand {
        command: String,
    },
    // Zone graph
    /// Request the zone adjacency graph from ZoneGuideManagerClient.
    QueryZoneGraph,
    // System
    Ping,
    Eject,
    SetHookState {
        enabled: bool,
    },
    /// Enable or disable automatic dialog acceptance (group invite, trade, etc.).
    SetAutoAccept {
        enabled: bool,
    },
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StartLogin {
                account_name,
                server_name,
                character_name,
                ..
            } => f
                .debug_struct("StartLogin")
                .field("account_name", account_name)
                .field("password", &"[REDACTED]")
                .field("server_name", server_name)
                .field("character_name", character_name)
                .finish(),
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
    /// Zone adjacency graph from ZoneGuideManagerClient.
    /// Simplified wire format: Vec of (zone_id, name, min_level, max_level, connections).
    /// Each connection is (dest_zone_id, transfer_type, disabled).
    ZoneGraph {
        zones: Vec<ZoneGraphEntry>,
    },
}

/// Wire-format for a single zone entry: (zone_id, name, min_level, max_level, connections).
/// Each connection is (dest_zone_id, transfer_type, disabled).
pub type ZoneGraphEntry = (u16, String, i32, i32, Vec<(u16, u8, bool)>);

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

/// Derive a deterministic `u64` session ID from a 32-byte session token.
/// Uses the first 8 bytes interpreted as little-endian. Both the DLL and
/// orchestrator call this on the same token to produce matching IPC names.
pub fn session_id_from_token(token: &SessionToken) -> u64 {
    u64::from_le_bytes(token[..8].try_into().unwrap())
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_deterministic() {
        let token: SessionToken = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // first 8 bytes → session_id
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // rest ignored
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let id = session_id_from_token(&token);
        // LE bytes: 0x0807060504030201
        assert_eq!(id, 0x0807060504030201);
        // Same token always yields the same id
        assert_eq!(session_id_from_token(&token), id);
    }

    #[test]
    fn session_id_zero_token() {
        let token: SessionToken = [0u8; 32];
        assert_eq!(session_id_from_token(&token), 0);
    }

    #[test]
    fn session_id_max_token() {
        let token: SessionToken = [0xFF; 32];
        assert_eq!(session_id_from_token(&token), u64::MAX);
    }

    #[test]
    fn pipe_name_format() {
        let name = pipe_name(0xDEADBEEF, 1234);
        assert_eq!(name, r"\\.\pipe\deadbeef_cmd_1234");
    }

    #[test]
    fn shared_memory_name_format() {
        let name = shared_memory_name(0xDEADBEEF, 1234);
        assert_eq!(name, "deadbeef_state_1234");
    }

    #[test]
    fn ipc_names_match_across_sides() {
        // Simulate both DLL and orchestrator deriving names from the same token
        let token: SessionToken = [
            0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x33, 0x44, // session_id bytes
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let pid: ClientId = 5678;

        // Both sides call session_id_from_token then pipe_name/shared_memory_name
        let dll_session_id = session_id_from_token(&token);
        let orch_session_id = session_id_from_token(&token);
        assert_eq!(dll_session_id, orch_session_id);

        assert_eq!(
            pipe_name(dll_session_id, pid),
            pipe_name(orch_session_id, pid)
        );
        assert_eq!(
            shared_memory_name(dll_session_id, pid),
            shared_memory_name(orch_session_id, pid)
        );
    }

    #[test]
    fn different_tokens_produce_different_names() {
        let token_a: SessionToken = [1; 32];
        let token_b: SessionToken = [2; 32];
        let pid = 100;

        let id_a = session_id_from_token(&token_a);
        let id_b = session_id_from_token(&token_b);
        assert_ne!(id_a, id_b);
        assert_ne!(pipe_name(id_a, pid), pipe_name(id_b, pid));
        assert_ne!(shared_memory_name(id_a, pid), shared_memory_name(id_b, pid));
    }

    #[test]
    fn different_pids_produce_different_names() {
        let session_id = 0x1234;
        assert_ne!(pipe_name(session_id, 100), pipe_name(session_id, 200));
        assert_ne!(
            shared_memory_name(session_id, 100),
            shared_memory_name(session_id, 200)
        );
    }

    #[test]
    fn pipe_name_no_predictable_prefix() {
        // Session-derived names should NOT start with the legacy prefix pattern
        let token: SessionToken = [0x42; 32];
        let id = session_id_from_token(&token);
        let name = pipe_name(id, 999);
        assert!(!name.contains("dmft_cmd_"));
        assert!(!name.contains("dmft_state_"));
    }

    #[test]
    fn command_roundtrip_start_login() {
        use crate::protocol::{decode, encode};

        let cmd = Command::StartLogin {
            account_name: "testuser".into(),
            password: "hunter2".into(),
            server_name: "Teek".into(),
            character_name: "Legolas".into(),
        };
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        if let Command::StartLogin {
            account_name,
            server_name,
            ..
        } = decoded
        {
            assert_eq!(account_name, "testuser");
            assert_eq!(server_name, "Teek");
        } else {
            panic!("expected StartLogin");
        }
    }

    #[test]
    fn response_roundtrip_login_phase_update() {
        use crate::login::LoginPhase;
        use crate::protocol::{decode, encode};

        let resp = Response::LoginPhaseUpdate {
            phase: LoginPhase::CharacterSelecting,
        };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode");
        if let Response::LoginPhaseUpdate { phase } = decoded {
            assert_eq!(phase, LoginPhase::CharacterSelecting);
        } else {
            panic!("expected LoginPhaseUpdate");
        }
    }

    #[test]
    fn response_roundtrip_command_result() {
        use crate::protocol::{decode, encode};

        let resp = Response::CommandResult {
            success: true,
            message: "queued".into(),
        };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode");
        if let Response::CommandResult { success, message } = decoded {
            assert!(success);
            assert_eq!(message, "queued");
        } else {
            panic!("expected CommandResult");
        }
    }
}
