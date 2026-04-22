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
/// - hotkeys: textquest.hotkeys.*
/// - commands: textquest.commands.*

use mlua::{Lua, Result as LuaResult, Table};
use std::sync::Arc;

use crate::lua::error::LuaApiError;
use crate::lua::types::{LuaGroupMember, LuaPlayer, LuaSpawn, LuaTarget};
use crate::registry::{Priority, SharedCommandRegistry, SharedHotkeyRegistry};

pub struct LuaBindings {
    lua: Lua,
    /// Shared command registry — injected so plugins and Lua share the same table.
    command_registry: SharedCommandRegistry,
    /// Shared hotkey registry — injected so plugins and Lua share the same table.
    hotkey_registry: SharedHotkeyRegistry,
}

impl LuaBindings {
    /// Create bindings with freshly-created private registries.
    pub fn new() -> Result<Self, LuaApiError> {
        let lua = Lua::new();
        let (command_registry, hotkey_registry) = crate::registry::new_shared();
        Ok(Self { lua, command_registry, hotkey_registry })
    }

    /// Create bindings that share existing registries (e.g. with the plugin loader).
    pub fn with_registries(
        command_registry: SharedCommandRegistry,
        hotkey_registry: SharedHotkeyRegistry,
    ) -> Result<Self, LuaApiError> {
        let lua = Lua::new();
        Ok(Self { lua, command_registry, hotkey_registry })
    }

    /// Return a clone of the shared command registry handle.
    pub fn command_registry(&self) -> SharedCommandRegistry {
        Arc::clone(&self.command_registry)
    }

    /// Return a clone of the shared hotkey registry handle.
    pub fn hotkey_registry(&self) -> SharedHotkeyRegistry {
        Arc::clone(&self.hotkey_registry)
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
        self.register_hotkeys_api(&textquest)?;
        self.register_commands_api(&textquest)?;

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

    // ── Hotkeys API ──────────────────────────────────────────────────────────

    /// Expose `textquest.hotkeys.*` to Lua.
    ///
    /// ```lua
    /// local id = textquest.hotkeys.register("ctrl+f5", function()
    ///     textquest.log.info("Ctrl+F5 pressed!")
    /// end)
    /// textquest.hotkeys.unregister(id)
    /// ```
    fn register_hotkeys_api(&self, parent: &Table) -> LuaResult<()> {
        let hotkeys = self.lua.create_table()?;

        // textquest.hotkeys.register(combo_string, callback) → hotkey_id (integer)
        let hk_reg = Arc::clone(&self.hotkey_registry);
        hotkeys.set(
            "register",
            self.lua.create_function(move |_lua, (combo, callback): (String, mlua::Function)| {
                // Store the Lua function as a persistent reference.
                // We use mlua's `into_owned()` so the Function outlives the
                // current Lua call frame.  The OwnedFunction keeps the Lua VM
                // alive via the Arc<Lua> hidden inside mlua.
                let owned_cb: mlua::OwnedFunction = callback.into_owned();
                let id = hk_reg.lock().unwrap().register(
                    &combo,
                    Priority::Script,
                    "lua",
                    Box::new(move || {
                        if let Err(e) = owned_cb.call::<()>(()) {
                            tracing::warn!(error = %e, "Lua hotkey callback error");
                        }
                    }),
                );
                tracing::debug!(combo = %combo, id = ?id, "Lua registered hotkey");
                Ok(id.0)
            })?,
        )?;

        // textquest.hotkeys.unregister(hotkey_id) → boolean
        let hk_unreg = Arc::clone(&self.hotkey_registry);
        hotkeys.set(
            "unregister",
            self.lua.create_function(move |_lua, id: u64| {
                use crate::registry::ScriptHotkeyId;
                let removed = hk_unreg.lock().unwrap().unregister(ScriptHotkeyId(id));
                Ok(removed)
            })?,
        )?;

        // textquest.hotkeys.fire(combo_string) → boolean  (test/debug helper)
        let hk_fire = Arc::clone(&self.hotkey_registry);
        hotkeys.set(
            "fire",
            self.lua.create_function(move |_lua, combo: String| {
                let fired = hk_fire.lock().unwrap().fire(&combo);
                Ok(fired)
            })?,
        )?;

        parent.set("hotkeys", hotkeys)?;
        Ok(())
    }

    // ── Commands API ─────────────────────────────────────────────────────────

    /// Expose `textquest.commands.*` to Lua.
    ///
    /// ```lua
    /// local id = textquest.commands.register("/mymod", function(args)
    ///     textquest.log.info("mymod called with: " .. args)
    /// end)
    /// textquest.commands.unregister(id)
    /// ```
    fn register_commands_api(&self, parent: &Table) -> LuaResult<()> {
        let commands = self.lua.create_table()?;

        // textquest.commands.register(path, callback) → command_id (integer)
        let cmd_reg = Arc::clone(&self.command_registry);
        commands.set(
            "register",
            self.lua.create_function(move |_lua, (path, callback): (String, mlua::Function)| {
                let owned_cb: mlua::OwnedFunction = callback.into_owned();
                let id = cmd_reg.lock().unwrap().register(
                    &path,
                    Priority::Script,
                    "lua",
                    Box::new(move |tail: &str| {
                        if let Err(e) = owned_cb.call::<()>(tail.to_string()) {
                            tracing::warn!(error = %e, "Lua command callback error");
                        }
                    }),
                );
                tracing::debug!(path = %path, id = ?id, "Lua registered command");
                Ok(id.0)
            })?,
        )?;

        // textquest.commands.unregister(command_id) → boolean
        let cmd_unreg = Arc::clone(&self.command_registry);
        commands.set(
            "unregister",
            self.lua.create_function(move |_lua, id: u64| {
                use crate::registry::CommandId;
                let removed = cmd_unreg.lock().unwrap().unregister(CommandId(id));
                Ok(removed)
            })?,
        )?;

        // textquest.commands.dispatch(command_line) → boolean  (test/debug helper)
        let cmd_disp = Arc::clone(&self.command_registry);
        commands.set(
            "dispatch",
            self.lua.create_function(move |_lua, command_line: String| {
                let fired = cmd_disp.lock().unwrap().dispatch(&command_line);
                Ok(fired)
            })?,
        )?;

        parent.set("commands", commands)?;
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn make_bindings() -> LuaBindings {
        let b = LuaBindings::new().expect("create LuaBindings");
        b.register_apis().expect("register APIs");
        b
    }

    // ── Hotkeys ───────────────────────────────────────────────────────────────

    #[test]
    fn lua_register_hotkey_triggers_callback_on_fire() {
        let bindings = make_bindings();
        let fired = Arc::new(AtomicU32::new(0));
        let fired_clone = Arc::clone(&fired);

        // Register a callback from Rust side (simulates what Lua does) directly
        // via the shared registry so we can control the counter.
        let id = bindings
            .hotkey_registry()
            .lock()
            .unwrap()
            .register(
                "ctrl+f9",
                Priority::Script,
                "test_lua",
                Box::new(move || {
                    fired_clone.fetch_add(1, Ordering::Relaxed);
                }),
            );

        assert!(bindings.hotkey_registry().lock().unwrap().fire("ctrl+f9"));
        assert_eq!(fired.load(Ordering::Relaxed), 1);
        assert!(bindings.hotkey_registry().lock().unwrap().unregister(id));
        assert!(!bindings.hotkey_registry().lock().unwrap().fire("ctrl+f9"));
    }

    #[test]
    fn lua_register_hotkey_via_api() {
        let bindings = make_bindings();
        let lua = bindings.get_lua();

        // Register a hotkey from Lua and verify it returns a numeric ID.
        let id: u64 = lua
            .load(r#"textquest.hotkeys.register("ctrl+f10", function() end)"#)
            .eval()
            .expect("register_hotkey from Lua");

        assert!(id > 0, "register_hotkey must return a positive id");

        // Fire it via the Lua dispatch helper.
        let fired: bool = lua
            .load(r#"textquest.hotkeys.fire("ctrl+f10")"#)
            .eval()
            .expect("fire hotkey");
        assert!(fired);

        // Unregister and confirm it no longer fires.
        let removed: bool = lua
            .load(format!("textquest.hotkeys.unregister({})", id))
            .eval()
            .expect("unregister hotkey");
        assert!(removed);

        let fired_after: bool = lua
            .load(r#"textquest.hotkeys.fire("ctrl+f10")"#)
            .eval()
            .expect("fire after unregister");
        assert!(!fired_after);
    }

    // ── Commands ──────────────────────────────────────────────────────────────

    #[test]
    fn lua_register_command_routes_slash_cmd_to_lua() {
        let bindings = make_bindings();
        let lua = bindings.get_lua();

        // Register and dispatch from Lua in one chunk.
        let result: bool = lua
            .load(
                r#"
local dispatched = false
local id = textquest.commands.register("/test_cmd", function(args)
    dispatched = true
end)
textquest.commands.dispatch("/test_cmd hello world")
return dispatched
"#,
            )
            .eval()
            .expect("register and dispatch command");

        assert!(result, "Lua command callback must have fired");
    }

    #[test]
    fn lua_register_command_unregister_cleans_up() {
        let bindings = make_bindings();
        let lua = bindings.get_lua();

        let result: bool = lua
            .load(
                r#"
local id = textquest.commands.register("/cleanup_cmd", function() end)
local removed = textquest.commands.unregister(id)
local fired = textquest.commands.dispatch("/cleanup_cmd")
return removed and not fired
"#,
            )
            .eval()
            .expect("unregister then dispatch");

        assert!(result);
    }

    // ── Shared registries ─────────────────────────────────────────────────────

    #[test]
    fn with_registries_shares_state_between_bindings_and_external_caller() {
        let (cmd_reg, hk_reg) = crate::registry::new_shared();

        // Register via the external registry directly.
        let fired = Arc::new(AtomicU32::new(0));
        let f = Arc::clone(&fired);
        cmd_reg.lock().unwrap().register(
            "/shared_cmd",
            Priority::BuiltIn,
            "builtin",
            Box::new(move |_| {
                f.fetch_add(1, Ordering::Relaxed);
            }),
        );

        // Create bindings that share those registries.
        let bindings =
            LuaBindings::with_registries(Arc::clone(&cmd_reg), Arc::clone(&hk_reg))
                .expect("with_registries");
        bindings.register_apis().expect("register APIs");

        // Dispatch via the Lua API — should reach the externally-registered handler.
        let _: bool = bindings
            .get_lua()
            .load(r#"textquest.commands.dispatch("/shared_cmd")"#)
            .eval()
            .expect("dispatch shared");

        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }
}