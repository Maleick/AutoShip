use mlua::{Lua, Result as LuaResult};
use textquest_common::types::SpawnData;

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