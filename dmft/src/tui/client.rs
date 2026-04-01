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
