# Named NPC System

TextQuest tracks named (non-generic) NPCs automatically during camp loops. When a
named mob spawns or dies the system emits `NamedAlert` events, estimates respawn
windows from the database, and exposes a priority-ordered target API for the MA.

## Overview

Three crates collaborate on named NPC tracking:

| Crate | Module | Role |
|---|---|---|
| `textquest` | `eq::named_db` | Loads per-zone TOML files from `config/named_mobs/` |
| `textquest` | `eq::named_tracker` | Tick-by-tick spawn tracking, alert emission |
| `textquest-dll` | `combat::strategy` | `is_named_mob` detection, `NamedPreference` target scan |

## Detection: `is_named`

A spawn is treated as **named** if its displayed name does **not** start with a
generic article (`a `, `an `, `the `) and is not empty. This mirrors the EQ
naming convention where generic mobs are always prefixed with an article.

```
Generic: "a fire beetle", "an orc pawn", "the scribe"
Named:   "Emperor Crush", "Lord Nagafen", "Ghoul Lord"
```

The check is case-insensitive — `"A Fire Beetle"` is generic, `"Phinigel Autropos"`
is named.

## The Named Mob Database

Zone definitions live in `config/named_mobs/<zone>.toml`. Each file names the
zone and lists one `[[named]]` entry per mob.

### TOML Schema

```toml
zone = "crushbone"        # Must match EQ's internal zone short name (lowercase)

[[named]]
name = "Emperor Crush"    # Displayed name, exact case (lookup is case-insensitive)
level = 15                # Expected mob level
respawn_min_minutes = 28  # Shortest observed respawn (opens the respawn window)
respawn_max_minutes = 32  # Longest observed respawn (closes the respawn window)
location = [-688.0, 118.0, 28.0]  # EQ coordinates [x, y, z] for map pin
drops = ["Crushbone Belt", "Emperor Crush's Crown"]  # Notable loot (informational)
priority = "high"         # high | medium | low
```

### Priority Levels

| Priority | Meaning |
|---|---|
| `high` | Kill on sight — rare drops, quest targets, raid keys |
| `medium` | Worth pulling if nearby, not worth diverting a camp |
| `low` | Track for information; engage only when convenient |

### Example: Crushbone

```toml
zone = "crushbone"

[[named]]
name = "Emperor Crush"
level = 15
respawn_min_minutes = 28
respawn_max_minutes = 32
location = [-688.0, 118.0, 28.0]
drops = ["Crushbone Belt", "Emperor Crush's Crown"]
priority = "high"

[[named]]
name = "Ambassador DVinn"
level = 13
respawn_min_minutes = 22
respawn_max_minutes = 28
location = [-450.0, 250.0, 50.0]
drops = ["Elven Chainmail", "DVinn's Ceremonial Sword"]
priority = "medium"

[[named]]
name = "Lord Darish"
level = 12
respawn_min_minutes = 16
respawn_max_minutes = 22
location = [-340.0, 370.0, 28.0]
drops = ["Crushbone Shoulderpads"]
priority = "low"
```

## Spawn Tracking: `NamedTracker`

`NamedTracker` consumes the spawn list produced each game tick and maintains
per-mob state across ticks.

### Lifecycle

```
set_zone("crushbone")          ← call on every zone change; clears all tracked state
update(&spawns, tick)          ← call every tick; returns Vec<NamedAlert>
```

### Alert Events

```rust
pub enum NamedAlert {
    SpawnUp   { name, zone },                        // mob appeared or respawned
    SpawnDown { name, zone, respawn_estimate },       // mob died or despawned
}
```

`SpawnDown.respawn_estimate` is the earliest tick at which the mob may appear
again. If the database has a respawn range, `respawn_window_end_tick` tracks
when the window closes.

### Querying Live State

```rust
// All tracked spawns (alive + dead), alive-first, alphabetical within group
let status: Vec<&NamedSpawnStatus> = tracker.tracked_spawns();

// Highest-priority currently alive named mob (overrides normal MA target)
let target: Option<&NamedSpawnStatus> = tracker.priority_target();

// Named mobs inside their respawn window right now
let in_window: Vec<&NamedSpawnStatus> = tracker.in_respawn_window(current_tick);
```

### Integration Pattern

```rust
// Orchestrator setup
let db = NamedMobDatabase::load(Path::new("config/named_mobs"))?;
let mut named_tracker = NamedTracker::with_db(db);

// Zone change hook
named_tracker.set_zone(&new_zone);

// Each tick
let alerts = named_tracker.update(&current_spawns, tick);
for alert in alerts {
    match alert {
        NamedAlert::SpawnUp { name, zone } => {
            // Notify Discord, update TUI spawn-events panel
        }
        NamedAlert::SpawnDown { name, zone, respawn_estimate } => {
            // Log death, start respawn timer on TUI
        }
    }
}

// MA target override
if let Some(named) = named_tracker.priority_target() {
    // Pull named_target.name before normal camp targets
}
```

## Combat Strategy Integration

`textquest-dll` uses `is_named_mob` in `ma_target_scan` to implement named
preference ordering. The `TargetScanConfig.named_preference` field controls
behaviour:

| Variant | Behaviour |
|---|---|
| `NamedPreference::NamedFirst` | Named mobs form the preferred pool; trash fills in if no named present |
| `NamedPreference::TrashFirst` | Generic mobs preferred; named skipped if trash available |
| `NamedPreference::NoPreference` | All candidates treated equally |

## Discord Alerts

Named spawn/death events route through the Discord webhook layer configured in
`textquest/src/discord/webhook.rs`. Connect a `#named-alerts` channel in your
webhook routing config to receive real-time notifications.

## Adding a New Zone

1. Create `config/named_mobs/<zone_short_name>.toml`
2. Add `[[named]]` blocks for each named mob (use `/who` or zone guides for
   respawn times and coordinates)
3. Restart the orchestrator — `NamedMobDatabase::load` re-reads the directory
   on startup
4. No code changes required

## Tick Math Reference

The tracker converts minutes → ticks at **4 ticks per second**:

```
ticks = minutes × 60 × 4
```

Examples:
- 28 min respawn → 6,720 ticks
- 32 min respawn → 7,680 ticks (window end)

A typical EQ named has a 20–30 minute respawn. The `DEFAULT_RESPAWN_TICKS`
constant (1,200 ticks ≈ 5 min) is used only when the mob is not in the database
and exists solely to give the TUI a non-zero countdown.

## Covered Zones

Zone files ship in `config/named_mobs/`:

| File | Zone |
|---|---|
| `crushbone.toml` | Crushbone |
| `lowerguk.toml` | Lower Guk |
| `upperguk.toml` | Upper Guk |
| `unrest.toml` | Estate of Unrest |
| `mistmoore.toml` | Castle Mistmoore |
| `karnors.toml` | Karnor's Castle |
| `chardok.toml` | Chardok |
| `sebilis.toml` | Old Sebilis |

Add zones by dropping a TOML file — no code changes needed.
