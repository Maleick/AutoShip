# Lua Integration Strategy

Research document for TextQuest Lua scripting support. Generated 2026-04-19.

---

## Table of Contents

1. [Lua VM Selection](#1-lua-vm-selection)
2. [Current Integration Status](#2-current-integration-status)
3. [Dependencies](#3-dependencies)
4. [Plugin Surface (Lua API)](#4-plugin-surface-lua-api)
5. [Usage](#5-usage)

---

## 1. Lua VM Selection

TextQuest uses **mlua** (crate: `mlua` v0.11) as the Lua VM.

### Alternatives Considered

| VM | Pros | Cons |
|---|------|------|
| mlua (selected) | Pure Rust, no C binding deps, async support, good FFI | Smaller ecosystem than lua-sys |
| rlua | Established, stable | C binding dependency |
| lua-sys | Direct Lua C API access | Requires Lua compiled, FFI overhead |

### Decision Rationale

- **Pure Rust**: No external Lua DLL dependencies simplifies distribution
- **No DLL hell**: Embedded VM avoids version conflicts with other EQ tools
- **Async-ready**: mlua supports async functions natively
- **API stability**: v0.11 is stable with proven API

### Lua Version

Configured with **Lua 5.4** via `mlua` (`features = ["lua54", "vendored", "send"]`).

Rationale: TextQuest embeds the interpreter so operators do not need a
preinstalled Lua runtime. OpenVanilla/MacroQuest parity remains the target, but
this repo currently uses Lua 5.4 rather than LuaJIT.

---

## 2. Current Integration Status

Lua integration is **partially implemented** with bindings and script loader in
place:

- `textquest/src/lua/` module exists with:
  - `mod.rs` - Module root
  - `bindings.rs` - API registration (360 lines)
  - `loader.rs` - Script loader and execution (211 lines)
  - `types.rs` - Lua types (`LuaPlayer`, `LuaSpawn`, `LuaTarget`, `LuaGroupMember`)
  - `error.rs` - Error handling (`LuaApiError`)

- `LuaBindings` struct initializes the Lua context and registers all APIs
- `ScriptLoader` handles script loading, execution, and management
- Player APIs read from an injected Rust-side snapshot when one is available and
  return zero values otherwise
- Navigation and command APIs queue typed requests for future orchestrator/IPC
  delivery
- Events store Lua callbacks and can be emitted from Lua or Rust through
  `LuaBindings::emit_event`

### Implemented Components

1. **Lua VM initialization** - `Lua::new()` creates isolated Lua state
2. **API table registration** - Creates `textquest` global table
3. **Domain registration** - Each domain (player, group, nav, etc.) registered as sub-table
4. **Script loader** - Loads and executes Lua scripts from filesystem
5. **REPL support** - `execute_string()` for runtime code execution
6. **Runtime request bridge scaffold** - queued navigation and slash-command
   requests are drainable from Rust

---

## 3. Dependencies

In `textquest/Cargo.toml`:

```toml
mlua = { version = "0.11", default-features = false, features = ["lua54", "vendored", "send"] }
```

### Features

| Feature | Status |
|---------|--------|
| lua54 | Enabled |
| lua52 | Not enabled |
| lua53 | Not enabled |

---

## 4. Plugin Surface (Lua API)

The Lua API exposes TextQuest functionality across these domains:

### textquest.player.*

Player character information and state.

```lua
-- Getters
textquest.player.get_hp()              -- Current HP (int)
textquest.player.get_hp_percent()        -- HP % (float)
textquest.player.get_mana()            -- Current mana (int)
textquest.player.get_mana_percent()     -- Mana % (float)
textquest.player.get_endurance()        -- Current endurance (int)
textquest.player.get_endurance_percent() -- Endurance % (float)
textquest.player.get_name()             -- Character name (string)
textquest.player.get_level()            -- Level (uint8)
textquest.player.get_class()           -- Class name (string)
textquest.player.get_class_id()          -- Class ID (uint8)
textquest.player.get_race_id()           -- Race ID (uint32)
textquest.player.get_x()                -- X coordinate (float)
textquest.player.get_y()                -- Y coordinate (float)
textquest.player.get_z()               -- Z coordinate (float)
textquest.player.get_heading()        -- Heading (float)
textquest.player.get_speed()           -- Movement speed (float)
textquest.player.is_moving()            -- Is moving (bool)
textquest.player.is_feigned()          -- Is feigned death (bool)
textquest.player.is_dead()              -- Is dead (bool)
textquest.player.is_gm()               -- Is GM (bool)
```

### textquest.group.*

Group management and member access.

```lua
textquest.group.get_member_count()  -- Group size (int)
textquest.group.get_member(index)    -- Member by index (table or nil)
textquest.group.get_members()       -- All members (table)
textquest.group.get_tank()         -- Main tank (table or nil)
textquest.group.get_assist()        -- Assist (table or nil)
textquest.group.get_master()       -- Master looter (table or nil)
```

### textquest.nav.*

Navigation and movement control.

```lua
textquest.nav.goto(x, y, z)              -- Navigate to position
textquest.nav.stick(target)              -- Stick to target
textquest.nav.stop()                   -- Stop movement
textquest.nav.follow(target)              -- Follow target
textquest.nav.add_waypoint(x, y, z, name) -- Add waypoint
textquest.nav.clear_waypoints()           -- Clear all waypoints
```

### textquest.combat.*

Combat actions andspell casting.

```lua
textquest.combat.cast(spell, target)       -- Cast spell
textquest.combat.assist(target)         -- Assist target
textquest.combat.attack(target)        -- Attack target
textquest.combat.disengage()          -- Stop attacking
textquest.combat.re_mez(target)        -- Re-mezz target
textquest.combat.rezz(target)           -- Rezz target
```

### textquest.state.*

Spawn tracking and target management.

```lua
textquest.state.get_spawns()          -- All spawns (table)
textquest.state.get_spawn(name)       -- Spawn by name (table or nil)
textquest.state.find_spawns(filter)   -- Find spawns by filter (table)
textquest.state.get_target()         -- Current target (table or nil)
textquest.state.set_target(name)    -- Set target by name (bool)
textquest.state.get_xtargets()       -- XTargets (table)
```

### textquest.config.*

Configuration management.

```lua
textquest.config.get(key)       -- Get config value
textquest.config.set(key, val) -- Set config value
textquest.config.save()       -- Save config
textquest.config.reload()     -- Reload config
```

### textquest.log.*

Logging to TextQuest log output.

```lua
textquest.log.info(message)   -- Info level log
textquest.log.warn(message)   -- Warning level log
textquest.log.error(message)  -- Error level log
textquest.log.debug(message)  -- Debug level log
```

### textquest.events.*

Event system for Lua script callbacks.

```lua
textquest.events.on(event, callback)  -- Register event handler
textquest.events.off(event)           -- Unregister all handlers for an event
textquest.events.emit(event, data)    -- Emit event and return callback count
```

Rust hosts can emit registered callbacks with:

```rust
bindings.emit_event("hp_change", data)?;
```

### textquest.commands.*

Lua scripts can register local slash-command callbacks and queue commands for
the orchestrator:

```lua
textquest.commands.register("/myscript", function(args)
  textquest.log.info(args)
end)

textquest.commands.execute("/sit")
```

### Supported Events (planned)

| Event | Description |
|-------|-------------|
| `on_hp_change` | HP changed |
| `on_mana_change` | Mana changed |
| `on_target_change` | Target changed |
| `on_spawn_add` | Spawn added |
| `on_spawn_remove` | Spawn removed |
| `on_zone` | Zone changed |
| `on_death` | Player died |
| `on_combat_start` | Entered combat |
| `on_combat_end` | Exited combat |

---

## 5. Usage

### Loading Scripts

Scripts are loaded from the scripts directory:

```
scripts/
  init.lua      -- Global initialization
  <character>/
    init.lua    -- Character-specific scripts
```

### Example Script

```lua
-- Current API smoke example
local tq = require("textquest")

tq.events.on("hp_change", function(data)
    if data.hp_percent < 30 then
        tq.commands.execute('/casting "Greater Healing"')
    end
end
```

See `scripts/lua/examples/06_textquest_api_smoke.lua` for a runnable example
using the lower-case `textquest` module.

---

## Next Steps

1. **Connect APIs to live state** - Feed snapshots from the live process reader
   and drain queued requests into orchestrator/DLL IPC
2. ~~**Script loader**~~ - Implemented in `loader.rs`
3. ~~**Event callback registry**~~ - Implemented in `bindings.rs`
4. **Event producers** - Wire up event emit calls from combat/state modules
5. **Error handling** - Expand script error catching and logging in live flows
6. **Script sandboxing** - Continue validating limits against real operator
   scripts

## Script Loader API

The `ScriptLoader` provides these functions:

```rust
let loader = create_loader(scripts_dir)?;

// Load init.lua from scripts root
loader.load_init_script()?;

// Load character-specific scripts
loader.load_character_script("character_name")?;

// Execute arbitrary Lua code
let result: i64 = loader.execute_string("return 1 + 1")?.cast()?;

// Call a registered API function
loader.call_function("player", "get_hp", vec![])?;

// Reload all scripts
loader.reload()?;
```
