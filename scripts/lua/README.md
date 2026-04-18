# TextQuest Lua Scripting API

This directory contains example Lua scripts demonstrating TextQuest's Lua automation capabilities. Scripts run within the TextQuest orchestrator and have access to game state, IPC messages, and operator controls.

## Overview

Lua scripts provide a lightweight scripting layer for operators to customize automation behavior without recompiling Rust. Scripts can:

- React to in-game events (combat, death, loot, etc.)
- Query game state (health, position, inventory, etc.)
- Execute automated actions (casting spells, moving, looting, etc.)
- Maintain persistent state and configuration
- Send notifications (Discord webhooks, TUI alerts, etc.)

## Example Scripts

### 1. Combat Helper (`01_combat_helper.lua`)

Manages healing rotation and target switching during combat.

**Demonstrates:**
- Event registration (`on_combat_start`, `on_combat_end`, `on_damage_taken`)
- Game state queries (`get_self_hp_percent()`, `get_group_members()`)
- Spell casting (`cast_spell()`)
- State tracking

**Key Features:**
- Automatic heal casting based on health thresholds
- Tank prioritization
- Group heal for multiple low members
- Damage tracking and combat statistics
- Configurable heal thresholds

**Usage:**
```lua
local combat = require("combat_helper")
combat.init()

-- Heals trigger automatically on damage events
-- Check status:
local stats = combat.get_stats()
print(stats.total_damage_taken)
```

### 2. Navigation Assistant (`02_navigation_assistant.lua`)

Records waypoints and plans repeatable routes through zones.

**Demonstrates:**
- Position queries (`get_position()`, `get_current_zone()`)
- Data persistence (`save_data()`, `load_data()`)
- Route planning and pathfinding stubs
- Movement automation (`move_to()`)

**Key Features:**
- Record waypoints during exploration
- Save named routes for replay
- Auto-distance checking between waypoints
- Route import/export as JSON
- Persistent storage via TextQuest

**Usage:**
```lua
local nav = require("navigation_assistant")
nav.init()

-- Record exploration
nav.start_recording()
nav.record_waypoint("camp_area")
nav.record_waypoint("vendor")
nav.stop_recording()

-- Save and replay
nav.save_route("camp_run")
nav.load_route("camp_run")
nav.navigate_to_next()
```

### 3. Buff Manager (`03_buff_manager.lua`)

Tracks active buffs and automatically recasts before expiration.

**Demonstrates:**
- Buff detection (`on_buff_added`, `on_buff_removed`)
- Duration tracking with expiration logic
- Scheduled recasting with cooldown management
- Configuration at runtime

**Key Features:**
- Configure buffs per character class
- Auto-recast before expiration margin
- Combat avoidance (don't cast in combat)
- Spell gem cooldown detection
- Statistics and emergency rebuff

**Usage:**
```lua
local buff = require("buff_manager")
buff.init()

-- Buffs maintain automatically every 30 seconds
-- Manual checks:
print(buff.has_buff("haste"))
print(buff.get_buff_remaining("strength"))

-- Runtime config
buff.add_buff_config({
  name = "custom_buff",
  spell = "Custom Spell",
  duration = 1800,
  recast_margin = 200,
})
```

### 4. Loot Filter (`04_loot_filter.lua`)

Evaluates loot and auto-loots based on custom rules.

**Demonstrates:**
- Loot events (`on_loot_available`)
- Priority ranking and rule evaluation
- Whitelist/blacklist functionality
- Item valuation
- Statistics and reporting

**Key Features:**
- Keyword-based priority matching
- Whitelist (always loot) and blacklist (never loot)
- Minimum value thresholds
- Automatic pickup of priority items
- Loot history and statistics
- JSON export/import of rules

**Usage:**
```lua
local loot = require("loot_filter")
loot.init()

-- Loot automatically fires on available items
-- Manual control:
loot.toggle_auto_loot()
loot.whitelist_item("Krono")
loot.blacklist_item("Trash Item")
loot.set_priority("Velious", 200)

local stats = loot.get_stats()
print(stats.total_value)
```

### 5. Discord Integration (`05_discord_integration.lua`)

Sends real-time updates to Discord about game events.

**Demonstrates:**
- HTTP requests (`http_post()`)
- Webhook formatting (Discord embeds)
- Event-driven notifications
- Throttling and queuing
- Configuration management

**Key Features:**
- Combat event notifications
- Death alerts with location and killer
- Loot reports with item value
- Raid event reports (boss kills, wipes)
- Fleet status summaries
- Throttling to prevent spam
- Test message functionality

**Usage:**
```lua
local discord = require("discord_integration")

-- Configure webhook URL
discord.set_webhook_url("https://discord.com/api/webhooks/...")
discord.init()

-- Events post automatically
-- Manual posts:
discord.send_fleet_status()
discord.report_error("Something went wrong!", "Critical Alert")
discord.send_test()
```

## TextQuest Lua API Reference

Scripts access game state and execute actions through the global `TextQuest` table.

### Event Handlers

Register callbacks for game events:

```lua
TextQuest.on_combat_start(callback)
TextQuest.on_combat_end(callback)
TextQuest.on_death(callback)
TextQuest.on_buff_added(callback)
TextQuest.on_buff_removed(callback)
TextQuest.on_loot_available(callback)
TextQuest.on_damage_taken(callback)
TextQuest.on_spell_cast_complete(callback)
```

### Game State Queries

```lua
-- Position and movement
TextQuest.get_position()              -- Returns {x, y, z}
TextQuest.get_current_zone()          -- Returns zone name

-- Character state
TextQuest.get_self_hp_percent()       -- Returns 0-100
TextQuest.get_self_mana_percent()     -- Returns 0-100
TextQuest.get_self_level()            -- Returns level

-- Combat state
TextQuest.in_combat()                 -- Returns boolean
TextQuest.get_current_target()        -- Returns target info
TextQuest.get_group_members()         -- Returns array of group members
TextQuest.get_group_member_hp_percent(name)  -- Returns 0-100

-- Inventory
TextQuest.get_inventory()             -- Returns inventory contents
TextQuest.get_inventory_free_slots()  -- Returns free slot count
```

### Actions

```lua
-- Combat actions
TextQuest.cast_spell(spell_name, target)
TextQuest.set_target(target_name)

-- Movement
TextQuest.move_to(x, y, z)
TextQuest.move_forward(distance)
TextQuest.rotate_to_heading(heading)

-- Looting
TextQuest.loot_item(item_name)

-- Messaging
TextQuest.say(message)
TextQuest.chat(message, channel)
```

### Persistence

Store data that persists across script reloads:

```lua
TextQuest.save_data(key, value)       -- Serialize and store
TextQuest.load_data(key)              -- Retrieve and deserialize

-- Example:
TextQuest.save_data("waypoints", state.waypoints)
local waypoints = TextQuest.load_data("waypoints")
```

### HTTP

Make web requests from scripts:

```lua
TextQuest.http_post(url, payload)    -- POST JSON, returns boolean
TextQuest.http_get(url)               -- GET request, returns response
```

### Configuration

```lua
TextQuest.get_config(key)             -- Retrieve config value
TextQuest.set_config(key, value)      -- Store config value
```

## Loading and Using Scripts

### Via TUI

1. Open the Scripts menu in TextQuest TUI
2. Click "Load Script"
3. Select Lua script from `scripts/lua/examples/`
4. Script initializes and listens for events

### Via Config

Add to character configuration:

```toml
[automation]
lua_scripts = [
  "scripts/lua/examples/01_combat_helper.lua",
  "scripts/lua/examples/03_buff_manager.lua",
]
```

### Manual Loading

```lua
local combat = require("01_combat_helper")
combat.init()
```

## Script Development Guidelines

### 1. Namespace Your Module

Always return a module table to avoid conflicts:

```lua
local MyScript = {}

function MyScript.init()
  -- initialization
end

return MyScript
```

### 2. Error Handling

Check for TextQuest API availability:

```lua
if not TextQuest or not TextQuest.cast_spell then
  print("TextQuest API not available")
  return false
end
```

### 3. Configuration

Make scripts configurable:

```lua
local CONFIG = {
  enable_feature = true,
  threshold = 50,
}

function MyScript.set_config(key, value)
  CONFIG[key] = value
end
```

### 4. Logging

Use consistent log formatting for TUI integration:

```lua
print(string.format("[MyScript] Event: %s", event_name))
```

### 5. State Management

Keep mutable state in a `state` table:

```lua
local state = {
  active = false,
  counter = 0,
}

function MyScript.reset()
  state = {active = false, counter = 0}
end
```

### 6. Performance

- Cache expensive queries (e.g., `get_position()`)
- Use throttling for high-frequency events
- Avoid tight loops in event handlers
- Yield control back to orchestrator frequently

## Troubleshooting

### Script not loading

- Check syntax with `luac -p script.lua`
- Ensure script returns a module table
- Check TUI console for error messages
- Verify TextQuest API is available

### Events not firing

- Verify event handler is registered in `init()`
- Check if feature is enabled in configuration
- Review TUI logs for missed events
- Test with print statements in callback

### Actions not executing

- Confirm TextQuest API method exists
- Check TextQuest.in_combat() if avoiding combat actions
- Verify target/spell names are exact
- Add error handling around API calls

## Examples & Community

These example scripts serve as templates for custom automation:

1. **Combat Helper** → Extend with custom heal logic per class
2. **Navigation** → Build zone-specific waypoint libraries
3. **Buff Manager** → Create class-specific buff rotations
4. **Loot Filter** → Tune for your farming goals
5. **Discord** → Add custom notifications for your raid schedule

Share community scripts in the TextQuest GitHub discussions or Wiki.

## Performance & Stability

- Scripts run in the orchestrator thread (non-blocking)
- Maximum execution time per event: 1 second
- Persistent storage is atomic (transactions)
- Memory limit per script: 10MB
- Maximum scripts loaded simultaneously: 10

## Security

Scripts execute in a sandboxed Lua environment:

- No file I/O access (use TextQuest.save_data instead)
- No subprocess execution
- Network requests limited to TextQuest.http_* APIs
- No access to system environment variables
- All script actions logged for audit

## Future Enhancements

Planned Lua API additions:

- Advanced pathfinding (navmesh integration)
- Packet manipulation (custom spell effects)
- Database queries (pattern matching, offset lookups)
- Machine learning integration (behavior prediction)
- Multiplayer coordination (multi-box synchronization)
- Plugin system (load shared libraries)

## Contributing

To contribute new example scripts:

1. Create script in `scripts/lua/examples/NN_name.lua`
2. Follow naming convention: `NN_` prefix for ordering
3. Include comprehensive docstring and examples
4. Test with multiple characters/classes
5. Document API usage patterns used
6. Submit PR with description of capabilities

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for details.
