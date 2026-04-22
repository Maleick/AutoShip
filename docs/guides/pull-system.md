# Pull System

> **Phase 2.3d** — Pull system for the TextQuest automation platform.
>
> Implementation references:
> - [`textquest/src/camp/state.rs`](../../textquest/src/camp/state.rs) — `CampLoop` FSM
> - [`textquest/src/camp/puller.rs`](../../textquest/src/camp/puller.rs) — target selection
> - [`textquest/src/camp/config.rs`](../../textquest/src/camp/config.rs) — `CampConfig`

## Overview

The pull system automates the core EverQuest farming loop: a designated puller fetches a
mob from the spawn area and brings it back to the camp where the group kills it. TextQuest
models this as a finite state machine (FSM) inside `CampLoop`.

**State cycle:**

```
Idle → Pulling → Fighting → Looting → Medding → Idle → …
```

Each transition is driven either by a tick timer or by a `CampSnapshot` (live game state).

---

## Core Types

### `CampConfig`

Stored in `config/camps/<name>.toml`. Controls where mobs are pulled from and when.

| Field | Type | Purpose |
|---|---|---|
| `name` | `String` | File stem identifier (e.g. `crushbone_entrance`) |
| `zone` | `String` | Zone short name (e.g. `crushbone`) |
| `camp_center` | `[f32; 3]` | XYZ anchor the group camps at |
| `pull_point` | `[f32; 3]` | XYZ coordinate the puller walks to before pulling |
| `pull_radius` | `f32` | Max distance from `pull_point` to consider a mob pullable |
| `camp_radius` | `f32` | Radius around `camp_center` members should stay within |
| `leash_radius` | `f32` | Max distance a pulled mob travels before being abandoned |
| `rest_mana_pct` | `u8` | Sit and med between pulls when healer mana is below this |
| `pull_mana_pct` | `u8` | Do not pull unless healer mana is at or above this |
| `level_range` | `[u8; 2]` | Min/max mob level for this camp (progression gating) |
| `pull_mob_names` | `Vec<String>` | Preferred mob names (empty = pull anything in range) |
| `ignore_mob_names` | `Vec<String>` | Mob names to never pull |
| `burn_mob_names` | `Vec<String>` | Named mobs to burn immediately (all DPS + CD) |
| `return_no_aggro` | `bool` | Delay return-to-camp if a member still has aggro |
| `next_camp` | `Option<String>` | Camp to progress to when the group outlevels this one |
| `prev_camp` | `Option<String>` | Camp to fall back to (reverse progression) |

### `Role`

Each `CampMember` is assigned exactly one role. Pull behavior depends on this:

| Role | Pull behavior |
|---|---|
| `Puller` | Receives `/target <mob>` + `/attack` on every pull |
| `Tank` | Falls back to puller when no `Puller` exists; assists puller on Fighting transition |
| `Healer` | Targets tank for healing; sits to med when mana is low |
| `CC` | Receives crowd-control commands before DPS engages |
| `Dps` | Assists tank (or puller) and attacks |
| `Bard` | Twist songs during combat |

### `CampState`

| State | Description |
|---|---|
| `Idle` | At camp, medded up, ready to pull |
| `Pulling` | Puller has been sent; waiting for mob to arrive |
| `Fighting` | Mob is in camp; full group engaged |
| `Looting` | Mob is dead; looting corpses |
| `Medding` | Group resting between pulls |
| `Buffing` | Rebuffing after resting |
| `Recovery` | A member died; waiting for rez/regroup |

---

## Pull Target Selection

`select_pull_target(spawns, config, cc_tracker, hvt_watchlist) → Option<String>`

Filters all nearby spawns and returns the best pull target. Priority order:

1. **Configured name match** (`pull_mob_names`) — closest matching spawn wins
2. **HVT watchlist** — named mobs on the high-value-target list
3. **Closest NPC** to `pull_point` — generic fallback

**Automatic exclusions:**

- Players (`SpawnType::Player`)
- Corpses (`SpawnType::Corpse`)
- Mobs already tracked by the CC system
- Mobs outside `pull_radius` of `pull_point`
- Mobs in `ignore_mob_names`

Extended variant: `select_pull_target_with_named(…, named_tracker)` checks the named
mob database first. If a named mob from the database is alive and in range it takes
top priority over everything else.

---

## Pull Patterns

### Single-Pull (default)

One mob at a time. The puller targets a single mob, pulls it to `pull_point`, and the
group kills it before the next pull begins.

```toml
# config/camps/crushbone_entrance.toml
name          = "crushbone_entrance"
zone          = "crushbone"
camp_center   = [100.0, 200.0, 0.0]
pull_point    = [150.0, 250.0, 0.0]
pull_radius   = 200.0
camp_radius   = 30.0
leash_radius  = 100.0
rest_mana_pct = 60
pull_mana_pct = 30
level_range   = [5, 12]
pull_mob_names = ["an orc pawn", "an orc centurion"]
```

### Directional Pull

Place `pull_point` along the natural path between the spawn and the camp center. This
guides which mobs are in range and keeps the pull path predictable.

```
[spawn area]  →  pull_point (150, 250)  →  camp_center (100, 200)
```

Set `pull_radius` to cover only one side of a room or corridor to avoid training.

### Burn Pull (Named Mobs)

Add a named mob to `burn_mob_names` to trigger a coordinated burn:

```toml
burn_mob_names = ["Emperor Crush", "King Doric"]
```

When a burn mob is pulled, all DPS activate cooldowns and the CC members hold their
abilities. The burn finishes before any other logic runs.

### HVT Pull (Named Tracker Priority)

Register named mobs in the named mob database. The tracker promotes them above all
other candidates when alive and in range — no config change needed per camp.

---

## Leash

The leash prevents a pulled mob from being chased too far from camp. If the mob
paths beyond `leash_radius` from `camp_center`, the puller disengages and the group
returns to camp.

```toml
leash_radius = 100.0   # EQ units from camp_center
```

**Relationship between radii:**

```
camp_radius  <  leash_radius  <  pull_radius
     25              100             200
```

- `camp_radius`: where group members stand
- `leash_radius`: maximum mob chase distance
- `pull_radius`: how far out to look for mobs

---

## Return-to-Camp Behavior

After each kill, all group members return to their camp positions:

1. `Fighting → Looting`: `/attack off` + `CombatDisengage` sent to all members
2. `Looting → Medding`: casters `/sit` when loot cycle completes
3. `Medding → Idle`: `/stand` when healer mana exceeds `rest_mana_pct`

### `return_no_aggro`

When `true`, casters do not `/sit` until they are out of combat:

```toml
return_no_aggro = true
```

Useful for camps where adds path nearby (e.g. Sebilis disco room).

---

## Mana Gating

The camp will not pull if the healer's mana is below `pull_mana_pct`:

```toml
pull_mana_pct = 30   # Wait until healer has ≥30% mana before pulling
rest_mana_pct = 60   # Sit and med until healer has ≥60% mana
```

---

## Recovery

If any group member dies, the FSM enters `Recovery`:

1. All combat stops
2. Cleric is directed to rez
3. Group waits for the dead member(s) to be alive
4. After `RECOVERY_REPOSITION_TICKS`, the FSM returns to `Idle`

Three consecutive wipes (full group deaths) cause the camp loop to stop automatically.

---

## Example Configurations

### Crushbone — entrance camp (5-12)

```toml
name          = "crushbone_entrance"
zone          = "crushbone"
camp_center   = [100.0, 200.0, 0.0]
pull_point    = [150.0, 250.0, 0.0]
pull_radius   = 200.0
camp_radius   = 30.0
leash_radius  = 100.0
rest_mana_pct = 60
pull_mana_pct = 30
level_range   = [5, 12]
pull_mob_names = ["an orc pawn", "an orc centurion"]
ignore_mob_names = ["Ambassador DVinn"]
next_camp     = "crushbone_throne"
```

### Sebilis Disco — AoE burn camp (50-60, named-heavy)

```toml
name              = "sebilis_disco"
zone              = "sebilis"
camp_center       = [0.0, 0.0, 0.0]
pull_point        = [40.0, 0.0, 0.0]
pull_radius       = 120.0
camp_radius       = 20.0
leash_radius      = 60.0
rest_mana_pct     = 80
pull_mana_pct     = 60
level_range       = [50, 60]
pull_mob_names    = ["a froglok knight", "a froglok squire"]
burn_mob_names    = ["Trakanon", "Xygoz"]
return_no_aggro   = true
prev_camp         = "lguk_dead_side"
```

---

## Testing

Run pull system unit tests:

```bash
cargo test --lib -p textquest camp::puller
cargo test --lib -p textquest camp::state
```

Run pull system integration tests (requires Windows or `#[cfg(windows)]` gate):

```bash
cargo test --test pull_system_integration
```

Run scoped (avoids recompiling unrelated crates):

```bash
cargo test -p textquest pull_system
```

---

## Architecture Notes

- `CampLoop::tick(snapshot)` is the single entry point — call it once per game frame
- Snapshots are optional; without them the FSM uses timer-based transitions only
- `CampAction::Slash(cmd)` wraps `/eq slash commands`; `CampAction::CombatEngage` wraps structured IPC
- The puller module (`camp::puller`) is platform-independent (`#[cfg(windows)]` is on the `state` module only)
- Camp configs are hot-loadable: save a new TOML to `config/camps/` and the orchestrator picks it up on the next progression check
