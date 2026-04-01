use crate::eq::structs::{GroupInfo, SpawnInfo};

/// Per-client state for each attached EQ process.
#[derive(Debug, Clone)]
pub struct ClientState {
    /// OS process ID of the EQ client.
    pub pid: u32,
    /// Base address of the EQ module in process memory.
    pub eq_base: u64,
    /// Local player spawn info (populated after reading memory).
    pub local_player: Option<SpawnInfo>,
    /// Currently targeted spawn, if any.
    pub target: Option<SpawnInfo>,
    /// All nearby spawns read from the spawn linked list.
    pub spawns: Vec<SpawnInfo>,
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
}

impl ClientState {
    /// Create a new client state for the given process.
    #[must_use]
    pub fn new(pid: u32, eq_base: u64) -> Self {
        Self {
            pid,
            eq_base,
            local_player: None,
            target: None,
            spawns: Vec::new(),
            zone_name: String::from("Unknown"),
            character_name: String::new(),
            group_info: None,
            client_status: format!("Attached to PID {pid}"),
            is_demo: false,
        }
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
}
