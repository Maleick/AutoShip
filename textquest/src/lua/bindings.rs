//! Lua API bindings for TextQuest.
//!
//! This module registers TextQuest core APIs with the Lua runtime, organized by domain:
//! - player: textquest.player.*
//! - group: textquest.group.*
//! - nav: textquest.nav.*
//! - combat: textquest.combat.*
//! - state: textquest.state.*
/// - config: textquest.config.*
/// - log: textquest.log.*

use mlua::{Lua, Result as LuaResult, Table};
use std::sync::Arc;

use crate::lua::error::LuaApiError;
use crate::lua::types::{LuaGroupMember, LuaPlayer, LuaSpawn, LuaTarget};

pub struct LuaBindings {
    lua: Lua,
}

impl LuaBindings {
    pub fn new() -> Result<Self, LuaApiError> {
        let lua = Lua::new();
        Ok(Self { lua })
    }

    pub fn register_apis(&self) -> LuaResult<()> {
        let globals = self.lua.globals();

        let textquest = self.lua.create_table()?;

        self.register_player_api(&textquest)?;
        self.register_group_api(&textquest)?;
        self.register_nav_api(&textquest)?;
        self.register_combat_api(&textquest)?;
        self.register_state_api(&textquest)?;
        self.register_config_api(&textquest)?;
        self.register_log_api(&textquest)?;
        self.register_events_api(&textquest)?;

        globals.set("textquest", textquest)?;

        Ok(())
    }

    fn register_player_api(&self, parent: &Table) -> LuaResult<()> {
        let player = self.lua.create_table()?;

        player.set("get_hp", self.lua.create_function(|_, ()| Ok(0i64))?)?;
        player.set("get_hp_percent", self.lua.create_function(|_, ()| Ok(0f32))?)?;
        player.set("get_mana", self.lua.create_function(|_, ()| Ok(0i32))?)?;
        player.set("get_mana_percent", self.lua.create_function(|_, ()| Ok(0f32))?)?;
        player.set("get_endurance", self.lua.create_function(|_, ()| Ok(0i32))?)?;
        player.set("get_endurance_percent", self.lua.create_function(|_, ()| Ok(0f32))?)?;
        player.set("get_name", self.lua.create_function(|_, ()| Ok("".to_string()))?)?;
        player.set("get_level", self.lua.create_function(|_, ()| Ok(0u8))?)?;
        player.set("get_class", self.lua.create_function(|_, ()| Ok("".to_string()))?)?;
        player.set("get_class_id", self.lua.create_function(|_, ()| Ok(0u8))?)?;
        player.set("get_race_id", self.lua.create_function(|_, ()| Ok(0u32))?)?;
        player.set("get_x", self.lua.create_function(|_, ()| Ok(0f32))?)?;
        player.set("get_y", self.lua.create_function(|_, ()| Ok(0f32))?)?;
        player.set("get_z", self.lua.create_function(|_, ()| Ok(0f32))?)?;
        player.set("get_heading", self.lua.create_function(|_, ()| Ok(0f32))?)?;
        player.set("get_speed", self.lua.create_function(|_, ()| Ok(0f32))?)?;
        player.set("is_moving", self.lua.create_function(|_, ()| Ok(false))?)?;
        player.set("is_feigned", self.lua.create_function(|_, ()| Ok(false))?)?;
        player.set("is_dead", self.lua.create_function(|_, ()| Ok(false))?)?;
        player.set("is_gm", self.lua.create_function(|_, ()| Ok(false))?)?;

        parent.set("player", player)?;

        Ok(())
    }

    fn register_group_api(&self, parent: &Table) -> LuaResult<()> {
        let group = self.lua.create_table()?;

        group.set(
            "get_member_count",
            self.lua.create_function(|_, ()| Ok(0usize))?,
        )?;
        group.set(
            "get_member",
            self.lua
                .create_function(|_, index: usize| Ok(mlua::Value::Nil))?,
        )?;
        group.set(
            "get_members",
            self.lua.create_function(|_, ()| Ok(self.lua.create_table()?))?,
        )?;
        group.set(
            "get_tank",
            self.lua.create_function(|_, ()| Ok(mlua::Value::Nil))?,
        )?;
        group.set(
            "get_assist",
            self.lua.create_function(|_, ()| Ok(mlua::Value::Nil))?,
        )?;
        group.set(
            "get_master",
            self.lua.create_function(|_, ()| Ok(mlua::Value::Nil))?,
        )?;

        parent.set("group", group)?;

        Ok(())
    }

    fn register_nav_api(&self, parent: &Table) -> LuaResult<()> {
        let nav = self.lua.create_table()?;

        nav.set(
            "goto",
            self.lua.create_function(|_, (x, y, z): (f32, f32, f32)| {
                tracing::debug!("nav.goto({}, {}, {})", x, y, z);
                Ok(true)
            })?,
        )?;
        nav.set(
            "stick",
            self.lua.create_function(|_, target: String| {
                tracing::debug!("nav.stick(\"{}\")", target);
                Ok(true)
            })?,
        )?;
        nav.set(
            "stop",
            self.lua.create_function(|_, ()| {
                tracing::debug!("nav.stop()");
                Ok(true)
            })?,
        )?;
        nav.set(
            "follow",
            self.lua.create_function(|_, target: String| {
                tracing::debug!("nav.follow(\"{}\")", target);
                Ok(true)
            })?,
        )?;
        nav.set(
            "add_waypoint",
            self.lua.create_function(|_, (x, y, z, name): (f32, f32, f32, String)| {
                tracing::debug!("nav.add_waypoint({}, {}, {}, \"{}\")", x, y, z, name);
                Ok(true)
            })?,
        )?;
        nav.set(
            "clear_waypoints",
            self.lua.create_function(|_, ()| {
                tracing::debug!("nav.clear_waypoints()");
                Ok(true)
            })?,
        )?;

        parent.set("nav", nav)?;

        Ok(())
    }

    fn register_combat_api(&self, parent: &Table) -> LuaResult<()> {
        let combat = self.lua.create_table()?;

        combat.set(
            "cast",
            self.lua.create_function(|_, (spell, target): (String, Option<String>)| {
                tracing::debug!("combat.cast(\"{:?}\", {:?})", spell, target);
                Ok(true)
            })?,
        )?;
        combat.set(
            "assist",
            self.lua.create_function(|_, target: Option<String>| {
                tracing::debug!("combat.assist({:?})", target);
                Ok(true)
            })?,
        )?;
        combat.set(
            "attack",
            self.lua.create_function(|_, target: Option<String>| {
                tracing::debug!("combat.attack({:?})", target);
                Ok(true)
            })?,
        )?;
        combat.set(
            "disengage",
            self.lua.create_function(|_, ()| {
                tracing::debug!("combat.disengage()");
                Ok(true)
            })?,
        )?;
        combat.set(
            "re mez",
            self.lua.create_function(|_, target: String| {
                tracing::debug!("combat.re_mez(\"{}\")", target);
                Ok(true)
            })?,
        )?;
        combat.set(
            "rezz",
            self.lua.create_function(|_, target: Option<String>| {
                tracing::debug!("combat.rezz({:?})", target);
                Ok(true)
            })?,
        )?;

        parent.set("combat", combat)?;

        Ok(())
    }

    fn register_state_api(&self, parent: &Table) -> LuaResult<()> {
        let state = self.lua.create_table()?;

        state.set(
            "get_spawns",
            self.lua.create_function(|_, ()| Ok(self.lua.create_table()?))?,
        )?;
        state.set(
            "get_spawn",
            self.lua
                .create_function(|_, name: String| Ok(mlua::Value::Nil))?,
        )?;
        state.set(
            "find_spawns",
            self.lua
                .create_function(|_, filter: String| Ok(self.lua.create_table()?))?,
        )?;
        state.set(
            "get_target",
            self.lua.create_function(|_, ()| Ok(mlua::Value::Nil))?,
        )?;
        state.set(
            "set_target",
            self.lua.create_function(|_, target: String| Ok(true))?,
        )?;
        state.set(
            "get_xtargets",
            self.lua.create_function(|_, ()| Ok(self.lua.create_table()?))?,
        )?;

        parent.set("state", state)?;

        Ok(())
    }

    fn register_config_api(&self, parent: &Table) -> LuaResult<()> {
        let config = self.lua.create_table()?;

        config.set(
            "get",
            self.lua
                .create_function(|_, key: String| Ok(mlua::Value::Nil))?,
        )?;
        config.set(
            "set",
            self.lua.create_function(|_, (key, value): (String, mlua::Value)| {
                tracing::debug!("config.set(\"{}\", {:?})", key, value);
                Ok(true)
            })?,
        )?;
        config.set(
            "save",
            self.lua.create_function(|_, ()| {
                tracing::debug!("config.save()");
                Ok(true)
            })?,
        )?;
        config.set(
            "reload",
            self.lua.create_function(|_, ()| {
                tracing::debug!("config.reload()");
                Ok(true)
            })?,
        )?;

        parent.set("config", config)?;

        Ok(())
    }

    fn register_log_api(&self, parent: &Table) -> LuaResult<()> {
        let log = self.lua.create_table()?;

        log.set(
            "info",
            self.lua.create_function(|_, msg: String| {
                tracing::info!("[Lua] {}", msg);
                Ok(())
            })?,
        )?;
        log.set(
            "warn",
            self.lua.create_function(|_, msg: String| {
                tracing::warn!("[Lua] {}", msg);
                Ok(())
            })?,
        )?;
        log.set(
            "error",
            self.lua.create_function(|_, msg: String| {
                tracing::error!("[Lua] {}", msg);
                Ok(())
            })?,
        )?;
        log.set(
            "debug",
            self.lua.create_function(|_, msg: String| {
                tracing::debug!("[Lua] {}", msg);
                Ok(())
            })?,
        )?;

        parent.set("log", log)?;

        Ok(())
    }

    fn register_events_api(&self, parent: &Table) -> LuaResult<()> {
        let events = self.lua.create_table()?;

        events.set(
            "on",
            self.lua.create_function(
                |_, (event, callback): (String, mlua::Function)| {
                    tracing::debug!("events.on(\"{}\")", event);
                    Ok(())
                },
            )?,
        )?;
        events.set(
            "off",
            self.lua.create_function(|_, event: String| {
                tracing::debug!("events.off(\"{}\")", event);
                Ok(())
            })?,
        )?;
        events.set(
            "emit",
            self.lua.create_function(|_, (event, data): (String, mlua::Value)| {
                tracing::debug!("events.emit(\"{}\", {:?})", event, data);
                Ok(())
            })?,
        )?;

        parent.set("events", events)?;

        Ok(())
    }

    pub fn get_lua(&self) -> &Lua {
        &self.lua
    }
}

impl Default for LuaBindings {
    fn default() -> Self {
        Self::new().expect("Failed to create Lua context")
    }
}