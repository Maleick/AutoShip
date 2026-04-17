use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerWatchConfig {
    pub watch_zones: Vec<String>,
    pub alert_on_entry: bool,
    pub alert_on_exit: bool,
}
