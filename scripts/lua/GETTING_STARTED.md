# Getting Started with TextQuest Lua Scripts

This guide walks you through loading and customizing Lua scripts for your TextQuest automation.

## Quick Start (5 minutes)

### 1. Load an Example Script

```bash
# Copy example to your scripts directory
cp scripts/lua/examples/01_combat_helper.lua ~/.textquest/scripts/

# In TextQuest TUI:
# - Press 'S' for Scripts menu
# - Select "Load Script"
# - Choose "01_combat_helper.lua"
```

### 2. Verify It Loaded

- Script prints `[CombatHelper] Ready. Listening for combat events.` to TUI console
- Engage in combat
- Watch for healing messages in the console

### 3. Configure It

Edit script parameters:

```lua
-- Find CONFIG table in script (top of file)
local CONFIG = {
  self_heal_threshold = 40,      -- Lower = heal sooner
  tank_heal_threshold = 35,      -- Tank-focused healing
  fast_heal_spell = "Complete Heal",
}
```

## Script Categories

Choose scripts based on your needs:

### Combat Automation
- **01_combat_helper.lua** - Heal rotation, target switching
  - Best for: Clerics, Druids, Shamans
  - Reduces APM in long fights

### Navigation & Movement
- **02_navigation_assistant.lua** - Waypoint recording, route replay
  - Best for: Multi-boxing, repeatablefarms
  - Automates boring travel sequences

### Buff Management
- **03_buff_manager.lua** - Auto-buff refresh, duration tracking
  - Best for: All classes
  - Reduces need for manual rebuffing

### Loot & Economy
- **04_loot_filter.lua** - Smart loot rules, auto-pickup
  - Best for: Farm characters, group looters
  - Increases farm efficiency

### Monitoring & Alerts
- **05_discord_integration.lua** - Real-time Discord notifications
  - Best for: Multi-box monitoring, raid tracking
  - Stay informed while AFK

## Customization Recipes

### Recipe 1: Class-Specific Healing

For a Shaman with different heal spells:

```lua
-- 01_combat_helper.lua modifications
local CONFIG = {
  self_heal_threshold = 50,     -- Shamans need more healing
  tank_heal_threshold = 40,
  
  fast_heal_spell = "Restless Healing",      -- Shaman fast heal
  group_heal_spell = "Greater Healing Wave", -- Shaman group heal
  cure_spell = "Cure Poison",
}
```

### Recipe 2: Aggressive Loot Strategy

Adjust loot filter for maximum value:

```lua
-- 04_loot_filter.lua modifications
local CONFIG = {
  min_value_to_loot = 10,    -- Pick up everything valuable
  
  priorities = {
    ["Krono"] = 10000,         -- MAX PRIORITY
    ["Planar Gremlin Scale"] = 800,
    ["Defiant"] = 100,
  },
  
  whitelist = {
    ["Raw-hide Armor"] = true,
  },
}
```

### Recipe 3: Discord Farm Monitoring

Get alerts when loot drops:

```lua
-- In your init code or Discord config:
discord.set_webhook_url("https://discord.com/api/webhooks/YOUR_ID/YOUR_TOKEN")
discord.toggle_feature("loot")     -- Enable loot notifications
discord.toggle_feature("death")    -- Enable death alerts
discord.send_test()                -- Verify connection
```

### Recipe 4: Multi-Zone Navigation

Record routes in multiple zones:

```lua
local nav = require("navigation_assistant")
nav.init()

-- Zone 1: Dreadlands
nav.start_recording()
nav.record_waypoint("Camp_A")
nav.record_waypoint("Camp_B")
nav.stop_recording()
nav.save_route("dreadlands_circuit")

-- Zone 2: Fungal Forest
-- Switch zone, repeat...
nav.start_recording()
nav.record_waypoint("Safe_Room")
nav.stop_recording()
nav.save_route("fungal_circuit")

-- Later, replay:
nav.load_route("dreadlands_circuit")
while nav.navigate_to_next() do
  -- Automatically moves to each waypoint
end
```

## Combining Scripts

Load multiple scripts together for synergy:

### Solo Farmer Setup
1. Combat Helper - Auto-heal during fights
2. Loot Filter - Auto-loot valuable drops
3. Discord Integration - Get alerts on screen

```lua
-- Combined init code
local combat = require("01_combat_helper")
local loot = require("04_loot_filter")
local discord = require("05_discord_integration")

combat.init()
loot.init()
discord.init()

discord.set_webhook_url("YOUR_WEBHOOK_URL")
discord.toggle_feature("loot")
```

### Multi-Box Group Setup
1. Combat Helper - Each class heals its role
2. Navigation Assistant - Replay group movement together
3. Buff Manager - Keep buffs fresh on all clients

```lua
-- On each client:
local combat = require("01_combat_helper")
local nav = require("navigation_assistant")
local buff = require("03_buff_manager")

combat.init()
nav.init()
buff.init()

-- Load shared route
nav.load_route("group_circuit")
```

## Troubleshooting

### Script loads but doesn't respond

**Check:**
1. Is the event you're listening for happening?
   - Test with `print()` statements in event handlers
2. Is TextQuest API available?
   - Add: `if not TextQuest then print("API not found") return end`
3. Is the script paused?
   - Check TUI Scripts menu - look for "Paused" status

**Fix:**
```lua
-- Add debug logging at top of init()
function MyScript.init()
  print("[MyScript] init() called - starting...")
  
  if not TextQuest then
    print("[MyScript] ERROR: TextQuest API not available")
    return
  end
  
  print("[MyScript] API available, registering handlers...")
end
```

### Script crashes or stops responding

**Check:**
1. Infinite loop in event handler?
2. Missing error handling on API calls?
3. Accessing undefined variables?

**Fix:**
```lua
-- Add try-catch pattern
function safe_cast_heal(target, spell)
  if not TextQuest or not TextQuest.cast_spell then
    print("[MyScript] ERROR: cast_spell not available")
    return false
  end
  
  local success = TextQuest.cast_spell(spell, target)
  if not success then
    print(string.format("[MyScript] Failed to cast %s on %s", spell, target))
  end
  return success
end
```

### Configuration changes don't take effect

**Solution:**
Reload the script via TUI:
- Scripts menu → Select script → "Reload"

Or in code:
```lua
-- Expose config setter
function MyScript.set_threshold(value)
  CONFIG.threshold = value
  print(string.format("[MyScript] Threshold set to %d", value))
end

-- Call from TUI console or other scripts:
MyScript.set_threshold(45)
```

## Performance Tips

### Reduce API Calls
```lua
-- SLOW: Queries every event
function on_damage()
  local hp = TextQuest.get_self_hp_percent()  -- Expensive!
  if hp < 40 then cast_heal() end
end

-- FAST: Cache query result
local last_hp = 100
function on_damage()
  last_hp = TextQuest.get_self_hp_percent()
  if last_hp < 40 then cast_heal() end
end
```

### Throttle High-Frequency Events
```lua
-- SLOW: Responds to every tick
function on_tick()
  evaluate_buffs()  -- Called 60x/second!
end

-- FAST: Check every N seconds
local last_check = 0
function on_tick()
  if os.time() - last_check < 5 then return end
  last_check = os.time()
  evaluate_buffs()
end
```

### Avoid Nested Loops
```lua
-- SLOW: O(n²) complexity
for _, member in ipairs(group) do
  for _, buff in ipairs(member.buffs) do
    if buff.expires < threshold then
      -- Exponential complexity
    end
  end
end

-- FAST: Single pass
for _, member in ipairs(group) do
  local next_expire = find_soonest_expiring_buff(member)
  if next_expire < threshold then
    -- Linear complexity
  end
end
```

## Advanced Usage

### Persistent Configuration

Save/load settings across reloads:

```lua
function MyScript.save_config()
  TextQuest.save_data("my_script_config", CONFIG)
end

function MyScript.load_config()
  local saved = TextQuest.load_data("my_script_config")
  if saved then
    CONFIG = saved
  end
end

function MyScript.init()
  MyScript.load_config()
  -- ... rest of init
end
```

### Exposing Functions to TUI

Make script functions callable from TUI console:

```lua
-- Define public API
function MyScript.status()
  return {
    active = state.active,
    buffs_active = #state.active_buffs,
  }
end

function MyScript.toggle()
  state.active = not state.active
  print("[MyScript] Active: " .. tostring(state.active))
end
```

Then in TUI console:
```
script:call(script_name, "status")
script:call(script_name, "toggle")
```

## Next Steps

1. **Try one example** - Load Combat Helper and test in combat
2. **Customize it** - Adjust thresholds for your class/playstyle
3. **Combine two scripts** - Load Navigation + Buff Manager together
4. **Create your own** - Copy an example and modify for your needs
5. **Share with community** - Submit interesting scripts as PRs

## References

- Full API docs: [README.md](README.md)
- Example scripts: `scripts/lua/examples/`
- Configuration guide: `CONFIGURATION.md` (coming soon)
- Community scripts: GitHub Discussions

## Getting Help

- **Syntax errors?** Check with `luac -p script.lua`
- **API questions?** See [README.md](README.md) Reference section
- **Script debugging?** Add `print()` statements and watch TUI console
- **Feature requests?** Post in GitHub Issues tagged `lua-scripting`

Happy scripting!
