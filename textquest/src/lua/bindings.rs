//! Lua API bindings for TextQuest.
//!
//! This module registers TextQuest core APIs with the Lua runtime, organized by domain:
//! - player: textquest.player.*
//! - group: textquest.group.*
//! - nav: textquest.nav.*
//! - combat: textquest.combat.*
//! - state: textquest.state.*
/// - config: textquest.config.*
/// - ipc: textquest.ipc.*
/// - debug: textquest.debug.*
/// - log: textquest.log.*
/// - hotkeys: textquest.hotkeys.*
/// - commands: textquest.commands.*
use mlua::{
    Error as LuaError, Function, Lua, LuaOptions, RegistryKey, Result as LuaResult, Table, Value,
    Variadic,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex, RwLock};

use crate::lua::error::LuaApiError;
use crate::lua::sandbox;
use crate::lua::types::{
    LuaCommandRequest, LuaNavigationRequest, LuaPlayerSnapshot, LuaRuntimeState, LuaWaypoint,
};
use crate::registry::{Priority, SharedCommandRegistry, SharedHotkeyRegistry};

static LUA_COMMAND_TRACING_ENABLED: AtomicBool = AtomicBool::new(false);

type EventHandlers = Arc<Mutex<HashMap<String, Vec<RegistryKey>>>>;

pub struct LuaBindings {
    lua: Lua,
    sandbox_instruction_counter: Arc<AtomicU64>,
    runtime_state: Arc<RwLock<LuaRuntimeState>>,
    event_handlers: EventHandlers,
    /// Shared command registry — injected so plugins and Lua share the same table.
    command_registry: SharedCommandRegistry,
    /// Shared hotkey registry — injected so plugins and Lua share the same table.
    hotkey_registry: SharedHotkeyRegistry,
}

/// Construct a sandboxed `Lua` VM (safe libs only, memory + CPU limits applied).
fn make_sandboxed_lua() -> Result<(Lua, Arc<AtomicU64>), LuaApiError> {
    let lua = Lua::new_with(sandbox::sandbox_libs(), LuaOptions::default())
        .map_err(|e| LuaApiError::BindingError(e.to_string()))?;
    let instruction_counter =
        sandbox::apply(&lua).map_err(|e| LuaApiError::BindingError(e.to_string()))?;
    Ok((lua, instruction_counter))
}

impl LuaBindings {
    /// Create bindings with freshly-created private registries.
    ///
    /// The Lua VM is sandboxed: only safe standard libraries are loaded, dangerous
    /// globals are removed, and memory / CPU limits are enforced.
    pub fn new() -> Result<Self, LuaApiError> {
        let (lua, sandbox_instruction_counter) = make_sandboxed_lua()?;
        let (command_registry, hotkey_registry) = crate::registry::new_shared();
        Ok(Self {
            lua,
            sandbox_instruction_counter,
            runtime_state: Arc::new(RwLock::new(LuaRuntimeState::default())),
            event_handlers: Arc::new(Mutex::new(HashMap::new())),
            command_registry,
            hotkey_registry,
        })
    }

    /// Create bindings that share existing registries (e.g. with the plugin loader).
    ///
    /// The Lua VM is sandboxed identically to [`LuaBindings::new`].
    pub fn with_registries(
        command_registry: SharedCommandRegistry,
        hotkey_registry: SharedHotkeyRegistry,
    ) -> Result<Self, LuaApiError> {
        let (lua, sandbox_instruction_counter) = make_sandboxed_lua()?;
        Ok(Self {
            lua,
            sandbox_instruction_counter,
            runtime_state: Arc::new(RwLock::new(LuaRuntimeState::default())),
            event_handlers: Arc::new(Mutex::new(HashMap::new())),
            command_registry,
            hotkey_registry,
        })
    }

    /// Return a clone of the shared command registry handle.
    pub fn command_registry(&self) -> SharedCommandRegistry {
        Arc::clone(&self.command_registry)
    }

    /// Return a clone of the shared hotkey registry handle.
    pub fn hotkey_registry(&self) -> SharedHotkeyRegistry {
        Arc::clone(&self.hotkey_registry)
    }

    /// Replace the player snapshot used by `textquest.player.*` calls.
    pub fn set_player_snapshot(&self, player: LuaPlayerSnapshot) {
        self.runtime_state
            .write()
            .expect("lua runtime_state lock poisoned")
            .player = Some(player);
    }

    /// Clear the player snapshot; Lua player getters fall back to zero values.
    pub fn clear_player_snapshot(&self) {
        self.runtime_state
            .write()
            .expect("lua runtime_state lock poisoned")
            .player = None;
    }

    pub fn player_snapshot(&self) -> Option<LuaPlayerSnapshot> {
        self.runtime_state
            .read()
            .expect("lua runtime_state lock poisoned")
            .player
            .clone()
    }

    /// Drain queued navigation requests produced by Lua scripts.
    pub fn drain_navigation_requests(&self) -> Vec<LuaNavigationRequest> {
        std::mem::take(
            &mut self
                .runtime_state
                .write()
                .expect("lua runtime_state lock poisoned")
                .navigation_requests,
        )
    }

    /// Drain queued slash-command requests produced by Lua scripts.
    pub fn drain_command_requests(&self) -> Vec<LuaCommandRequest> {
        std::mem::take(
            &mut self
                .runtime_state
                .write()
                .expect("lua runtime_state lock poisoned")
                .command_requests,
        )
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
        self.register_ipc_api(&textquest)?;
        self.register_debug_api(&textquest)?;
        self.register_log_api(&textquest)?;
        self.register_events_api(&textquest)?;
        self.register_hotkeys_api(&textquest)?;
        self.register_commands_api(&textquest)?;
        self.register_issue_791_compat_api(&textquest)?;

        globals.set("textquest", textquest.clone())?;
        self.register_textquest_require(&globals, &textquest)?;

        Ok(())
    }

    fn register_textquest_require(&self, globals: &Table, textquest: &Table) -> LuaResult<()> {
        let textquest_module = textquest.clone();
        globals.set(
            "require",
            self.lua.create_function(move |_, module: String| {
                if module == "textquest" {
                    Ok(textquest_module.clone())
                } else {
                    Err(LuaError::RuntimeError(format!(
                        "sandbox: require('{module}') not allowed"
                    )))
                }
            })?,
        )?;

        Ok(())
    }

    fn register_issue_791_compat_api(&self, parent: &Table) -> LuaResult<()> {
        parent.set(
            "get_spawns",
            self.lua.create_function(|lua, ()| lua.create_table())?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        parent.set(
            "get_player",
            self.lua.create_function(move |lua, ()| {
                player_snapshot_table(lua, read_player_snapshot(&runtime_state))
            })?,
        )?;
        parent.set(
            "cast_spell",
            self.lua.create_function(|_, spell: String| {
                tracing::debug!(spell = %spell, "lua cast_spell requested");
                Ok(true)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        parent.set(
            "move_to",
            self.lua
                .create_function(move |_, (x, y, z): (f32, f32, f32)| {
                    queue_navigation_request(
                        &runtime_state,
                        LuaNavigationRequest::Goto { x, y, z },
                    );
                    tracing::debug!(x, y, z, "lua move_to requested");
                    Ok(true)
                })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        parent.set(
            "execute_command",
            self.lua.create_function(move |_, command: String| {
                queue_command_request(&runtime_state, command.clone(), None, false);
                tracing::debug!(command = %command, "lua execute_command requested");
                Ok(true)
            })?,
        )?;
        parent.set(
            "set_camp_state",
            self.lua.create_function(|_, state: String| {
                tracing::debug!(state = %state, "lua set_camp_state requested");
                Ok(true)
            })?,
        )?;

        let camp_config = self.lua.create_table()?;
        camp_config.set(
            "get",
            self.lua.create_function(|_, args: Variadic<Value>| {
                let args: Vec<Value> = args.into_iter().collect();
                let key = method_string_arg(&args, 0).unwrap_or_default();
                tracing::debug!(key = %key, "lua camp_config.get requested");
                Ok(Value::Nil)
            })?,
        )?;
        camp_config.set(
            "set",
            self.lua.create_function(|_, args: Variadic<Value>| {
                let args: Vec<Value> = args.into_iter().collect();
                let key = method_string_arg(&args, 0).unwrap_or_default();
                let value = method_arg(&args, 1).cloned().unwrap_or(Value::Nil);
                tracing::debug!(key = %key, value = ?value, "lua camp_config.set requested");
                Ok(true)
            })?,
        )?;
        parent.set("camp_config", camp_config)?;

        self.make_log_table_callable(parent)?;

        Ok(())
    }

    fn make_log_table_callable(&self, parent: &Table) -> LuaResult<()> {
        let log: Table = parent.get("log")?;
        let metatable = self.lua.create_table()?;
        metatable.set(
            "__call",
            self.lua.create_function(|_, args: Variadic<Value>| {
                let args: Vec<Value> = args.into_iter().collect();
                let level = method_string_arg(&args, 0).unwrap_or_else(|| "info".to_string());
                let message = method_string_arg(&args, 1).unwrap_or_default();
                trace_lua_log(&level, &message);
                Ok(())
            })?,
        )?;
        log.set_metatable(Some(metatable))?;

        Ok(())
    }

    fn register_player_api(&self, parent: &Table) -> LuaResult<()> {
        let player = self.lua.create_table()?;

        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_hp",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.hp)
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_hp_percent",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.hp_percent())
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_mana",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.mana)
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_mana_percent",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.mana_percent())
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_endurance",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.endurance)
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_endurance_percent",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.endurance_percent())
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_name",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.name)
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_level",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.level)
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_class",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.class_name)
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_class_id",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.class_id)
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_race_id",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.race_id)
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_x",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.x)
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_y",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.y)
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_z",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.z)
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_heading",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.heading)
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "get_speed",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.speed)
                    .unwrap_or_default())
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "is_moving",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.is_moving())
                    .unwrap_or(false))
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "is_feigned",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.is_feigned)
                    .unwrap_or(false))
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "is_dead",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.is_dead)
                    .unwrap_or(false))
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        player.set(
            "is_gm",
            self.lua.create_function(move |_, ()| {
                Ok(read_player_snapshot(&runtime_state)
                    .map(|player| player.is_gm)
                    .unwrap_or(false))
            })?,
        )?;

        parent.set("player", player)?;

        Ok(())
    }

    fn register_group_api(&self, parent: &Table) -> LuaResult<()> {
        let group = self.lua.create_table()?;
        let runtime_state = Arc::clone(&self.runtime_state);

        group.set(
            "get_member_count",
            self.lua.create_function(move |_, ()| {
                Ok(read_group_member_count(&runtime_state))
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        group.set(
            "get_member",
            self.lua.create_function(move |lua, index: usize| {
                group_member_at(lua, index, &runtime_state)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        group.set(
            "get_members",
            self.lua
                .create_function(move |lua, ()| group_members_table(lua, &runtime_state))?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        group.set(
            "get_tank",
            self.lua.create_function(move |_, ()| {
                Ok(read_group_role(&runtime_state, "tank"))
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        group.set(
            "get_assist",
            self.lua.create_function(move |_, ()| {
                Ok(read_group_role(&runtime_state, "assist"))
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        group.set(
            "get_master",
            self.lua.create_function(move |_, ()| {
                Ok(read_group_role(&runtime_state, "master"))
            })?,
        )?;

        parent.set("group", group)?;

        Ok(())
    }

    fn register_nav_api(&self, parent: &Table) -> LuaResult<()> {
        let nav = self.lua.create_table()?;

        let runtime_state = Arc::clone(&self.runtime_state);
        nav.set(
            "goto",
            self.lua
                .create_function(move |_, (x, y, z): (f32, f32, f32)| {
                    queue_navigation_request(
                        &runtime_state,
                        LuaNavigationRequest::Goto { x, y, z },
                    );
                    tracing::debug!("nav.goto({}, {}, {})", x, y, z);
                    Ok(true)
                })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        nav.set(
            "stick",
            self.lua.create_function(move |_, target: String| {
                queue_navigation_request(
                    &runtime_state,
                    LuaNavigationRequest::Stick {
                        target: target.clone(),
                    },
                );
                tracing::debug!("nav.stick(\"{}\")", target);
                Ok(true)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        nav.set(
            "stop",
            self.lua.create_function(move |_, ()| {
                queue_navigation_request(&runtime_state, LuaNavigationRequest::Stop);
                tracing::debug!("nav.stop()");
                Ok(true)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        nav.set(
            "follow",
            self.lua.create_function(move |_, target: String| {
                queue_navigation_request(
                    &runtime_state,
                    LuaNavigationRequest::Follow {
                        target: target.clone(),
                    },
                );
                tracing::debug!("nav.follow(\"{}\")", target);
                Ok(true)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        nav.set(
            "add_waypoint",
            self.lua
                .create_function(move |_, (x, y, z, name): (f32, f32, f32, String)| {
                    if name.trim().is_empty() {
                        return Err(LuaError::RuntimeError(
                            "nav.add_waypoint requires a non-empty name".to_string(),
                        ));
                    }
                    queue_navigation_request(
                        &runtime_state,
                        LuaNavigationRequest::AddWaypoint {
                            x,
                            y,
                            z,
                            name: name.clone(),
                        },
                    );
                    tracing::debug!("nav.add_waypoint({}, {}, {}, \"{}\")", x, y, z, name);
                    Ok(true)
                })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        nav.set(
            "clear_waypoints",
            self.lua.create_function(move |_, ()| {
                queue_navigation_request(&runtime_state, LuaNavigationRequest::ClearWaypoints);
                tracing::debug!("nav.clear_waypoints()");
                Ok(true)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        nav.set(
            "get_waypoints",
            self.lua
                .create_function(move |lua, ()| navigation_waypoint_table(lua, &runtime_state))?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        nav.set(
            "is_stuck",
            self.lua.create_function(move |_, ()| {
                Ok(runtime_state
                    .read()
                    .expect("lua runtime_state lock poisoned")
                    .navigation_is_stuck)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        nav.set(
            "stuck_reason",
            self.lua.create_function(move |_, ()| {
                let state = runtime_state
                    .read()
                    .expect("lua runtime_state lock poisoned");
                Ok(state.navigation_stuck_reason.clone())
            })?,
        )?;

        parent.set("nav", nav)?;

        Ok(())
    }

    fn register_combat_api(&self, parent: &Table) -> LuaResult<()> {
        let combat = self.lua.create_table()?;

        let runtime_state = Arc::clone(&self.runtime_state);
        combat.set(
            "cast",
            self.lua.create_function(move |_, (spell, target): (String, Option<String>)| {
                let spell = spell.trim().to_string();
                if spell.is_empty() {
                    return Err(LuaError::RuntimeError(
                        "combat.cast requires a non-empty spell name".to_string(),
                    ));
                }
                let command = target
                    .as_ref()
                    .filter(|value| !value.trim().is_empty())
                    .map(|target| {
                        format!(
                            "/cast {} {}",
                            lua_string_arg(&spell),
                            lua_string_arg(target.as_str())
                        )
                    })
                    .unwrap_or_else(|| format!("/cast {}", lua_string_arg(&spell)));
                queue_combat_command(&runtime_state, command);
                tracing::debug!("combat.cast(\"{:?}\", {:?})", spell, target);
                Ok(true)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        combat.set(
            "assist",
            self.lua.create_function(move |_, target: Option<String>| {
                let command = target
                    .as_ref()
                    .filter(|value| !value.trim().is_empty())
                    .map(|target| format!("/assist {}", lua_string_arg(target.as_str())))
                    .unwrap_or_else(|| "/assist".to_string());
                queue_combat_command(&runtime_state, command);
                tracing::debug!("combat.assist({:?})", target);
                Ok(true)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        combat.set(
            "attack",
            self.lua.create_function(move |_, target: Option<String>| {
                let command = target
                    .as_ref()
                    .filter(|value| !value.trim().is_empty())
                    .map(|target| format!("/attack {}", lua_string_arg(target.as_str())))
                    .unwrap_or_else(|| "/attack".to_string());
                queue_combat_command(&runtime_state, command);
                tracing::debug!("combat.attack({:?})", target);
                Ok(true)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        combat.set(
            "disengage",
            self.lua.create_function(move |_, ()| {
                queue_combat_command(&runtime_state, "/disengage".to_string());
                tracing::debug!("combat.disengage()");
                Ok(true)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        combat.set(
            "re mez",
            self.lua.create_function(move |_, target: String| {
                let target = target.trim().to_string();
                if target.is_empty() {
                    return Err(LuaError::RuntimeError(
                        "combat.re_mez requires a target name".to_string(),
                    ));
                }
                queue_combat_command(
                    &runtime_state,
                    format!("/remez {}", lua_string_arg(target.as_str())),
                );
                tracing::debug!("combat.re_mez(\"{}\")", target);
                Ok(true)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        combat.set(
            "re_mez",
            self.lua.create_function(move |_, target: String| {
                let target = target.trim().to_string();
                if target.is_empty() {
                    return Err(LuaError::RuntimeError(
                        "combat.re_mez requires a target name".to_string(),
                    ));
                }
                queue_combat_command(
                    &runtime_state,
                    format!("/remez {}", lua_string_arg(target.as_str())),
                );
                tracing::debug!("combat.re_mez(\"{}\")", target);
                Ok(true)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        combat.set(
            "rezz",
            self.lua.create_function(move |_, target: Option<String>| {
                let command = target
                    .as_ref()
                    .filter(|value| !value.trim().is_empty())
                    .map(|target| format!("/rezz {}", lua_string_arg(target.as_str())))
                    .unwrap_or_else(|| "/rezz".to_string());
                queue_combat_command(&runtime_state, command);
                tracing::debug!("combat.rezz({:?})", target);
                Ok(true)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        combat.set(
            "set_target",
            self.lua.create_function(move |_, target: String| {
                let target = target.trim().to_string();
                if target.is_empty() {
                    return Err(LuaError::RuntimeError(
                        "combat.set_target requires a target name".to_string(),
                    ));
                }
                let target = target.to_lowercase();
                let matched = {
                    let state = runtime_state
                        .read()
                        .expect("lua runtime_state lock poisoned");
                    state
                        .spawns
                        .iter()
                        .find(|spawn| {
                            spawn.name.eq_ignore_ascii_case(&target)
                                || spawn
                                    .displayed_name
                                    .eq_ignore_ascii_case(&target)
                                || spawn.spawn_id.to_string().eq(&target)
                        })
                        .cloned()
                };
                if let Some(found) = matched {
                    runtime_state
                        .write()
                        .expect("lua runtime_state lock poisoned")
                        .target = Some(found);
                    queue_combat_command(
                        &runtime_state,
                        format!("/target {}", lua_string_arg(target.as_str())),
                    );
                    Ok(true)
                } else {
                    Err(LuaError::RuntimeError(format!(
                        "combat.set_target not found: {target}"
                    )))
                }
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        combat.set(
            "get_buffs",
            self.lua.create_function(move |lua, ()| {
                let buffs = runtime_state
                    .read()
                    .expect("lua runtime_state lock poisoned")
                    .buffs
                    .clone();
                lua_create_string_array(lua, &buffs)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        combat.set(
            "get_debuffs",
            self.lua.create_function(move |lua, ()| {
                let debuffs = runtime_state
                    .read()
                    .expect("lua runtime_state lock poisoned")
                    .debuffs
                    .clone();
                lua_create_string_array(lua, &debuffs)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        combat.set(
            "has_buff",
            self.lua.create_function(move |_, name: String| {
                Ok(has_buff_state(&runtime_state, &name))
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        combat.set(
            "has_debuff",
            self.lua.create_function(move |_, name: String| {
                Ok(has_debuff_state(&runtime_state, &name))
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        combat.set(
            "get_target",
            self.lua.create_function(move |lua, ()| {
                state_target_table(lua, &runtime_state)
            })?,
        )?;

        parent.set("combat", combat)?;

        Ok(())
    }

    fn register_state_api(&self, parent: &Table) -> LuaResult<()> {
        let state = self.lua.create_table()?;

        let runtime_state = Arc::clone(&self.runtime_state);
        state.set(
            "get_spawns",
            self.lua
                .create_function(move |lua, ()| state_spawns_table(lua, &runtime_state))?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        state.set(
            "get_spawn",
            self.lua
                .create_function(move |lua, name: String| {
                    state_get_spawn(lua, &runtime_state, &name)
                })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        state.set(
            "find_spawns",
            self.lua
                .create_function(move |lua, filter: String| {
                    state_find_spawns(lua, &runtime_state, &filter)
                })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        state.set(
            "get_target",
            self.lua
                .create_function(move |lua, ()| state_target_table(lua, &runtime_state))?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        state.set(
            "set_target",
            self.lua.create_function(move |_, target: String| {
                if target.trim().is_empty() {
                    return Err(LuaError::RuntimeError(
                        "state.set_target requires a target name".to_string(),
                    ));
                }
                let target = target.to_lowercase();
                let matched = {
                    let state = runtime_state
                        .read()
                        .expect("lua runtime_state lock poisoned");
                    state
                        .spawns
                        .iter()
                        .find(|spawn| {
                            spawn.name.eq_ignore_ascii_case(&target)
                                || spawn.displayed_name.eq_ignore_ascii_case(&target)
                                || spawn.spawn_id.to_string().eq(&target)
                        })
                        .cloned()
                };
                if let Some(found) = matched {
                    runtime_state
                        .write()
                        .expect("lua runtime_state lock poisoned")
                        .target = Some(found);
                    Ok(true)
                } else {
                    Ok(false)
                }
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        state.set(
            "get_xtargets",
            self.lua
                .create_function(move |lua, ()| state_xtargets_table(lua, &runtime_state))?,
        )?;

        parent.set("state", state)?;

        Ok(())
    }

    fn register_config_api(&self, parent: &Table) -> LuaResult<()> {
        let config = self.lua.create_table()?;
        let runtime_state = Arc::clone(&self.runtime_state);

        config.set(
            "get",
            self.lua.create_function(move |lua, key: String| {
                runtime_state
                    .read()
                    .expect("lua runtime_state lock poisoned")
                    .plugin_config
                    .get(&key)
                    .map_or(Ok(mlua::Value::Nil), |value| {
                        Ok(mlua::Value::String(lua.create_string(value)?))
                    })
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        config.set(
            "set",
            self.lua.create_function(move |_, (key, value): (String, Value)| {
                let value = lua_value_to_string(&value);
                runtime_state
                    .write()
                    .expect("lua runtime_state lock poisoned")
                    .plugin_config
                    .insert(key, value);
                Ok(true)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        config.set(
            "save",
            self.lua.create_function(move |_, ()| {
                runtime_state
                    .write()
                    .expect("lua runtime_state lock poisoned")
                    .last_saved_config = true;
                tracing::debug!("config.save()");
                Ok(true)
            })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        config.set(
            "reload",
            self.lua.create_function(move |_, ()| {
                runtime_state
                    .write()
                    .expect("lua runtime_state lock poisoned")
                    .last_reloaded_config = true;
                tracing::debug!("config.reload()");
                Ok(true)
            })?,
        )?;

        parent.set("config", config)?;

        Ok(())
    }

    fn register_ipc_api(&self, parent: &Table) -> LuaResult<()> {
        let ipc = self.lua.create_table()?;
        let runtime_state = Arc::clone(&self.runtime_state);

        ipc.set(
            "send",
            self.lua
                .create_function(move |_, (command, target_box): (String, String)| {
                    if command.trim().is_empty() {
                        return Err(LuaError::RuntimeError(
                            "ipc.send requires a non-empty command".to_string(),
                        ));
                    }
                    queue_command_request(&runtime_state, command, Some(target_box), true);
                    Ok(true)
                })?,
        )?;
        let runtime_state = Arc::clone(&self.runtime_state);
        ipc.set(
            "broadcast",
            self.lua.create_function(move |_, command: String| {
                if command.trim().is_empty() {
                    return Err(LuaError::RuntimeError(
                        "ipc.broadcast requires a non-empty command".to_string(),
                    ));
                }
                queue_command_request(&runtime_state, command, None, true);
                Ok(true)
            })?,
        )?;

        parent.set("ipc", ipc)?;

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

        let handlers = Arc::clone(&self.event_handlers);
        events.set(
            "on",
            self.lua
                .create_function(move |lua, (event, callback): (String, Function)| {
                    let callback = lua.create_registry_value(callback)?;
                    let mut handlers = handlers.lock().expect("lua event_handlers lock poisoned");
                    let callbacks = handlers.entry(event.clone()).or_default();
                    callbacks.push(callback);
                    tracing::debug!(
                        event = %event,
                        callbacks = callbacks.len(),
                        "events.on registered Lua callback"
                    );
                    Ok(callbacks.len())
                })?,
        )?;
        let handlers = Arc::clone(&self.event_handlers);
        events.set(
            "off",
            self.lua.create_function(move |_, event: String| {
                let removed = handlers
                    .lock()
                    .expect("lua event_handlers lock poisoned")
                    .remove(&event)
                    .map(|callbacks| callbacks.len())
                    .unwrap_or_default();
                tracing::debug!(event = %event, removed, "events.off removed Lua callbacks");
                Ok(removed)
            })?,
        )?;
        let handlers = Arc::clone(&self.event_handlers);
        events.set(
            "emit",
            self.lua
                .create_function(move |lua, (event, data): (String, Value)| {
                    let callback_count = emit_lua_event(lua, &handlers, &event, data)?;
                    tracing::debug!(
                        event = %event,
                        callbacks = callback_count,
                        "events.emit dispatched Lua callbacks"
                    );
                    Ok(callback_count)
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
            self.lua.create_function(
                move |_lua, (combo, callback): (String, mlua::Function)| {
                    let callback = callback.clone();
                    let id = hk_reg.lock().unwrap().register(
                        &combo,
                        Priority::Script,
                        "lua",
                        Box::new(move || {
                            if let Err(e) = callback.call::<()>(()) {
                                tracing::warn!(error = %e, "Lua hotkey callback error");
                            }
                        }),
                    );
                    tracing::debug!(combo = %combo, id = ?id, "Lua registered hotkey");
                    Ok(id.as_raw())
                },
            )?,
        )?;

        // textquest.hotkeys.unregister(hotkey_id) → boolean
        let hk_unreg = Arc::clone(&self.hotkey_registry);
        hotkeys.set(
            "unregister",
            self.lua.create_function(move |_lua, id: u64| {
                use crate::registry::ScriptHotkeyId;
                let removed = hk_unreg
                    .lock()
                    .unwrap()
                    .unregister(ScriptHotkeyId::from_raw(id));
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

    fn register_debug_api(&self, parent: &Table) -> LuaResult<()> {
        let debug = self.lua.create_table()?;

        debug.set(
            "hex_dump",
            self.lua.create_function(|_, args: Variadic<Value>| {
                let args: Vec<Value> = args.into_iter().collect();
                let address = method_usize_arg(&args, 0, "address")?;
                let size = method_optional_usize_arg(&args, 1, "size")?
                    .unwrap_or(64)
                    .min(4096);
                if size == 0 {
                    return Err(LuaError::RuntimeError(
                        "debug.hex_dump size must be greater than zero".to_string(),
                    ));
                }

                tracing::debug!(
                    address = format_args!("{address:#x}"),
                    size,
                    "lua debug hex_dump requested"
                );
                Ok(format_debug_hex_dump_request(address, size))
            })?,
        )?;
        debug.set(
            "enable_command_tracing",
            self.lua.create_function(|_, ()| {
                LUA_COMMAND_TRACING_ENABLED.store(true, std::sync::atomic::Ordering::Relaxed);
                tracing::debug!("lua debug command tracing enabled");
                Ok(true)
            })?,
        )?;
        debug.set(
            "command_tracing_enabled",
            self.lua.create_function(|_, ()| {
                Ok(LUA_COMMAND_TRACING_ENABLED.load(std::sync::atomic::Ordering::Relaxed))
            })?,
        )?;
        debug.set(
            "inspect_camp_state",
            self.lua.create_function(|lua, ()| {
                let state = lua.create_table()?;
                state.set("status", "unavailable")?;
                state.set("ipc_queue_depth", 0u32)?;
                state.set("scripts", lua.create_table()?)?;
                state.set("plugins", lua.create_table()?)?;
                state.set(
                    "command_tracing_enabled",
                    LUA_COMMAND_TRACING_ENABLED.load(std::sync::atomic::Ordering::Relaxed),
                )?;
                Ok(state)
            })?,
        )?;

        parent.set("debug", debug)?;
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
            self.lua
                .create_function(move |_lua, (path, callback): (String, mlua::Function)| {
                    let callback = callback.clone();
                    let id = cmd_reg.lock().unwrap().register(
                        &path,
                        Priority::Script,
                        "lua",
                        Box::new(move |tail: &str| {
                            if let Err(e) = callback.call::<()>(tail.to_string()) {
                                tracing::warn!(error = %e, "Lua command callback error");
                            }
                        }),
                    );
                    tracing::debug!(path = %path, id = ?id, "Lua registered command");
                    Ok(id.as_raw())
                })?,
        )?;

        // textquest.commands.unregister(command_id) → boolean
        let cmd_unreg = Arc::clone(&self.command_registry);
        commands.set(
            "unregister",
            self.lua.create_function(move |_lua, id: u64| {
                use crate::registry::CommandId;
                let removed = cmd_unreg
                    .lock()
                    .unwrap()
                    .unregister(CommandId::from_raw(id));
                Ok(removed)
            })?,
        )?;

        // textquest.commands.dispatch(command_line) → boolean  (test/debug helper)
        let cmd_disp = Arc::clone(&self.command_registry);
        commands.set(
            "dispatch",
            self.lua
                .create_function(move |_lua, command_line: String| {
                    let fired = cmd_disp.lock().unwrap().dispatch(&command_line);
                    Ok(fired)
                })?,
        )?;

        // textquest.commands.execute(command_line) → boolean. The orchestrator can
        // drain these requests and deliver them to the live client IPC surface.
        let runtime_state = Arc::clone(&self.runtime_state);
        commands.set(
            "execute",
            self.lua
                .create_function(move |_lua, command_line: String| {
                    queue_command_request(&runtime_state, command_line, None, false);
                    Ok(true)
                })?,
        )?;

        parent.set("commands", commands)?;
        Ok(())
    }

    pub fn get_lua(&self) -> &Lua {
        &self.lua
    }

    /// Reset sandbox CPU budget before a top-level script invocation.
    pub fn reset_sandbox_instruction_counter(&self) {
        sandbox::reset_instruction_counter(&self.sandbox_instruction_counter);
    }

    /// Emit a TextQuest event into registered Lua callbacks.
    pub fn emit_event(&self, event: &str, data: Value) -> LuaResult<usize> {
        self.reset_sandbox_instruction_counter();
        emit_lua_event(&self.lua, &self.event_handlers, event, data)
    }
}

impl Default for LuaBindings {
    fn default() -> Self {
        Self::new().expect("Failed to create Lua context")
    }
}

fn method_arg(args: &[Value], index: usize) -> Option<&Value> {
    let offset = if matches!(args.first(), Some(Value::Table(_))) {
        1
    } else {
        0
    };
    args.get(offset + index)
}

fn method_string_arg(args: &[Value], index: usize) -> Option<String> {
    match method_arg(args, index)? {
        Value::String(value) => Some(value.to_string_lossy()),
        _ => None,
    }
}

fn method_usize_arg(args: &[Value], index: usize, name: &str) -> LuaResult<usize> {
    method_optional_usize_arg(args, index, name)?.ok_or_else(|| {
        LuaError::RuntimeError(format!("debug.hex_dump missing required {name} argument"))
    })
}

fn method_optional_usize_arg(args: &[Value], index: usize, name: &str) -> LuaResult<Option<usize>> {
    let Some(value) = method_arg(args, index) else {
        return Ok(None);
    };

    match value {
        Value::Integer(value) if *value >= 0 => Ok(Some(*value as usize)),
        Value::Number(value) if value.is_finite() && *value >= 0.0 && value.fract() == 0.0 => {
            Ok(Some(*value as usize))
        }
        Value::String(value) => parse_lua_usize(&value.to_string_lossy())
            .map(Some)
            .map_err(|_| LuaError::RuntimeError(format!("debug.hex_dump invalid {name}"))),
        _ => Err(LuaError::RuntimeError(format!(
            "debug.hex_dump {name} must be a non-negative integer"
        ))),
    }
}

fn parse_lua_usize(raw: &str) -> Result<usize, std::num::ParseIntError> {
    let trimmed = raw.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        usize::from_str_radix(hex, 16)
    } else {
        trimmed.parse::<usize>()
    }
}

fn format_debug_hex_dump_request(address: usize, size: usize) -> String {
    format!("hex_dump address={address:#x} size={size} backend=MemoryRead")
}

fn trace_lua_log(level: &str, message: &str) {
    match level {
        "debug" => tracing::debug!("[Lua] {}", message),
        "warn" | "warning" => tracing::warn!("[Lua] {}", message),
        "error" => tracing::error!("[Lua] {}", message),
        _ => tracing::info!("[Lua] {}", message),
    }
}

fn read_player_snapshot(state: &Arc<RwLock<LuaRuntimeState>>) -> Option<LuaPlayerSnapshot> {
    state
        .read()
        .expect("lua runtime_state lock poisoned")
        .player
        .clone()
}

fn player_snapshot_table(lua: &Lua, player: Option<LuaPlayerSnapshot>) -> LuaResult<Table> {
    let player = player.unwrap_or_default();
    let hp_percent = player.hp_percent();
    let mana_percent = player.mana_percent();
    let endurance_percent = player.endurance_percent();
    let is_moving = player.is_moving();
    let table = lua.create_table()?;
    table.set("name", player.name)?;
    table.set("level", player.level)?;
    table.set("class", player.class_name)?;
    table.set("class_id", player.class_id)?;
    table.set("race_id", player.race_id)?;
    table.set("hp", player.hp)?;
    table.set("hp_percent", hp_percent)?;
    table.set("mana", player.mana)?;
    table.set("mana_percent", mana_percent)?;
    table.set("endurance", player.endurance)?;
    table.set("endurance_percent", endurance_percent)?;
    table.set("x", player.x)?;
    table.set("y", player.y)?;
    table.set("z", player.z)?;
    table.set("heading", player.heading)?;
    table.set("speed", player.speed)?;
    table.set("is_moving", is_moving)?;
    table.set("is_feigned", player.is_feigned)?;
    table.set("is_dead", player.is_dead)?;
    table.set("is_gm", player.is_gm)?;
    Ok(table)
}

fn lua_create_string_array(lua: &Lua, values: &[String]) -> LuaResult<Table> {
    let table = lua.create_table()?;
    for (index, value) in values.iter().enumerate() {
        table.set(index + 1, value.as_str())?;
    }
    Ok(table)
}

fn lua_string_arg(raw: &str) -> String {
    format!("\"{}\"", raw.replace('\\', "\\\\").replace('\"', "\\\""))
}

fn lua_value_to_string(value: &Value) -> String {
    match value {
        Value::Nil => "nil".to_string(),
        Value::Boolean(v) => v.to_string(),
        Value::Integer(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.to_string_lossy(),
        _ => format!("{value:?}"),
    }
}

fn read_group_member_count(state: &Arc<RwLock<LuaRuntimeState>>) -> usize {
    state
        .read()
        .expect("lua runtime_state lock poisoned")
        .group_members
        .len()
}

fn read_group_role(state: &Arc<RwLock<LuaRuntimeState>>, role: &str) -> Option<String> {
    let state = state.read().expect("lua runtime_state lock poisoned");
    match role {
        "tank" => state.group_tank.clone(),
        "assist" => state.group_assist.clone(),
        "master" => state.group_master.clone(),
        _ => None,
    }
}

fn group_member_at(
    lua: &Lua,
    index: usize,
    state: &Arc<RwLock<LuaRuntimeState>>,
) -> LuaResult<Value> {
    let state = state.read().expect("lua runtime_state lock poisoned");
    match index {
        1..=usize::MAX => {
            state
                .group_members
                .get(index.saturating_sub(1))
                .map_or(Ok(Value::Nil), |name| Ok(Value::String(lua.create_string(name)?)))
        }
        _ => Ok(Value::Nil),
    }
}

fn group_members_table(lua: &Lua, state: &Arc<RwLock<LuaRuntimeState>>) -> LuaResult<Table> {
    let members = state
        .read()
        .expect("lua runtime_state lock poisoned")
        .group_members
        .clone();
    let table = lua.create_table()?;
    for (idx, member) in members.iter().enumerate() {
        table.set(idx + 1, member.as_str())?;
    }
    Ok(table)
}

fn navigation_waypoint_table(
    lua: &Lua,
    state: &Arc<RwLock<LuaRuntimeState>>,
) -> LuaResult<Table> {
    let waypoints = state
        .read()
        .expect("lua runtime_state lock poisoned")
        .waypoints
        .clone();
    let table = lua.create_table()?;
    for (index, waypoint) in waypoints.iter().enumerate() {
        let waypoint_entry = lua.create_table()?;
        waypoint_entry.set("name", waypoint.name.as_str())?;
        waypoint_entry.set("x", waypoint.x)?;
        waypoint_entry.set("y", waypoint.y)?;
        waypoint_entry.set("z", waypoint.z)?;
        table.set(index + 1, waypoint_entry)?;
    }
    Ok(table)
}

fn state_target_table(
    lua: &Lua,
    state: &Arc<RwLock<LuaRuntimeState>>,
) -> LuaResult<Value> {
    state
        .read()
        .expect("lua runtime_state lock poisoned")
        .target
        .as_ref()
        .map_or(Ok(Value::Nil), |target| spawn_to_table(lua, target).map(Value::Table))
}

fn state_spawns_table(lua: &Lua, state: &Arc<RwLock<LuaRuntimeState>>) -> LuaResult<Table> {
    let spawns = state
        .read()
        .expect("lua runtime_state lock poisoned")
        .spawns
        .clone();
    spawn_list_table(lua, &spawns)
}

fn state_xtargets_table(lua: &Lua, state: &Arc<RwLock<LuaRuntimeState>>) -> LuaResult<Table> {
    let xtargets = state
        .read()
        .expect("lua runtime_state lock poisoned")
        .xtargets
        .clone();
    spawn_list_table(lua, &xtargets)
}

fn state_get_spawn(lua: &Lua, state: &Arc<RwLock<LuaRuntimeState>>, name: &str) -> LuaResult<Value> {
    let name = name.to_ascii_lowercase();
    let found = {
        state
            .read()
            .expect("lua runtime_state lock poisoned")
            .spawns
            .iter()
            .find(|spawn| {
                spawn.name.eq_ignore_ascii_case(&name)
                    || spawn.displayed_name.eq_ignore_ascii_case(&name)
                    || spawn.spawn_id.to_string().eq(&name)
            })
            .cloned()
    };
    match found {
        Some(found) => spawn_to_table(lua, &found).map(Value::Table),
        None => Ok(Value::Nil),
    }
}

fn state_find_spawns(
    lua: &Lua,
    state: &Arc<RwLock<LuaRuntimeState>>,
    filter: &str,
) -> LuaResult<Table> {
    let filter = filter.to_ascii_lowercase();
    let spawns = {
        let state = state.read().expect("lua runtime_state lock poisoned");
        state
            .spawns
            .iter()
            .filter(|spawn| {
                spawn.name.to_ascii_lowercase().contains(&filter)
                    || spawn
                        .displayed_name
                        .to_ascii_lowercase()
                        .contains(&filter)
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    spawn_list_table(lua, &spawns)
}

fn has_buff_state(state: &Arc<RwLock<LuaRuntimeState>>, target: &str) -> bool {
    let target = target.to_ascii_lowercase();
    state
        .read()
        .expect("lua runtime_state lock poisoned")
        .buffs
        .iter()
        .any(|value| value.to_ascii_lowercase() == target)
}

fn has_debuff_state(state: &Arc<RwLock<LuaRuntimeState>>, target: &str) -> bool {
    let target = target.to_ascii_lowercase();
    state
        .read()
        .expect("lua runtime_state lock poisoned")
        .debuffs
        .iter()
        .any(|value| value.to_ascii_lowercase() == target)
}

fn spawn_list_table(lua: &Lua, spawns: &[textquest_common::types::SpawnData]) -> LuaResult<Table> {
    let table = lua.create_table()?;
    for (idx, spawn) in spawns.iter().enumerate() {
        table.set(idx + 1, spawn_to_table(lua, spawn)?)?;
    }
    Ok(table)
}

fn spawn_to_table(lua: &Lua, spawn: &textquest_common::types::SpawnData) -> LuaResult<Table> {
    let is_dead = spawn.stand_state == 111 || spawn.stand_state == 120;
    let is_feigned = spawn.stand_state == 110;
    let table = lua.create_table()?;
    table.set("spawn_id", spawn.spawn_id)?;
    table.set("name", spawn.name.as_str())?;
    table.set("displayed_name", spawn.displayed_name.as_str())?;
    table.set("level", spawn.level)?;
    table.set("class_id", spawn.class_id)?;
    table.set("race_id", spawn.race_id)?;
    table.set("class_name", spawn.class_str())?;
    table.set("race_name", spawn.race_name())?;
    table.set("hp", spawn.hp_current)?;
    table.set("hp_max", spawn.hp_max)?;
    table.set("mana", spawn.mana_current)?;
    table.set("mana_max", spawn.mana_max)?;
    table.set("endurance", spawn.endurance_current)?;
    table.set("endurance_max", spawn.endurance_max as i32)?;
    table.set("x", spawn.x)?;
    table.set("y", spawn.y)?;
    table.set("z", spawn.z)?;
    table.set("heading", spawn.heading)?;
    table.set("is_dead", is_dead)?;
    table.set("is_feigned", is_feigned)?;
    table.set("is_gm", spawn.is_gm)?;
    Ok(table)
}

fn queue_navigation_request(state: &Arc<RwLock<LuaRuntimeState>>, request: LuaNavigationRequest) {
    let mut state = state.write().expect("lua runtime_state lock poisoned");
    match &request {
        LuaNavigationRequest::AddWaypoint { x, y, z, name } => state.waypoints.push(LuaWaypoint {
            name: name.clone(),
            x: *x,
            y: *y,
            z: *z,
        }),
        LuaNavigationRequest::ClearWaypoints => state.waypoints.clear(),
        _ => {}
    }
    state.navigation_requests.push(request);
}

fn queue_combat_command(state: &Arc<RwLock<LuaRuntimeState>>, command: String) {
    queue_command_request(state, command, None, false)
}

fn queue_command_request(
    state: &Arc<RwLock<LuaRuntimeState>>,
    command: String,
    target_box: Option<String>,
    via_ipc: bool,
) {
    state
        .write()
        .expect("lua runtime_state lock poisoned")
        .command_requests
        .push(LuaCommandRequest {
            command,
            target_box,
            via_ipc,
        });
}

fn emit_lua_event(
    lua: &Lua,
    handlers: &EventHandlers,
    event: &str,
    data: Value,
) -> LuaResult<usize> {
    let callbacks = {
        let handlers = handlers.lock().expect("lua event_handlers lock poisoned");
        let Some(keys) = handlers.get(event) else {
            return Ok(0);
        };

        let mut callbacks = Vec::with_capacity(keys.len());
        for key in keys {
            callbacks.push(lua.registry_value::<Function>(key)?);
        }
        callbacks
    };

    for callback in &callbacks {
        callback.call::<()>(data.clone())?;
    }

    Ok(callbacks.len())
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
        let id = bindings.hotkey_registry().lock().unwrap().register(
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

    // ── LuaBindings construction ──────────────────────────────────────────────

    #[test]
    fn new_does_not_panic() {
        let _b = LuaBindings::new().expect("LuaBindings::new must not panic");
    }

    #[test]
    fn default_does_not_panic() {
        let _b = LuaBindings::default();
    }

    // ── API bindings: each domain callable from Lua ────────────────────────────

    #[test]
    fn player_api_all_fns_callable() {
        let b = make_bindings();
        let lua = b.get_lua();
        let script = r#"
            local hp = textquest.player.get_hp()
            local hp_pct = textquest.player.get_hp_percent()
            local mana = textquest.player.get_mana()
            local mana_pct = textquest.player.get_mana_percent()
            local end_ = textquest.player.get_endurance()
            local end_pct = textquest.player.get_endurance_percent()
            local name = textquest.player.get_name()
            local lvl = textquest.player.get_level()
            local class = textquest.player.get_class()
            local class_id = textquest.player.get_class_id()
            local race_id = textquest.player.get_race_id()
            local x = textquest.player.get_x()
            local y = textquest.player.get_y()
            local z = textquest.player.get_z()
            local hd = textquest.player.get_heading()
            local spd = textquest.player.get_speed()
            local moving = textquest.player.is_moving()
            local feigned = textquest.player.is_feigned()
            local dead = textquest.player.is_dead()
            local gm = textquest.player.is_gm()
            return true
        "#;
        let ok: bool = lua.load(script).eval().expect("player API callable");
        assert!(ok);
    }

    #[test]
    fn player_api_reads_injected_snapshot() {
        let b = make_bindings();
        b.set_player_snapshot(LuaPlayerSnapshot {
            name: "Frostreaver".to_string(),
            level: 60,
            class_name: "Warrior".to_string(),
            class_id: 1,
            race_id: 1,
            hp: 750,
            hp_max: 1000,
            mana: 25,
            mana_max: 100,
            endurance: 50,
            endurance_max: 100,
            x: 1.5,
            y: 2.5,
            z: 3.5,
            heading: 90.0,
            speed: 4.0,
            is_feigned: false,
            is_dead: false,
            is_gm: false,
        });

        let (name, hp_pct, x, moving): (String, f32, f32, bool) = b
            .get_lua()
            .load(
                r#"
return textquest.player.get_name(),
       textquest.player.get_hp_percent(),
       textquest.player.get_x(),
       textquest.player.is_moving()
"#,
            )
            .eval()
            .expect("read injected player snapshot");

        assert_eq!(name, "Frostreaver");
        assert_eq!(hp_pct, 75.0);
        assert_eq!(x, 1.5);
        assert!(moving);
    }

    #[test]
    fn group_api_all_fns_callable() {
        let b = make_bindings();
        {
            let mut state = b.runtime_state.write().expect("lua runtime_state lock poisoned");
            state.group_members = vec!["Ari".to_string(), "Bex".to_string()];
            state.group_tank = Some("Ari".to_string());
            state.group_assist = Some("Bex".to_string());
            state.group_master = Some("Ari".to_string());
        }
        let lua = b.get_lua();
        let script = r#"
            local cnt = textquest.group.get_member_count()
            local m = textquest.group.get_member(1)
            local members = textquest.group.get_members()
            local tank = textquest.group.get_tank()
            local assist = textquest.group.get_assist()
            local master = textquest.group.get_master()
            return true
        "#;
        let ok: bool = lua.load(script).eval().expect("group API callable");
        assert!(ok);
    }

    #[test]
    fn nav_api_all_fns_callable_using_bracket_for_goto() {
        let b = make_bindings();
        let lua = b.get_lua();
        // `goto` is a Lua 5.2+ reserved keyword — must use bracket notation.
        let script = r#"
            local r1 = textquest.nav['goto'](1.0, 2.0, 3.0)
            local r2 = textquest.nav.stick("target")
            local r3 = textquest.nav.stop()
            local r4 = textquest.nav.follow("leader")
            local r5 = textquest.nav.add_waypoint(0.0, 0.0, 0.0, "wp1")
            local r6 = textquest.nav.clear_waypoints()
            return r1 and r2 and r3 and r4 and r5 and r6
        "#;
        let ok: bool = lua.load(script).eval().expect("nav API callable");
        assert!(ok, "all nav API functions must return true");
    }

    #[test]
    fn nav_api_exposes_waypoints_and_stuck_state() {
        let b = make_bindings();
        {
            let mut state = b.runtime_state.write().expect("lua runtime_state lock poisoned");
            state.waypoints.push(LuaWaypoint {
                name: "camp".to_string(),
                x: 100.0,
                y: 200.0,
                z: 5.0,
            });
            state.navigation_is_stuck = true;
            state.navigation_stuck_reason = Some("blocked by terrain".to_string());
        }

        let (count, first_name, stuck, reason): (u32, String, bool, String) = b
            .get_lua()
            .load(
                r#"
local waypoints = textquest.nav.get_waypoints()
local stuck = textquest.nav.is_stuck()
local reason = textquest.nav.stuck_reason() or ""
return #waypoints, waypoints[1].name, stuck, reason
"#,
            )
            .eval()
            .expect("read nav state");
        assert_eq!(count, 1);
        assert_eq!(first_name, "camp");
        assert!(stuck);
        assert_eq!(reason, "blocked by terrain");
    }

    #[test]
    fn nav_and_command_apis_queue_runtime_requests() {
        let b = make_bindings();
        let lua = b.get_lua();

        lua.load(
            r#"
textquest.nav['goto'](11.0, 22.0, 33.0)
textquest.nav.stop()
textquest.commands.execute("/sit")
textquest.execute_command("/stand")
"#,
        )
        .exec()
        .expect("queue Lua requests");

        assert_eq!(
            b.drain_navigation_requests(),
            vec![
                LuaNavigationRequest::Goto {
                    x: 11.0,
                    y: 22.0,
                    z: 33.0
                },
                LuaNavigationRequest::Stop
            ]
        );
        assert_eq!(
            b.drain_command_requests(),
            vec![
                LuaCommandRequest {
                    command: "/sit".to_string()
                    ,
                    target_box: None,
                    via_ipc: false,
                },
                LuaCommandRequest {
                    command: "/stand".to_string()
                    ,
                    target_box: None,
                    via_ipc: false,
                }
            ]
        );
    }

    #[test]
    fn combat_api_all_fns_callable() {
        let b = make_bindings();
        let lua = b.get_lua();
        {
            let mut state = b.runtime_state.write().expect("lua runtime_state lock poisoned");
            state.spawns = vec![textquest_common::types::SpawnData {
                spawn_id: 11,
                name: "Rathyl".to_string(),
                displayed_name: "a_ratyl".to_string(),
                spawn_type: 1,
                level: 55,
                class_id: 1,
                race_id: 1,
                x: 12.0,
                y: 8.0,
                z: 0.0,
                heading: 90.0,
                hp_current: 10,
                hp_max: 10,
                mana_current: 0,
                mana_max: 0,
                endurance_current: 100,
                endurance_max: 100,
                speed_run: 0.0,
                stand_state: 0,
                is_gm: false,
            }];
            state.buffs = vec!["Rage".to_string(), "Haste".to_string()];
            state.debuffs = vec!["Curse".to_string()];
        }
        let script = r#"
            local r1 = textquest.combat.cast("Fire Bolt", nil)
            local r2 = textquest.combat.assist(nil)
            local r3 = textquest.combat.attack(nil)
            local r4 = textquest.combat.disengage()
            local r5 = textquest.combat.rezz(nil)
            local r6 = textquest.combat.re_mez("Rathyl")
            local r7 = textquest.combat.set_target("Rathyl")
            local has_rage = textquest.combat.has_buff("Rage")
            local has_curse = textquest.combat.has_debuff("Curse")
            local buffs = textquest.combat.get_buffs()
            local debuffs = textquest.combat.get_debuffs()
            local target = textquest.combat.get_target()
            return r1 and r2 and r3 and r4 and r5 and r6 and r7 and has_rage
                and has_curse and #buffs == 2 and #debuffs == 1 and target.name == "a_ratyl"
        "#;
        let ok: bool = lua.load(script).eval().expect("combat API callable");
        assert!(ok);
        assert_eq!(
            b.drain_command_requests(),
            vec![
                LuaCommandRequest {
                    command: "/cast \"Fire Bolt\"".to_string(),
                    target_box: None,
                    via_ipc: false,
                },
                LuaCommandRequest {
                    command: "/assist".to_string(),
                    target_box: None,
                    via_ipc: false,
                },
                LuaCommandRequest {
                    command: "/attack".to_string(),
                    target_box: None,
                    via_ipc: false,
                },
                LuaCommandRequest {
                    command: "/disengage".to_string(),
                    target_box: None,
                    via_ipc: false,
                },
                LuaCommandRequest {
                    command: "/rezz".to_string(),
                    target_box: None,
                    via_ipc: false,
                },
                LuaCommandRequest {
                    command: "/remez \"Rathyl\"".to_string(),
                    target_box: None,
                    via_ipc: false,
                },
                LuaCommandRequest {
                    command: "/target \"rathyl\"".to_string(),
                    target_box: None,
                    via_ipc: false,
                },
            ]
        );
    }

    #[test]
    fn combat_cast_requires_spell_name() {
        let b = make_bindings();
        let result: mlua::Result<bool> = b
            .get_lua()
            .load(r#"textquest.combat.cast("", nil)"#)
            .eval();
        assert!(result.is_err(), "combat.cast requires a non-empty spell");
    }

    #[test]
    fn state_api_all_fns_callable() {
        let b = make_bindings();
        {
            let mut state = b.runtime_state.write().expect("lua runtime_state lock poisoned");
            state.spawns = vec![
                textquest_common::types::SpawnData {
                    spawn_id: 1,
                    name: "Rathyl".to_string(),
                    displayed_name: "a_ratyl".to_string(),
                    spawn_type: 1,
                    level: 55,
                    class_id: 1,
                    race_id: 1,
                    x: 12.0,
                    y: 8.0,
                    z: 0.0,
                    heading: 90.0,
                    hp_current: 10,
                    hp_max: 10,
                    mana_current: 0,
                    mana_max: 0,
                    endurance_current: 100,
                    endurance_max: 100,
                    speed_run: 0.0,
                    stand_state: 0,
                    is_gm: false,
                },
                textquest_common::types::SpawnData {
                    spawn_id: 2,
                    name: "Undead".to_string(),
                    displayed_name: "a_undead".to_string(),
                    spawn_type: 1,
                    level: 3,
                    class_id: 1,
                    race_id: 1,
                    x: 4.0,
                    y: 6.0,
                    z: 0.0,
                    heading: 15.0,
                    hp_current: 5,
                    hp_max: 5,
                    mana_current: 0,
                    mana_max: 0,
                    endurance_current: 100,
                    endurance_max: 100,
                    speed_run: 0.0,
                    stand_state: 0,
                    is_gm: false,
                },
            ];
            state.target = state.spawns.first().cloned();
            state.xtargets = vec![textquest_common::types::SpawnData {
                spawn_id: 3,
                name: "XTarget".to_string(),
                displayed_name: "a_target".to_string(),
                spawn_type: 1,
                level: 10,
                class_id: 1,
                race_id: 1,
                x: 1.0,
                y: 1.0,
                z: 1.0,
                heading: 0.0,
                hp_current: 5,
                hp_max: 5,
                mana_current: 0,
                mana_max: 0,
                endurance_current: 100,
                endurance_max: 100,
                speed_run: 0.0,
                stand_state: 0,
                is_gm: false,
            }];
        }
        let lua = b.get_lua();
        let script = r#"
            local spawns = textquest.state.get_spawns()
            local sp = textquest.state.get_spawn("Mob")
            local found = textquest.state.find_spawns("undead")
            local tgt = textquest.state.get_target()
            local ok = textquest.state.set_target("Rathyl")
            local xt = textquest.state.get_xtargets()
            return true
        "#;
        let ok: bool = lua.load(script).eval().expect("state API callable");
        assert!(ok);
        let result: Option<textquest_common::types::SpawnData> = {
            b.runtime_state
                .read()
                .expect("lua runtime_state lock poisoned")
                .target
                .clone()
        };
        assert!(result.is_some());
    }

    #[test]
    fn config_api_all_fns_callable() {
        let b = make_bindings();
        let lua = b.get_lua();
        let script = r#"
            local ok1 = textquest.config.set("key", "value")
            local v = textquest.config.get("key")
            local ok2 = textquest.config.save()
            local ok3 = textquest.config.reload()
            return ok1 and v == "value" and ok2 and ok3
        "#;
        let ok: bool = lua.load(script).eval().expect("config API callable");
        assert!(ok);
        {
            let state = b.runtime_state.read().expect("lua runtime_state lock poisoned");
            assert_eq!(
                state
                    .plugin_config
                    .get("key")
                    .expect("config value stored"),
                "value"
            );
            assert!(state.last_saved_config);
            assert!(state.last_reloaded_config);
        }
    }

    #[test]
    fn ipc_api_queueing_metadata() {
        let b = make_bindings();
        let lua = b.get_lua();
        lua.load(
            r#"
textquest.ipc.send("/follow", "box-7")
textquest.ipc.broadcast("/say hello")
"#,
        )
        .exec()
        .expect("ipc commands");

        assert_eq!(
            b.drain_command_requests(),
            vec![
                LuaCommandRequest {
                    command: "/follow".to_string(),
                    target_box: Some("box-7".to_string()),
                    via_ipc: true,
                },
                LuaCommandRequest {
                    command: "/say hello".to_string(),
                    target_box: None,
                    via_ipc: true,
                }
            ]
        );
    }

    #[test]
    fn log_api_all_fns_callable() {
        let b = make_bindings();
        let lua = b.get_lua();
        let script = r#"
            textquest.log.info("info message")
            textquest.log.warn("warn message")
            textquest.log.error("error message")
            textquest.log.debug("debug message")
            return true
        "#;
        let ok: bool = lua.load(script).eval().expect("log API callable");
        assert!(ok);
    }

    #[test]
    fn debug_api_hex_dump_and_state_are_callable() {
        let b = make_bindings();
        let lua = b.get_lua();
        let script = r#"
            local dump = textquest.debug:hex_dump("0x1000", 32)
            local tracing_ok = textquest.debug.enable_command_tracing()
            local tracing_enabled = textquest.debug.command_tracing_enabled()
            local state = textquest.debug.inspect_camp_state()
            return dump, tracing_ok, tracing_enabled, state.status, state.ipc_queue_depth
        "#;
        let (dump, tracing_ok, tracing_enabled, status, queue_depth): (
            String,
            bool,
            bool,
            String,
            u32,
        ) = lua.load(script).eval().expect("debug API callable");
        assert!(dump.contains("address=0x1000"));
        assert!(dump.contains("size=32"));
        assert!(tracing_ok);
        assert!(tracing_enabled);
        assert_eq!(status, "unavailable");
        assert_eq!(queue_depth, 0);
    }

    #[test]
    fn issue_791_top_level_api_shape_is_callable() {
        let b = make_bindings();
        let lua = b.get_lua();
        let script = r#"
            local textquest = require("textquest")
            local spawns = textquest.get_spawns()
            local player = textquest.get_player()
            local cast_ok = textquest.cast_spell("group heal")
            local move_ok = textquest.move_to(1.0, 2.0, 3.0)
            local config_ok = textquest.camp_config:set("heal_mana_pct", 50)
            textquest.log("info", "My message")
            return type(spawns) == "table"
                and type(player) == "table"
                and cast_ok
                and move_ok
                and config_ok
        "#;
        let ok: bool = lua
            .load(script)
            .eval()
            .expect("issue #791 API shape callable");
        assert!(ok);
    }

    #[test]
    fn events_api_all_fns_callable() {
        let b = make_bindings();
        let lua = b.get_lua();
        let script = r#"
            textquest.events.on("combat_start", function() end)
            textquest.events.off("combat_start")
            textquest.events.emit("test_event", {data = 1})
            return true
        "#;
        let ok: bool = lua.load(script).eval().expect("events API callable");
        assert!(ok);
    }

    #[test]
    fn events_api_invokes_registered_callbacks() {
        let b = make_bindings();
        let lua = b.get_lua();
        let count: i64 = lua
            .load(
                r#"
local count = 0
textquest.events.on("hp_change", function(data)
    count = count + data.delta
end)
local fired = textquest.events.emit("hp_change", { delta = 2 })
return count + fired
"#,
            )
            .eval()
            .expect("event callback should run");

        assert_eq!(count, 3);
    }

    // ── Error conditions ───────────────────────────────────────────────────────

    #[test]
    fn malformed_syntax_returns_error() {
        let b = make_bindings();
        let lua = b.get_lua();
        // Missing 'end' — syntax error
        let result = lua.load("function broken(").eval::<mlua::Value>();
        assert!(result.is_err(), "malformed Lua syntax must return Err");
    }

    #[test]
    fn runtime_error_in_script_returns_error() {
        let b = make_bindings();
        let lua = b.get_lua();
        let result = lua
            .load("error('deliberate runtime error')")
            .eval::<mlua::Value>();
        assert!(result.is_err(), "runtime error must propagate as Err");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("deliberate runtime error"),
            "error message must propagate: {msg}"
        );
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
        let bindings = LuaBindings::with_registries(Arc::clone(&cmd_reg), Arc::clone(&hk_reg))
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
