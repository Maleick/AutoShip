# TextQuest Lua Script Configuration Guide

This guide explains how to configure Lua scripts for your specific character class, playstyle, and goals.

## Configuration Pattern

All example scripts follow a standard configuration pattern:

```lua
local CONFIG = {
  -- Feature flags
  enable_feature = true,
  
  -- Thresholds and limits
  min_value = 50,
  max_distance = 500,
  
  -- Spell and item names
  primary_spell = "Spell Name",
  target_item = "Item Name",
  
  -- Lists and maps
  whitelist = { "Item1", "Item2" },
  priorities = { ["Item"] = 100 },
  
  -- Logging and debug
  enable_logging = true,
  show_details = false,
}
```

## Per-Script Configuration

### Combat Helper Configuration

```lua
local CONFIG = {
  -- Healing thresholds (percentage of max HP)
  self_heal_threshold = 40,      -- Heal self when below this
  tank_heal_threshold = 35,      -- Heal tank when below this
  group_heal_threshold = 50,     -- Cast group heal if 2+ members below this

  -- Spell names (MUST match your spellbook)
  fast_heal_spell = "Complete Heal",
  group_heal_spell = "Group Heal",
  cure_spell = "Cure Poison",

  -- Behavior
  enable_combat_log = true,      -- Print combat actions
  enable_damage_tracking = true, -- Track incoming damage
}
```

**Customization Tips:**

| Class | self_heal | tank_heal | heal_spell | group_spell |
|-------|-----------|-----------|-----------|-------------|
| Cleric | 35% | 30% | Complete Heal | Full Heal Circle |
| Druid | 45% | 40% | Regrowth | Regrowth All |
| Shaman | 50% | 45% | Restless Heal | Greater Healing Wave |
| Enchanter | N/A | N/A | N/A | N/A |

**Examples:**

```lua
-- Aggressive healing (more casts, less downtime)
self_heal_threshold = 60
tank_heal_threshold = 55

-- Conservative healing (mana-efficient)
self_heal_threshold = 25
tank_heal_threshold = 20
```

### Navigation Assistant Configuration

```lua
local CONFIG = {
  -- Pathfinding behavior
  enable_auto_pathing = false,   -- Automatically move between waypoints
  enable_auto_pull = false,      -- Pull mobs at waypoints
  path_smoothing = true,         -- Smooth movement curves

  -- Safety limits
  max_waypoint_distance = 500,   -- Warn if waypoints > 500 units apart
  safe_radius = 50,              -- Distance to consider "arrived" at WP
}
```

**Zone-Specific Waypoint Naming:**

```lua
-- Dreadlands Circuit
nav.record_waypoint("dreads_camp_main")
nav.record_waypoint("dreads_east_camp")
nav.record_waypoint("dreads_north_pull")

-- Fungal Forest
nav.record_waypoint("fungal_safe_room")
nav.record_waypoint("fungal_east_camp")
nav.record_waypoint("fungal_boss_room")
```

**Route Planning Tips:**

1. Record waypoints while physically running the route
2. Go slowly and carefully (path should be safe)
3. Include "safe" waypoints before danger
4. Test route with 1 client before multi-boxing
5. Save waypoints frequently (alt+S in script)

### Buff Manager Configuration

```lua
local CONFIG = {
  enable_auto_recast = true,  -- Automatically refresh buffs

  buffs_to_maintain = {
    {
      name = "haste",
      spell = "Spirit of the Cheetah",
      duration = 2400,        -- How long buff lasts (seconds)
      recast_margin = 300,    -- Recast 5 min before expiry
    },
    -- ... more buffs
  },

  -- Prevent casting in these situations
  prevent_cast_in_combat = false,        -- Safe to cast while fighting
  prevent_cast_in_spell_gem_cooldown = true,

  -- Debug
  enable_logging = true,
  log_recasts = true,
}
```

**Class-Specific Buff Loadouts:**

**Cleric:**
```lua
buffs_to_maintain = {
  { name = "haste", spell = "Spirit of the Cheetah", duration = 2400, recast_margin = 300 },
  { name = "strength", spell = "Strength of the Kunark", duration = 1800, recast_margin = 200 },
  { name = "ac", spell = "Armor of Righteousness", duration = 3600, recast_margin = 400 },
}
```

**Ranger:**
```lua
buffs_to_maintain = {
  { name = "haste", spell = "Spirit of the Cheetah", duration = 2400, recast_margin = 300 },
  { name = "attack", spell = "Strength of the Pack", duration = 1800, recast_margin = 200 },
  { name = "regen", spell = "Regeneration", duration = 2400, recast_margin = 300 },
}
```

**Mage:**
```lua
buffs_to_maintain = {
  { name = "haste", spell = "Spirit of the Cheetah", duration = 2400, recast_margin = 300 },
  { name = "mana_regen", spell = "Mana Regeneration", duration = 3600, recast_margin = 400 },
  { name = "shield", spell = "Shielding", duration = 3600, recast_margin = 400 },
}
```

### Loot Filter Configuration

```lua
local CONFIG = {
  -- Auto-loot controls
  enable_auto_loot = false,              -- Automatically loot valid items

  -- Value threshold (in platinum)
  min_value_to_loot = 50,                -- Only loot if > 50pp

  -- Keyword priorities (higher = loot sooner)
  priorities = {
    ["Krono"] = 1000,                    -- Absolute highest priority
    ["Conflagrant Ore"] = 500,
    ["Ethereal"] = 300,
    ["Kunark"] = 250,
    ["Velious"] = 200,
    ["Planar"] = 150,
    ["Ornate Plate"] = 100,
  },

  -- Never loot these items
  blacklist = {
    ["Rusty Short Sword"] = true,
    ["Tattered Cloth"] = true,
  },

  -- Always loot these (overrides other rules)
  whitelist = {
    ["Krono"] = true,
    ["Bronze Plate Boots"] = true,
  },

  -- Logging
  enable_logging = true,
  show_loot_evaluations = false,
}
```

**Loot Rule Examples:**

```lua
-- Aggressive farming (pickup everything > 10pp)
min_value_to_loot = 10
priorities = {
  [""] = 1,  -- Everything gets priority 1
}

-- Picky farming (only best items)
min_value_to_loot = 500
priorities = {
  ["Krono"] = 10000,
  ["Planar"] = 5000,
  ["Kunark"] = 1000,
}

-- Container-focused
whitelist = {
  ["Leather Pack"] = true,
  ["Cloth Satchel"] = true,
  ["Bag of Holding"] = true,
}
blacklist = {
  ["Steel Dagger"] = true,  -- Vendor trash
  ["Common Food"] = true,
}
```

**Item Price Suggestions:**

| Category | Price | Notes |
|----------|-------|-------|
| Krono | 500,000pp+ | Always loot |
| Planar Items | 100-500pp | Class-specific |
| Kunark Items | 50-200pp | Farm common |
| Velious Items | 30-150pp | Selective pickup |
| Raw Materials | 10-100pp | Bulk farming |
| Vendor Trash | <10pp | Usually skip |

### Discord Integration Configuration

```lua
local CONFIG = {
  -- Webhook URL from Discord developer portal
  webhook_url = nil,  -- Set via discord.set_webhook_url()

  -- Bot identity
  bot_name = "TextQuest",

  -- What to report
  report_combat_start = true,   -- Combat engaged
  report_death = true,          -- Character died
  report_levelup = true,        -- Leveled up
  report_loot = true,           -- Looted items
  report_raid_events = true,    -- Boss kills, wipes
  report_errors = true,         -- Script errors

  -- Throttling (prevent message spam)
  throttle_seconds = 5,         -- Min 5 sec between same event type
  throttle_events = {},         -- Auto-filled by script
}
```

**Setup Instructions:**

1. Create Discord server/channel for TextQuest
2. Create Discord webhook:
   - Server → Channel Settings → Integrations → Webhooks
   - Create webhook, copy URL
3. Configure in script:
   ```lua
   discord.set_webhook_url("https://discord.com/api/webhooks/YOUR_ID/YOUR_TOKEN")
   ```
4. Send test message:
   ```lua
   discord.send_test()
   ```

**Message Examples:**

```lua
-- Combat alert
discord.report_combat_start()
-- Posts: "Combat Started: Engaging **Dragon of Thule**"

-- Death alert
discord.report_death({zone = "Dreadlands", killer_name = "Great Wyvern"})
-- Posts: "Character Died in **Dreadlands** (Killer: Great Wyvern)"

-- Loot notification
discord.report_loot({name = "Krono", value = 500000})
-- Posts: "Looted: **Krono** (500000p)"
```

## Runtime Configuration Changes

Modify configuration while script is running:

### Combat Helper
```lua
local combat = require("01_combat_helper")

-- Adjust healing threshold
combat.CONFIG.self_heal_threshold = 50

-- Change healing spell
combat.CONFIG.fast_heal_spell = "Superior Healing"
```

### Loot Filter
```lua
local loot = require("04_loot_filter")

-- Add item to whitelist
loot.whitelist_item("Ethereal Boots")

-- Adjust priority
loot.set_priority("Conflagrant", 600)

-- Toggle auto-loot
loot.toggle_auto_loot()
```

### Buff Manager
```lua
local buff = require("03_buff_manager")

-- Add new buff dynamically
buff.add_buff_config({
  name = "custom_buff",
  spell = "My Spell",
  duration = 1800,
  recast_margin = 200,
})

-- Remove buff
buff.remove_buff_config("strength")
```

## Configuration Files

### Saving to File

Export config as JSON for sharing/backup:

```lua
local loot = require("04_loot_filter")

-- Export current rules
local json = loot.export_config()
print(json)

-- Manually save to file (via TUI):
-- Copy output and save to: ~/.textquest/configs/loot_rules.json
```

### Loading from File

Import saved configuration:

```lua
-- Read JSON file (you would implement this)
local config_json = '{"whitelist":["Krono","Ethereal"]}'

-- Parse and apply
local loot = require("04_loot_filter")
-- (Requires TextQuest.load_json() API - coming soon)
```

## Best Practices

### 1. Test Changes Safely

```lua
-- Before deploying to main character:
-- 1. Create test character
-- 2. Load script with new config
-- 3. Run for 1 hour, verify behavior
-- 4. Check logs for errors
-- 5. Only then deploy to main
```

### 2. Document Your Changes

```lua
-- At top of modified script:
-- CUSTOMIZATIONS:
--   - self_heal_threshold: 50% (lowered from 40% for cleric)
--   - fast_heal_spell: Changed to "Superior Healing"
--   - Added custom buff: "Spirit of Wolves"
```

### 3. Version Control

```lua
-- Keep original example
-- Create backup before editing
-- Use git to track changes

cp scripts/lua/examples/03_buff_manager.lua \
   scripts/lua/examples/03_buff_manager.lua.orig

git add scripts/lua/examples/
```

### 4. Progressive Rollout

```lua
-- Week 1: Run with conservative settings
min_value_to_loot = 100

-- Week 2: Proven stable, reduce threshold
min_value_to_loot = 50

-- Week 3: Fine-tune further
min_value_to_loot = 25
```

## Troubleshooting Configuration

### Script doesn't use my config changes

**Check:**
1. Did you reload the script? (TUI → Scripts → Reload)
2. Did you use the correct CONFIG key name?
3. Is the value the right type? (string vs number)

**Fix:**
```lua
-- Print current config to verify
print("[MyScript] Config:", CONFIG)

-- Reload script
TextQuest.reload_script("my_script")
```

### Script crashes with config error

**Check:**
- Mismatched quotes in spell names?
- Missing comma in table definition?
- String instead of number?

**Fix:**
```lua
-- WRONG: Unclosed string
spell = "Heal

-- RIGHT: Closed string
spell = "Heal",

-- WRONG: Missing comma
spell = "Heal"
duration = 1800

-- RIGHT: Added comma
spell = "Heal",
duration = 1800,
```

### Config reverts after reload

**Check:**
- Did you modify the hardcoded CONFIG table?
- Or are you calling a function that resets it?

**Fix:**
```lua
-- Use TextQuest.save_data() for persistence
function MyScript.save_config()
  TextQuest.save_data("my_config", CONFIG)
end

function MyScript.init()
  -- Load saved config
  local saved = TextQuest.load_data("my_config")
  if saved then CONFIG = saved end
end
```

## Configuration Reference

### Performance Tuning

If script causes TUI lag:

```lua
-- Reduce logging
enable_logging = false
show_loot_evaluations = false
log_recasts = false

-- Increase check intervals
-- (modify the main loop timing)
```

### Memory Optimization

For long-running sessions:

```lua
-- Limit history size
state.damage_log = {}  -- Clear periodically

-- Trim route list
-- Remove unused routes: delete_route(name)

-- Clear loot history
loot.reset_history()
```

### Network Tuning

For Discord integration over slow connections:

```lua
-- Increase throttle time
throttle_seconds = 10  -- Fewer messages

-- Batch messages
-- Enable queue: flush_queue() periodically
```

## Next Steps

1. Pick one script to customize
2. Read its CONFIG section carefully
3. Make 1-2 small changes
4. Test thoroughly
5. Gradually expand configuration

See [GETTING_STARTED.md](GETTING_STARTED.md) for practical examples.
