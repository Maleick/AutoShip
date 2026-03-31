use crate::eq::structs::{GroupInfo, SpawnInfo};

/// Per-client state for each attached EQ process.
#[derive(Debug, Clone)]
pub struct ClientState {
    pub pid: u32,
    pub eq_base: u64,
    pub local_player: Option<SpawnInfo>,
    pub target: Option<SpawnInfo>,
    pub spawns: Vec<SpawnInfo>,
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
            client_status: format!("Attached to PID {}", pid),
            is_demo: false,
        }
    }
}
