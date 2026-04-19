use super::live_cast_capture::LiveCastCaptureSnapshot;
use crate::eq::structs::{GroupInfo, SpawnInfo};
use std::time::Instant;
use textquest_common::types::SlotLifecycle;

/// Per-client state for each attached EQ process.
#[derive(Debug, Clone)]
pub struct ClientState {
    /// OS process ID of the EQ client.
    pub pid: u32,
    /// Whether the IPC path for this client is considered available.
    pub connected: bool,
    /// Base address of the EQ module in process memory.
    pub eq_base: u64,
    /// Local player spawn info (populated after reading memory).
    pub local_player: Option<SpawnInfo>,
    /// Currently targeted spawn, if any.
    pub target: Option<SpawnInfo>,
    /// All nearby spawns read from the spawn linked list.
    pub spawns: Vec<SpawnInfo>,
    /// Most recent fast-field refresh timestamp (player/target/group).
    pub last_fast_refresh: Option<Instant>,
    /// Most recent spawn-list refresh timestamp.
    pub last_spawn_refresh: Option<Instant>,
    /// Monotonic version of the last successful spawn snapshot.
    pub spawn_revision: u64,
    /// Current zone short name.
    pub zone_name: String,
    /// Character name parsed from the DLL-renamed window title.
    pub character_name: String,
    /// Group membership info for this client.
    pub group_info: Option<GroupInfo>,
    /// Status message specific to this client.
    pub client_status: String,
    /// Whether this client was created from demo data (not a real process).
    pub is_demo: bool,
    /// Last logged live cast snapshot when `TEXTQUEST_CAST_CAPTURE=1`.
    pub last_live_cast_capture: Option<LiveCastCaptureSnapshot>,
    /// Operator-visible lifecycle state for this slot.
    pub slot_lifecycle: SlotLifecycle,
    /// Name of the launch profile used to start this session slot, if any.
    pub launch_profile: Option<String>,
    /// Name of the active session preset that this slot belongs to, if any.
    pub session_preset: Option<String>,
}

impl ClientState {
    /// Create a new client state for the given process.
    #[must_use]
    pub fn new(pid: u32, eq_base: u64) -> Self {
        Self {
            pid,
            connected: true,
            eq_base,
            local_player: None,
            target: None,
            spawns: Vec::new(),
            last_fast_refresh: None,
            last_spawn_refresh: None,
            spawn_revision: 0,
            zone_name: String::from("Unknown"),
            character_name: String::new(),
            group_info: None,
            client_status: format!("Attached to PID {pid}"),
            is_demo: false,
            last_live_cast_capture: None,
            slot_lifecycle: SlotLifecycle::Configured,
            launch_profile: None,
            session_preset: None,
        }
    }

    pub fn send_command(&self, command: &str) -> anyhow::Result<()> {
        crate::command_dispatch::dispatch_local_command(self.pid, command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_pid_and_base() {
        let cs = ClientState::new(1234, 0x140000000);
        assert_eq!(cs.pid, 1234);
        assert_eq!(cs.eq_base, 0x140000000);
    }

    #[test]
    fn new_defaults_no_player() {
        let cs = ClientState::new(1, 0);
        assert!(cs.local_player.is_none());
        assert!(cs.target.is_none());
    }

    #[test]
    fn new_defaults_empty_spawns() {
        let cs = ClientState::new(1, 0);
        assert!(cs.spawns.is_empty());
        assert_eq!(cs.spawn_revision, 0);
        assert!(cs.last_fast_refresh.is_none());
        assert!(cs.last_spawn_refresh.is_none());
    }

    #[test]
    fn new_defaults_zone_unknown() {
        let cs = ClientState::new(1, 0);
        assert_eq!(cs.zone_name, "Unknown");
    }

    #[test]
    fn new_defaults_empty_character_name() {
        let cs = ClientState::new(1, 0);
        assert!(cs.character_name.is_empty());
    }

    #[test]
    fn new_defaults_no_group_info() {
        let cs = ClientState::new(1, 0);
        assert!(cs.group_info.is_none());
    }

    #[test]
    fn new_status_contains_pid() {
        let cs = ClientState::new(5678, 0);
        assert!(cs.client_status.contains("5678"));
    }

    #[test]
    fn new_defaults_not_demo() {
        let cs = ClientState::new(1, 0);
        assert!(!cs.is_demo);
    }

    #[test]
    fn new_defaults_no_live_cast_capture() {
        let cs = ClientState::new(1, 0);
        assert!(cs.last_live_cast_capture.is_none());
    }

    #[test]
    fn new_defaults_slot_lifecycle_configured() {
        let cs = ClientState::new(1, 0);
        assert!(matches!(
            cs.slot_lifecycle,
            textquest_common::types::SlotLifecycle::Configured
        ));
    }

    #[test]
    fn new_defaults_no_launch_profile() {
        let cs = ClientState::new(1, 0);
        assert!(cs.launch_profile.is_none());
    }

    #[test]
    fn new_defaults_no_session_preset() {
        let cs = ClientState::new(1, 0);
        assert!(cs.session_preset.is_none());
    }
}
