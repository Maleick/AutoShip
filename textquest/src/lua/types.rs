use std::collections::HashMap;

use textquest_common::types::SpawnData;

#[derive(Debug, Clone, PartialEq)]
pub struct LuaPlayerSnapshot {
    pub name: String,
    pub level: u8,
    pub class_name: String,
    pub class_id: u8,
    pub race_id: u32,
    pub hp: i64,
    pub hp_max: i64,
    pub mana: i32,
    pub mana_max: i32,
    pub endurance: i32,
    pub endurance_max: i32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub heading: f32,
    pub speed: f32,
    pub is_feigned: bool,
    pub is_dead: bool,
    pub is_gm: bool,
}

impl Default for LuaPlayerSnapshot {
    fn default() -> Self {
        Self {
            name: String::new(),
            level: 0,
            class_name: String::new(),
            class_id: 0,
            race_id: 0,
            hp: 0,
            hp_max: 0,
            mana: 0,
            mana_max: 0,
            endurance: 0,
            endurance_max: 0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            speed: 0.0,
            is_feigned: false,
            is_dead: false,
            is_gm: false,
        }
    }
}

impl LuaPlayerSnapshot {
    pub fn hp_percent(&self) -> f32 {
        if self.hp_max > 0 {
            (self.hp as f32 / self.hp_max as f32) * 100.0
        } else {
            0.0
        }
    }

    pub fn mana_percent(&self) -> f32 {
        if self.mana_max > 0 {
            (self.mana as f32 / self.mana_max as f32) * 100.0
        } else {
            0.0
        }
    }

    pub fn endurance_percent(&self) -> f32 {
        if self.endurance_max > 0 {
            (self.endurance as f32 / self.endurance_max as f32) * 100.0
        } else {
            0.0
        }
    }

    pub fn is_moving(&self) -> bool {
        self.speed > 0.0
    }
}

impl From<&SpawnData> for LuaPlayerSnapshot {
    fn from(spawn_data: &SpawnData) -> Self {
        Self {
            name: spawn_data.name.clone(),
            level: spawn_data.level,
            class_name: spawn_data.class_str(),
            class_id: spawn_data.class_id,
            race_id: spawn_data.race_id,
            hp: spawn_data.hp_current,
            hp_max: spawn_data.hp_max,
            mana: spawn_data.mana_current,
            mana_max: spawn_data.mana_max,
            endurance: spawn_data.endurance_current,
            endurance_max: spawn_data.endurance_max as i32,
            x: spawn_data.x,
            y: spawn_data.y,
            z: spawn_data.z,
            heading: spawn_data.heading,
            speed: spawn_data.speed_run,
            is_feigned: spawn_data.stand_state == 110,
            is_dead: spawn_data.stand_state == 111,
            is_gm: spawn_data.is_gm,
        }
    }
}

impl From<&LuaPlayer> for LuaPlayerSnapshot {
    fn from(player: &LuaPlayer) -> Self {
        Self::from(&player.spawn_data)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LuaNavigationRequest {
    Goto {
        x: f32,
        y: f32,
        z: f32,
    },
    Stick {
        target: String,
    },
    Stop,
    Follow {
        target: String,
    },
    AddWaypoint {
        x: f32,
        y: f32,
        z: f32,
        name: String,
    },
    ClearWaypoints,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuaCommandRequest {
    pub command: String,
    pub target_box: Option<String>,
    pub via_ipc: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LuaWaypoint {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LuaRuntimeState {
    pub player: Option<LuaPlayerSnapshot>,
    pub spawns: Vec<SpawnData>,
    pub target: Option<SpawnData>,
    pub xtargets: Vec<SpawnData>,
    pub waypoints: Vec<LuaWaypoint>,
    pub navigation_is_stuck: bool,
    pub navigation_stuck_reason: Option<String>,
    pub group_members: Vec<String>,
    pub group_tank: Option<String>,
    pub group_assist: Option<String>,
    pub group_master: Option<String>,
    pub buffs: Vec<String>,
    pub debuffs: Vec<String>,
    pub plugin_config: HashMap<String, String>,
    pub last_saved_config: bool,
    pub last_reloaded_config: bool,
    pub navigation_requests: Vec<LuaNavigationRequest>,
    pub command_requests: Vec<LuaCommandRequest>,
}

pub struct LuaPlayer {
    pub spawn_data: SpawnData,
}

impl LuaPlayer {
    pub fn new(spawn_data: SpawnData) -> Self {
        Self { spawn_data }
    }

    pub fn hp(&self) -> i64 {
        self.spawn_data.hp_current
    }

    pub fn hp_max(&self) -> i64 {
        self.spawn_data.hp_max
    }

    pub fn hp_percent(&self) -> f32 {
        if self.spawn_data.hp_max > 0 {
            (self.spawn_data.hp_current as f32 / self.spawn_data.hp_max as f32) * 100.0
        } else {
            0.0
        }
    }

    pub fn mana(&self) -> i32 {
        self.spawn_data.mana_current
    }

    pub fn mana_max(&self) -> i32 {
        self.spawn_data.mana_max
    }

    pub fn mana_percent(&self) -> f32 {
        if self.spawn_data.mana_max > 0 {
            (self.spawn_data.mana_current as f32 / self.spawn_data.mana_max as f32) * 100.0
        } else {
            0.0
        }
    }

    pub fn endurance(&self) -> i32 {
        self.spawn_data.endurance_current
    }

    pub fn endurance_max(&self) -> i32 {
        self.spawn_data.endurance_max as i32
    }

    pub fn endurance_percent(&self) -> f32 {
        if self.spawn_data.endurance_max > 0 {
            (self.spawn_data.endurance_current as f32 / self.spawn_data.endurance_max as f32)
                * 100.0
        } else {
            0.0
        }
    }

    pub fn name(&self) -> &str {
        &self.spawn_data.name
    }

    pub fn level(&self) -> u8 {
        self.spawn_data.level
    }

    pub fn class_str(&self) -> String {
        self.spawn_data.class_str()
    }

    pub fn class_id(&self) -> u8 {
        self.spawn_data.class_id
    }

    pub fn race_id(&self) -> u32 {
        self.spawn_data.race_id
    }

    pub fn x(&self) -> f32 {
        self.spawn_data.x
    }

    pub fn y(&self) -> f32 {
        self.spawn_data.y
    }

    pub fn z(&self) -> f32 {
        self.spawn_data.z
    }

    pub fn heading(&self) -> f32 {
        self.spawn_data.heading
    }

    pub fn speed(&self) -> f32 {
        self.spawn_data.speed_run
    }

    pub fn is_moving(&self) -> bool {
        self.spawn_data.speed_run > 0.0
    }

    pub fn stand_state(&self) -> u8 {
        self.spawn_data.stand_state
    }

    pub fn is_feigned(&self) -> bool {
        self.spawn_data.stand_state == 110
    }

    pub fn is_dead(&self) -> bool {
        self.spawn_data.stand_state == 111
    }

    pub fn is_gm(&self) -> bool {
        self.spawn_data.is_gm
    }
}

pub struct LuaGroupMember {
    pub name: String,
    pub hp_percent: f32,
    pub mana_percent: f32,
    pub level: u8,
    pub class_id: u8,
    pub online: bool,
    pub is_tank: bool,
}

impl LuaGroupMember {
    pub fn new(name: String, hp_percent: f32, class_id: u8) -> Self {
        Self {
            name,
            hp_percent,
            mana_percent: 0.0,
            level: 0,
            class_id,
            online: true,
            is_tank: false,
        }
    }
}

pub struct LuaTarget {
    pub spawn_data: SpawnData,
}

impl LuaTarget {
    pub fn new(spawn_data: SpawnData) -> Self {
        Self { spawn_data }
    }

    pub fn spawn_id(&self) -> u32 {
        self.spawn_data.spawn_id
    }

    pub fn name(&self) -> &str {
        &self.spawn_data.name
    }

    pub fn level(&self) -> u8 {
        self.spawn_data.level
    }

    pub fn hp(&self) -> i64 {
        self.spawn_data.hp_current
    }

    pub fn hp_max(&self) -> i64 {
        self.spawn_data.hp_max
    }

    pub fn hp_percent(&self) -> f32 {
        if self.spawn_data.hp_max > 0 {
            (self.spawn_data.hp_current as f32 / self.spawn_data.hp_max as f32) * 100.0
        } else {
            0.0
        }
    }

    pub fn x(&self) -> f32 {
        self.spawn_data.x
    }

    pub fn y(&self) -> f32 {
        self.spawn_data.y
    }

    pub fn z(&self) -> f32 {
        self.spawn_data.z
    }

    pub fn distance(&self, x: f32, y: f32, z: f32) -> f32 {
        let dx = self.spawn_data.x - x;
        let dy = self.spawn_data.y - y;
        let dz = self.spawn_data.z - z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

pub struct LuaSpawn {
    pub spawn_data: SpawnData,
}

impl LuaSpawn {
    pub fn new(spawn_data: SpawnData) -> Self {
        Self { spawn_data }
    }

    pub fn spawn_id(&self) -> u32 {
        self.spawn_data.spawn_id
    }

    pub fn name(&self) -> &str {
        &self.spawn_data.name
    }

    pub fn displayed_name(&self) -> &str {
        &self.spawn_data.displayed_name
    }

    pub fn spawn_type(&self) -> u8 {
        self.spawn_data.spawn_type
    }

    pub fn level(&self) -> u8 {
        self.spawn_data.level
    }

    pub fn class_id(&self) -> u8 {
        self.spawn_data.class_id
    }

    pub fn race_id(&self) -> u32 {
        self.spawn_data.race_id
    }

    pub fn x(&self) -> f32 {
        self.spawn_data.x
    }

    pub fn y(&self) -> f32 {
        self.spawn_data.y
    }

    pub fn z(&self) -> f32 {
        self.spawn_data.z
    }

    pub fn heading(&self) -> f32 {
        self.spawn_data.heading
    }

    pub fn hp(&self) -> i64 {
        self.spawn_data.hp_current
    }

    pub fn hp_max(&self) -> i64 {
        self.spawn_data.hp_max
    }

    pub fn hp_percent(&self) -> f32 {
        if self.spawn_data.hp_max > 0 {
            (self.spawn_data.hp_current as f32 / self.spawn_data.hp_max as f32) * 100.0
        } else {
            0.0
        }
    }

    pub fn mana(&self) -> i32 {
        self.spawn_data.mana_current
    }

    pub fn speed(&self) -> f32 {
        self.spawn_data.speed_run
    }

    pub fn is_moving(&self) -> bool {
        self.spawn_data.speed_run > 0.0
    }

    pub fn class_str(&self) -> String {
        self.spawn_data.class_str()
    }
}
