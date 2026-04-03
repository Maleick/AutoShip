# MQ2 Plugin Deep Dive for Frostreaver

Research document mapping MacroQuest plugin functionality to Frostreaver's M1-M8 roadmap.
Generated 2026-03-28.

---

## Table of Contents

1. [MQ2Nav — Navigation Mesh Pathfinding](#1-mq2nav--navigation-mesh-pathfinding)
2. [MQ2MoveUtils — Movement: Stick, Follow, Camp](#2-mq2moveutils--movement-stick-follow-camp)
3. [MQ2Cast — Reliable Spell Casting](#3-mq2cast--reliable-spell-casting)
4. [Combat Macros — KissAssist / MuleAssist / MQ2Melee](#4-combat-macros--kissassist--muleassist--mq2melee)
5. [MQ2Map — Map Overlay](#5-mq2map--map-overlay)
6. [Optimal 36-Box Group Composition](#6-optimal-36-box-group-composition)
7. [Gap Analysis vs Frostreaver M1-M4](#7-gap-analysis-vs-frostreaver-m1-m4)

---

## 1. MQ2Nav — Navigation Mesh Pathfinding

### How It Works

MQ2Nav is a navmesh-powered pathfinding plugin built on the **RecastNavigation** library (the same library used by Unity, Unreal Engine, and most AAA game engines for NPC pathing). It consists of two components:

- **MQ2Nav.dll** — The MacroQuest plugin that loads navmeshes at runtime, computes paths, and drives character movement
- **MeshGenerator.exe** — A standalone tool (separate Win32/x64 builds) for creating and editing zone navigation meshes

### Navmesh Format

- Zone meshes stored as `.navmesh` files (one per zone, e.g., `gfaydark.navmesh`)
- Custom binary format with versioning (current v5, backward-compatible with v4)
- v5 added decompressed data size in the header for faster loading
- Meshes compressed with **zlib**; serialized with **Protocol Buffers**
- Community mesh packs available at [mqmesh.com](https://mqmesh.com/) covering most zones
- Stored in `Resources/MQ2Nav/` directory

### Mesh Generation Process

1. MQ2Nav exports zone geometry + dynamic objects (PoK stones, etc.) to a config file on zone load
2. MeshGenerator.exe imports EQ zone geometry via **EQEmu zone-utilities** (reads EQ's `.s3d`/`.eqg` zone files)
3. RecastNavigation voxelizes the geometry into a heightfield, then builds a compact navigation mesh
4. Off-mesh connections are manually added for doors, teleports, and gap bridges
5. The resulting `dtNavMesh` is serialized and compressed to `.navmesh`

### Path Computation

- Uses RecastNavigation's **Detour** library for A* pathfinding on the navmesh
- `dtNavMeshQuery` finds the nearest polygon, then computes a corridor of polygons to the destination
- Path is smoothed into a series of waypoints using the string-pulling algorithm (funnel algorithm)
- Off-mesh connections enable traversal through doors, up stairs, and across teleport points

### Commands

| Command | Description |
|---------|-------------|
| `/nav target` | Navigate to current target |
| `/nav loc X Y Z` | Navigate to coordinates |
| `/nav door` | Navigate to nearest door and open it |
| `/nav item [click]` | Navigate to ground item, optionally pick up |
| `/nav waypoint <name>` | Navigate to saved waypoint |
| `/nav stop` | Stop navigation |
| `/nav pause` | Pause/resume navigation |
| `/nav reload` | Reload current zone mesh |
| `/nav recordwaypoint <name> <tag>` | Save current position as waypoint |
| `/nav ui` | Toggle debug overlay |

### Macro Data (TLO)

| Variable | Description |
|----------|-------------|
| `${Navigation.Active}` | Is navigation in progress |
| `${Navigation.MeshLoaded}` | Is a navmesh loaded for current zone |
| `${Navigation.PathExists}` | Can a path be found to destination |
| `${Navigation.PathLength}` | Distance of computed path |
| `${Navigation.Velocity}` | Current movement speed |

### 3D Overlay

MQ2Nav includes an optional in-game overlay (via **imgui** + DirectX hooking) that renders:
- The navigation mesh as a wireframe on the ground
- The computed path as a colored line
- Off-mesh connections as purple line segments
- Debug UI for mesh inspection

### Dependencies

- RecastNavigation (navmesh + pathfinding)
- EQEmu zone-utilities (zone geometry loading)
- imgui (debug overlay UI)
- Protocol Buffers (mesh serialization)
- GLM (math)
- SDL2 (MeshGenerator visualization)
- zlib (compression)

### Relevance to Frostreaver

**What we already have (M3):**
- `dmft-dll/src/nav/state.rs` — Navigator FSM with waypoint-based pathfinding
- `dmft-dll/src/nav/stuck.rs` — StuckDetector with escalating recovery
- `dmft-dll/src/nav/humanize.rs` — MovementPersonality for human-like movement
- `dmft-dll/src/nav/waypoint.rs` — WaypointQueue
- `dmft/src/nav/recorder.rs` — WaypointRecorder with RDP simplification
- `dmft/src/nav/camp.rs` — CampManager
- `dmft/src/nav/router.rs` — Zone routing

**What MQ2Nav adds that we lack:**
- **Navmesh-based pathfinding** — Our M3 uses pre-recorded waypoints, not dynamic path computation. This is the #1 gap identified in prior research.
- **Zone geometry loading** — We don't parse EQ zone files (`.s3d`/`.eqg`). We could either:
  - Port RecastNavigation's Detour query library to Rust (for runtime path queries)
  - Generate navmeshes offline and load the `.navmesh` files directly
  - Use the EQEmu zone-utilities approach to extract geometry
- **Off-mesh connections** — Door traversal, teleport handling
- **3D overlay** — Not needed for TUI, but the path visualization concept could map to our TUI map view

**Recommended approach:** Don't rewrite RecastNavigation. Instead, build a Rust `.navmesh` loader that reads MQ2Nav's v5 format, wraps `dtNavMeshQuery` via FFI (or a Rust port of Detour), and feeds waypoints to our existing Navigator FSM. This gives us dynamic pathfinding with minimal new code.

---

## 2. MQ2MoveUtils — Movement: Stick, Follow, Camp

### Overview

MQ2MoveUtils (created 2004 by tonio, final version 11.0410) is the definitive movement plugin for EQ boxing. It provides four core commands that handle all automated character positioning.

### /stick — Combat Positioning

Maintains a specified distance and angle relative to a target. This is the most complex and important command.

**Basic:** `/stick [distance]` — Stick to target at distance (default: melee range)

**Positioning modes:**
| Mode | Description |
|------|-------------|
| `behind` | Position at target's rear arc |
| `!front` | Anywhere except frontal arc |
| `front` | Frontal arc (for tanking) |
| `pin` | Side positioning |
| `behindonce` | Initial rear position, then distance only |
| `snaproll [left\|right\|face\|rear]` | Run behind target, rotate to specified direction |

**Modifiers:**
| Modifier | Description |
|----------|-------------|
| `loose` | Human-like incremental turning (not instant snapping) |
| `truehead` | Actual keypress heading changes (not memory writes) |
| `healer` | Suppress face adjustments while in stick range |
| `hold` | Keep sticking to current target despite retargeting |
| `id [#]` | Stick to specific spawn ID |
| `always` | Auto-resume on next valid NPC when target lost |
| `moveback` | Walk backward if target approaches closer than distance |
| `underwater/uw` | Vertical angle adjustment |
| `delaystrafe` | Delay strafing to prevent endless circling |
| `usefleeing` | Don't front-position when target flees |
| `randomize` | Random arc values for `behind` and `!front` |
| `useback` | Walk backward for positioning instead of turning |

**Arc configuration:**
- `!frontarc #.#` — Frontal exclusion zone (1.1-260.0 degrees)
- `behindarc #.#` — Rear positioning arc (1.1-260.0 degrees)
- `/stick mod #` — Adjust distance, `/stick #%` — Percentage modifier

### /moveto — Point Navigation

Navigate to a specific location with arrival detection.

| Syntax | Description |
|--------|-------------|
| `/moveto loc Y X [Z]` | Move to coordinates |
| `/moveto id [#]` | Move to spawn ID |
| `/moveto yloc Y` / `xloc X` | Single-axis beeline |
| `dist #` | Arrival distance threshold |
| `usewalk` | Walk when approaching destination |
| `useback` | Walk backward when close |
| `breakonaggro` | Stop if aggro detected |
| `breakonhit` | Stop if attacked |

### /makecamp — Camp System

Establishes a camp location with automatic return and boundary enforcement.

| Feature | Description |
|---------|-------------|
| `/makecamp [on] [radius]` | Camp current location |
| `/makecamp loc Y X` | Camp specific coordinates |
| `/makecamp player [name]` | Dynamic camp on another PC |
| `return` | Force immediate return |
| `mindelay # / maxdelay #` | Return delay (ms) |
| `returnnoaggro` | Only return if not in combat |
| `returnnotlooting` | Don't return while looting |
| `leash [#]` | Force boundary enforcement |
| `scatter` | Randomized return positions |
| `bearing # / scatsize # / scatdist #` | Scatter parameters |

### /circle — Kiting

| Syntax | Description |
|--------|-------------|
| `/circle on [radius]` | Circle current location |
| `/circle loc Y X` | Circle specific point |
| `clockwise/cw` | Clockwise (default) |
| `counterclockwise/ccw` | Counter-clockwise |
| `drunken` | Random interval rotations |
| `backward` | Run backward |

### Stuck Detection

Built-in stuck logic with configurable parameters:
- `diststuck #.##` — Minimum movement distance threshold
- `pulsecheck #` — Pulses for averaging movement rate
- `pulseunstuck #` — Successful pulses before declaring unstuck
- `trytojump` — Include jumping in recovery attempts
- `turnhalf` — Reverse heading after 180 degrees without progress

### Safety Features

- `breakonwarp` — Stop if target warps beyond threshold
- `pauseonwarp` — Pause until target returns
- `breakongate` — Break if target gates
- `breakonsummon` — Disable if summoned beyond threshold
- `breakongm` — Stop if GM enters zone
- `autopause` — Pause during casting, stun, root, sit, feign death
- `mpause` / `mousepause` — Pause on keyboard/mouse input

### Heading Control Modes

- `true` — Actual keypress-based heading (most legit-looking)
- `loose` — Incremental heading adjustments (human-like)
- `fast` — Instant heading via memory write (least legit)

### Relevance to Frostreaver

**What we already have (M3-M4):**
- Navigator FSM covers `/moveto` equivalent
- StuckDetector with escalating recovery (matches MQ2MoveUtils stuck logic)
- MovementPersonality for humanization (matches `loose` heading mode)
- CampManager covers `/makecamp` equivalent
- Combat positioning in `dmft-dll/src/combat/positioning.rs`

**What MQ2MoveUtils adds that we lack:**
- **Full /stick implementation** — Our combat positioning is simpler. MQ2MoveUtils' `behind`, `!front`, `pin`, `snaproll`, arc configuration, and `moveback` are more sophisticated. This is gap #2 from prior research.
- **Healer stick mode** — Suppresses face adjustments in range, only moves when out of range
- **Circle strafing** — Not implemented (low priority for boxing)
- **Player follow** — `/makecamp player` for follow-the-leader; our CampManager is static-location only
- **Safety features** — `breakonwarp`, `breakonsummon`, `breakongm` are good ideas for anti-detection

**Recommended approach:** Enhance our Navigator/Combatant positioning to support MQ2MoveUtils-style arc parameters and stick modes. The `behind`/`!front`/`pin` positioning can be added as enum variants to our existing combat positioning system.

---

## 3. MQ2Cast — Reliable Spell Casting

### Overview

MQ2Cast handles all spell casting, item clicking, and AA activation with automatic retry, fizzle recovery, and state management. It's a dependency of virtually every combat macro.

### Core Command: /casting

**Syntax:** `/casting "Name" [type] [options]`

| Type | Example |
|------|---------|
| Spell by gem | `/casting "Complete Heal" gem1` |
| Spell by ID | `/casting 13 gem4` |
| Item | `/casting "Fungi Tunic" item` |
| Item by slot | `/casting "Clicky" leftear` |
| Alt ability | `/casting "Harm Touch" alt` |

### Key Options

| Option | Description |
|--------|-------------|
| `-maxtries\|#` | Retry up to N times on fizzle/interrupt |
| `-recast\|#` | Cast spell N times total |
| `-kill` | Keep casting until target dies |
| `-targetid\|#` | Target specific spawn before casting |
| `-invis` | Don't cast while invisible |
| `-bandolier\|#` | Equip bandolier set before casting |

### Cast Result States

The plugin tracks 18 distinct casting outcomes:

| Result | Meaning |
|--------|---------|
| `CAST_SUCCESS` | Spell landed |
| `CAST_FIZZLE` | Cast fizzled (retryable) |
| `CAST_COLLAPSE` | Gate collapsed (retryable) |
| `CAST_RESIST` | Target resisted |
| `CAST_IMMUNE` | Target immune |
| `CAST_INTERRUPTED` | Cast was interrupted (retryable) |
| `CAST_ABORTED` | Manually aborted |
| `CAST_NOTREADY` | Spell/gem not ready |
| `CAST_OUTOFMANA` | Insufficient mana |
| `CAST_OUTOFRANGE` | Target out of range |
| `CAST_CANNOTSEE` | No line of sight |
| `CAST_NOTARGET` | No valid target |
| `CAST_STANDING` | Need to be standing |
| `CAST_STUNNED` | Character is stunned |
| `CAST_DISTRACTED` | Character distracted |
| `CAST_INVISIBLE` | Currently invisible |
| `CAST_COMPONENTS` | Missing spell components |
| `CAST_TAKEHOLD` | Spell didn't take hold (already buffed) |

### Status Indicators (during casting)

| Flag | Meaning |
|------|---------|
| `C` | Casting in progress |
| `M` | Memorizing spell |
| `S` | Immobilizing (stopping movement) |
| `T` | Targeting |
| `E` | Item swapped (equipping clicky) |
| `D` | Ducking |
| `F` | Stick paused |
| `A` | Advpath paused |
| `I` | Idle |

### Auto-Features

1. **Fizzle/interrupt recovery** — Auto-retries on CAST_FIZZLE and CAST_COLLAPSE
2. **Immobilization** — Stops movement before casting (required for most spells)
3. **Plugin coordination** — Pauses MQ2MoveUtils stick and AdvPath during casting
4. **Item management** — Equips clickable items from bags, returns them after use
5. **Spell memorization** — Auto-memorizes spells to gem5 if not already loaded
6. **Spell set management** — Save/load/delete named spell loadouts (`/sss`, `/ssm`, `/ssl`, `/ssd`)

### TLO: ${Cast}

| Member | Returns |
|--------|---------|
| `${Cast.Ready}` | Ready to cast |
| `${Cast.Ready[M]}` | Ready to memorize |
| `${Cast.Result}` | Last casting outcome |
| `${Cast.Status}` | Current status flags |
| `${Cast.Effect}` | Currently casting spell name |
| `${Cast.Timing}` | Milliseconds until cast completes |
| `${Cast.Taken}` | Spell didn't take hold |

### Relevance to Frostreaver

**What we already have (M4):**
- `dmft-dll/src/hooks/casting.rs` — Spell casting hooks
- `dmft-dll/src/combat/gcd.rs` — GCD tracker
- `dmft-dll/src/combat/mana.rs` — ManaGovernor
- Direct function calls via `CAST_SPELL` offset (`0x1400D9F20`)

**What MQ2Cast adds that we should implement:**
- **Cast result state machine** — The 18 distinct result states are a gold standard. Our casting should track all of these and react accordingly. This is gap #3 from prior research.
- **Retry logic with maxtries** — Automatic fizzle/interrupt retry with configurable attempts
- **Immobilization before cast** — Stop movement, wait for server acknowledgment, then cast
- **Plugin coordination** — Pause navigation/movement during casting (we partially do this)
- **Spell memorization** — Auto-mem spells; spell set save/load
- **Item clicking** — Equip-use-return for clickable items (not yet implemented)

**Recommended approach:** Build a `CastEngine` struct that wraps our `CAST_SPELL` calls with a state machine tracking the 18 MQ2Cast result states. Add retry logic, immobilization, and coordination with the Navigator FSM.

---

## 4. Combat Macros — KissAssist / MuleAssist / MQ2Melee

### Overview

These are the complete combat automation systems that tie together movement, casting, targeting, and class-specific ability usage.

**Hierarchy:**
- **MQ2Melee** — Plugin (C++) that handles melee ability usage, positioning, and basic combat logic
- **KissAssist** — Macro (MQ2 scripting) that orchestrates all combat: healing, buffing, debuffing, nuking, pulling, CC. The most popular combat macro.
- **MuleAssist** — Fork of KissAssist 10.0.4 with a custom GUI (MAUI), additional features, and continued maintenance

### KissAssist Architecture

KissAssist is a single macro that handles all classes through INI-file configuration. Each character has a `KissAssist_server_name.ini` file.

**Dependencies:** MQ2Cast, MQ2Exchange, MQ2Melee, MQ2MoveUtils, MQ2Rez, MQ2Twist

**Launch:** `/mac kissassist [Role] [AssistName] [AssistHealth%]`

### Roles

| Role | Description |
|------|-------------|
| **Tank** | One per group. Engages targets, uses defensive abilities, maintains aggro |
| **Assist** | Default DPS role. Engages target when MA calls it at configured HP% |
| **Puller** | Pulls mobs to camp. Uses pull spells/abilities, handles singles vs multi-pulls |

### INI Section Structure

KissAssist INI files are organized into numbered ability sections:

**Healing (highest priority):**
- `HealSpell1` through `HealSpellN` — Direct heals with conditions
- `HealOverTimeSpell1-N` — HoT spells
- `PanicHeal` — Emergency heal at critical HP
- Target selection: tank > self > group members by HP threshold

**Buffs:**
- `Buff1` through `BuffN` — Self/group buffs with rebuff timers
- `CombatBuff1-N` — Applied only during combat
- `Aura1-N` — Maintained auras

**Debuffs:**
- `Debuff1-N` — Applied to target (slow, cripple, tash, malo, etc.)
- Priority ordering ensures most important debuffs land first

**Nukes/DPS:**
- `Nuke1-N` — Damage spells with mana thresholds
- `DoT1-N` — Damage over time with min HP conditions
- `MeleeAbility1-N` — Melee combat abilities

**Crowd Control:**
- `MezSpell` — Mezmerize spell
- `MezAE` — AE mez
- `RootSpell` — Root
- Targets: adds (non-assist-target mobs) on extended target window

**Pulling:**
- `PullSpell1-N` — Ranged pull abilities
- Pull logic: target mob → pull spell → run to camp → hand off to tank
- Safety: stops pulling if group HP low, too many mobs, etc.

### Priority System

KissAssist executes checks in priority order each tick:

1. **HolyShit** — Emergency conditions (self HP < threshold, named mob, etc.)
2. **Healing** — Panic heal > direct heal > HoT
3. **Curing** — Remove debuffs from group
4. **Buffing** — Rebuff expired buffs (non-combat only)
5. **Debuffing** — Slow, tash, malo on current target
6. **Crowd Control** — Mez/root adds
7. **DPS** — Nukes, DoTs, melee abilities
8. **Pet management** — Pet buffs, attack commands
9. **Medding** — Sit to regen mana/endurance when idle

### Mana Management

- `MedStart` — HP% threshold to start medding
- `MedCombat` — Allow medding during combat
- Healers continue healing while medding
- Melee characters stop attacking to med
- Nuke spells have individual mana thresholds (don't nuke below X% mana)

### Assist Mechanics

- **Main Assist (MA)** — One character designated as assist target picker
- **Assist HP%** — Don't engage until target below this HP (e.g., 98% for fast response, 80% for safer)
- Characters use `/assist <MA_name>` to acquire target
- `hold` flag prevents target switching during combat

### MuleAssist Additions

- Fork of KissAssist 10.0.4 with GUI configuration (MAUI)
- Raw INI editor with separate save
- Can import/convert KissAssist INI files
- Level-specific INI loading (`MuleAssist_server_name_lvl.ini`)
- Community INI library for sharing configurations

### MQ2Melee Plugin

Handles low-level melee combat:
- Auto-attack management
- Combat ability usage (kicks, bashes, backstab, etc.)
- Positioning (face target, stay behind)
- Disc usage with priority ordering
- Aggro management (stop attacking if not tank and pulling aggro)

### Relevance to Frostreaver

**What we already have (M4):**
- `ClassStrategy` trait with per-class implementations (warrior, cleric, enchanter, generic_dps)
- `HolyShit` conditional ability system
- GCD tracker, ManaGovernor
- PullCycle FSM
- Aggro detection (heading-based)
- Combat coordinator with MA broadcasting

**What KissAssist/MuleAssist add:**
- **INI-based configuration** — Our strategies are compiled Rust code, not configurable at runtime. For 36 characters, we need runtime configuration (INI or TOML per character).
- **Numbered ability priority lists** — KissAssist's `Nuke1`, `Nuke2`, etc. with individual conditions is more flexible than our compiled rotations
- **Cure logic** — Detecting and removing debuffs from group members
- **Extended target window integration** — For CC targeting adds
- **Buff rebuff tracking** — Timer-based rebuffing
- **Pet management** — Pet commands, pet buffs
- **Level-specific configs** — Different ability loadouts as characters level

**Recommended approach:** Add a TOML-based ability configuration system that mirrors KissAssist's INI structure. Each character gets a `config/toons/<name>.toml` with numbered ability sections. The `ClassStrategy` trait implementations can load abilities from config rather than hardcoding.

---

## 5. MQ2Map — Map Overlay

### Overview

MQ2Map is a core MacroQuest plugin that enhances EQ's built-in map window with additional spawn display, filtering, and customization.

### How It Works

MQ2Map hooks into EQ's existing map window (accessible via `/map` in-game). It does **not** create its own rendering surface — it adds map objects (lines, labels, circles) to the native EQ map data structures. This means:

- Spawns appear as colored markers on the standard EQ map
- Labels can show spawn name, level, class, race, HP%, etc.
- Filtering controls which spawn types are visible
- Radius circles show range rings around your character

### Key Commands

**`/mapfilter`** — Controls what appears on the map:

| Filter | Description |
|--------|-------------|
| `NPC` | Show/hide all NPCs |
| `PC` | Show/hide player characters |
| `Corpse` | Show/hide corpses |
| `Ground` | Show/hide ground spawns |
| `Pet` | Show/hide pets |
| `Named` | Show/hide named mobs |
| `Untargetable` | Show/hide untargetable spawns |
| `CastRadius #` | Draw spell range circle |
| `SpellRadius #` | Draw spell effect radius |
| `NormalLabels` | Label format for normal spawns |
| `TargetPath` | Draw path line to target |
| `TargetLine` | Draw direct line to target |

TUI parity: `:mapfilter <npc|pc|corpse|ground|pet|named|untargetable> [on|off]` toggles these categories in the map overlay.

**`/mapshow` / `/maphide`** — Show/hide specific spawns by search criteria. Changes persist until zone reload.

**`/highlight`** — Highlight specific spawns with custom color, size, and pulse effect.

**`/maploc Y X Z`** — Place a marker at coordinates.

**`/mapnames`** — Configure label format. Default `%N` (name). Can include level, class, race, ID, HP%.

**`/mapclick`** — Execute custom commands on map click with modifier keys.

**`/mapactivelayer`** — Switch between 4 map layers (0-3).

### MapSpawn Object

When hovering over map elements:
- Spawn form — returns full spawn data type (name, class, level, HP, etc.)
- Ground form — returns ground item data for looting

### Relevance to Frostreaver

**What we already have (M1 TUI):**
- `dmft/src/tui/ui.rs` — Spawn list table with filtering
- Spawn data reading (name, class, level, position, HP)
- Demo mode on macOS for UI development

**What MQ2Map adds for our TUI map view:**
- **2D map rendering** — We could render spawns as positioned markers on a coordinate grid in the TUI. This is the map view feature already requested.
- **Radius circles** — Show spell range, aggro radius around player
- **Label formatting** — Configurable spawn labels (already partially done in our table)
- **Filtering** — Already implemented in our spawn list; extend to map view
- **Target line** — Draw line from player to target on map
- **Layer system** — Multiple map layers for different data

**Recommended approach:** For TUI, implement a 2D ASCII/Unicode map widget in ratatui that plots spawns by X/Y coordinates. Use the existing spawn data from our M1 memory reading. No EQ rendering hooks needed — our external process approach means we draw in the terminal, not in-game.

---

## 6. Optimal 36-Box Group Composition

### Design Principles

For a 36-box raid force (6 groups of 6), the composition must balance:
- **Self-sufficiency** — Each group should function independently for group content
- **Raid viability** — Combined force can tackle raid targets
- **Automation friendliness** — Some classes are easier to automate than others
- **TLP progression** — Composition should work from Classic through modern expansions

### Recommended 36-Box Composition

#### Group 1 — Main Tank Group
| Slot | Class | Role |
|------|-------|------|
| 1 | **Warrior** | Main Tank — best defensive cooldowns for raids |
| 2 | **Cleric** | Primary healer — Complete Heal rotation |
| 3 | **Cleric** | Backup healer — keeps CH chain alive |
| 4 | **Shaman** | Slow, debuffs, melee buffs, backup heals |
| 5 | **Bard** | Overhaste, mana regen, resist songs for tank |
| 6 | **Enchanter** | Haste, slow backup, mana regen, CC |

#### Group 2 — Off-Tank / Puller Group
| Slot | Class | Role |
|------|-------|------|
| 1 | **Shadow Knight** | Off-tank, puller (FD splitting) |
| 2 | **Cleric** | Healer |
| 3 | **Shaman** | Slow, buffs, backup heals |
| 4 | **Bard** | Pulling songs, overhaste, mana regen |
| 5 | **Monk** | DPS, backup puller (FD) |
| 6 | **Berserker** | Melee DPS (or Rogue pre-Berserker) |

#### Group 3 — Melee DPS Group
| Slot | Class | Role |
|------|-------|------|
| 1 | **Paladin** | Off-tank, group heals, stuns |
| 2 | **Cleric** | Healer |
| 3 | **Bard** | Overhaste, melee DPS songs |
| 4 | **Monk** | DPS |
| 5 | **Rogue** | DPS (backstab, positional) |
| 6 | **Berserker** | DPS (or Ranger) |

#### Group 4 — Caster DPS Group
| Slot | Class | Role |
|------|-------|------|
| 1 | **Enchanter** | CC, haste, mana regen, tash |
| 2 | **Cleric** | Healer |
| 3 | **Wizard** | Burst caster DPS |
| 4 | **Wizard** | Burst caster DPS |
| 5 | **Magician** | Sustained DPS, pet tank for splits, CotH utility |
| 6 | **Druid** | Backup heals, ports, DPS, snare |

#### Group 5 — Pet / Utility Group
| Slot | Class | Role |
|------|-------|------|
| 1 | **Beastlord** | Melee DPS + pet, slow, buffs |
| 2 | **Shaman** | Healer, slow, buffs |
| 3 | **Magician** | Pet DPS, CotH |
| 4 | **Magician** | Pet DPS, CotH |
| 5 | **Necromancer** | DoT DPS, FD, mana tap, undead CC |
| 6 | **Bard** | Songs, pulling backup |

#### Group 6 — Flex / Economy Group
| Slot | Class | Role |
|------|-------|------|
| 1 | **Shadow Knight** | Tank, puller, FD |
| 2 | **Cleric** | Healer |
| 3 | **Enchanter** | CC, charm (economy), mana regen |
| 4 | **Necromancer** | DPS, FD, utility |
| 5 | **Ranger** | DPS, tracking, headshot (economy farming) |
| 6 | **Druid** | Ports, backup heals, snare, DPS |

### Class Totals (36 characters)

| Class | Count | Justification |
|-------|-------|---------------|
| Warrior | 1 | Main tank for raids |
| Shadow Knight | 2 | Off-tank, FD pull splitting |
| Paladin | 1 | Group heals, stuns, off-tank |
| Cleric | 5 | Healing backbone (raid CH rotation needs 3-4 minimum) |
| Shaman | 3 | Best slow, melee buffs, versatile healer |
| Druid | 2 | Ports, backup heals, snare, economy farming |
| Bard | 4 | Force multiplier — haste, mana regen, pulling, resist songs |
| Enchanter | 3 | CC (essential Classic-PoP), haste, mana regen, tash |
| Wizard | 2 | Burst DPS, ports |
| Magician | 3 | Consistent DPS, CotH for logistics, pet tanking |
| Necromancer | 2 | DoT DPS, FD, mana utility |
| Monk | 2 | DPS, FD backup puller |
| Rogue | 1 | Positional DPS, traps |
| Berserker | 2 | Melee DPS (available from GoD expansion) |
| Beastlord | 1 | Melee DPS + slow + pet |
| Ranger | 1 | Tracking, headshot farming, DPS |

### Key Principles

1. **Clerics are non-negotiable** — 5 clerics ensures CH chains for raids and every group has healing
2. **Bards are force multipliers** — 4 bards means most groups get overhaste + mana regen
3. **Enchanters essential early** — 3 enchanters for Classic/Kunark/Velious CC-heavy content
4. **Shamans over druids for slow** — Shaman slow is the most important debuff in early EQ
5. **FD classes for pulling** — SK and Monk for reliable FD splitting
6. **CotH logistics** — 3 mages means fast raid assembly via Call of the Hero
7. **Pre-Berserker** — Before Gates of Discord, replace Berserkers with Rogues or more Monks
8. **Automation priority** — Caster DPS (Wizard, Mage, Necro) is easier to automate than positional melee

### TLP Expansion Adjustments

| Era | Adjustment |
|-----|------------|
| **Classic** | No Berserkers or Beastlords — replace with Monks, Rogues, Rangers |
| **Kunark** | Add Beastlord if available on server ruleset |
| **Velious** | Full composition available minus Berserker |
| **PoP** | All classes available, Berserker still missing |
| **GoD+** | Full 36-box composition as listed |

---

## 7. Gap Analysis vs Frostreaver M1-M4

### Feature Comparison Matrix

| Feature | MQ2 Plugin | Frostreaver Status | Priority |
|---------|------------|-------------------|----------|
| **Navmesh pathfinding** | MQ2Nav (RecastNavigation) | M3: Waypoint-based only | **Critical** |
| **Stick/positioning** | MQ2MoveUtils /stick | M4: Basic combat positioning | **High** |
| **Follow player** | MQ2MoveUtils /makecamp player | M3: Static camp only | **High** |
| **Robust casting** | MQ2Cast (18 result states) | M4: Basic CastSpell call | **High** |
| **Cast retry/fizzle** | MQ2Cast -maxtries | Not implemented | **High** |
| **Spell memorization** | MQ2Cast /memorize | Not implemented | Medium |
| **Item clicking** | MQ2Cast item management | Not implemented | Medium |
| **Camp with scatter** | MQ2MoveUtils scatter | M3: Basic CampManager | Medium |
| **Circle strafing** | MQ2MoveUtils /circle | Not implemented | Low |
| **INI/config per toon** | KissAssist INI system | Compiled Rust strategies | **High** |
| **Ability priority lists** | KissAssist numbered abilities | HolyShit system (partial) | **High** |
| **Cure logic** | KissAssist cure section | Not implemented | Medium |
| **Buff rebuff tracking** | KissAssist buff timers | Not implemented | Medium |
| **Pet management** | KissAssist pet section | M4: IssuePetCommand only | Medium |
| **Map overlay** | MQ2Map (in-game) | TUI: Spawn list (no map) | Medium |
| **3D debug overlay** | MQ2Nav overlay | N/A (external TUI) | None |
| **Extended target CC** | KissAssist mez adds | M4: Enchanter CC (basic) | Medium |
| **GM detection** | MQ2MoveUtils breakongm | Not implemented | Low |
| **Warp detection** | MQ2MoveUtils breakonwarp | Not implemented | Low |

### Top 5 Implementation Priorities

1. **Navmesh pathfinding** — Load MQ2Nav `.navmesh` files, port Detour query to Rust or FFI. Replaces pre-recorded waypoints with dynamic pathing. Enables: door handling, obstacle avoidance, arbitrary destination navigation.

2. **Robust casting engine** — Build `CastEngine` with 18-state result tracking, retry logic, immobilization, item clicking. Replaces raw `CastSpell` calls. Enables: reliable automation across 36 clients.

3. **Enhanced stick/positioning** — Add `behind`/`!front`/`pin`/`snaproll` positioning modes to combat system. Add `loose` heading with arc configuration. Enables: proper melee DPS positioning, tank face-tanking.

4. **Runtime ability configuration** — TOML per-character configs with numbered ability priority lists. Move from compiled `ClassStrategy` implementations to data-driven approach. Enables: 36-character customization without recompilation.

5. **Player follow** — Extend CampManager with dynamic player-following mode (camp anchored to another character's position). Enables: formation movement, raid assembly.

### Architecture Recommendations

**For navmesh:** Create `dmft-dll/src/nav/mesh.rs` that:
- Loads MQ2Nav v5 `.navmesh` files (zlib decompress → protobuf deserialize → dtNavMesh)
- Wraps Detour's `dtNavMeshQuery` for path queries
- Feeds waypoints to existing Navigator FSM
- Consider: `recast-rs` crate or raw FFI to RecastNavigation C++ lib

**For casting:** Create `dmft-dll/src/combat/cast_engine.rs` that:
- State machine: Idle → Immobilizing → Targeting → Casting → Recovering
- Tracks all 18 MQ2Cast result states via game memory reads
- Retry logic with configurable max attempts
- Coordinates with Navigator (pause movement during cast)
- Handles item equip/use/return cycle

**For configuration:** Create `dmft-common/src/ability_config.rs` that:
- Defines TOML schema mirroring KissAssist INI sections
- Per-character files: `config/toons/<server>_<name>.toml`
- Sections: `[heals]`, `[buffs]`, `[debuffs]`, `[nukes]`, `[dots]`, `[melee]`, `[cc]`, `[pulls]`
- Each ability: name, spell_id, gem, priority, conditions (hp_threshold, mana_min, etc.)
- Hot-reloadable via file watcher

---

## Sources

- [MQ2Nav GitHub — brainiac/MQ2Nav](https://github.com/brainiac/MQ2Nav)
- [MQ2Nav Documentation — MMOBugs Wiki](https://www.mmobugs.com/wiki/index.php/MQ2Nav)
- [MQ2Nav — RedGuides](https://www.redguides.com/community/resources/mq2nav.146/)
- [MQ2Nav Meshes — mqmesh.com](https://mqmesh.com/)
- [MQ2MoveUtils — MMOBugs Wiki](https://www.mmobugs.com/wiki/index.php/MQ2MoveUtils)
- [MQ2MoveUtils v11 FAQ — MacroQuest Docs](https://docs.macroquest.org/plugins/community-plugins/mq2moveutils/mq2moveutils-v11-faq/)
- [MQ2Cast — MacroQuest Docs](https://docs.macroquest.org/plugins/community-plugins/mq2cast/)
- [MQ2Cast GitHub — RedGuides/MQ2Cast](https://github.com/RedGuides/MQ2Cast)
- [KissAssist Instructions — RedGuides](https://www.redguides.com/community/threads/kissassist-instructions-settings-info.26002/)
- [KissAssist Wiki — RedGuides](https://www.redguides.com/wiki/KissAssist)
- [MuleAssist — RedGuides](https://www.redguides.com/community/resources/muleassist.286/)
- [MQ2Map — MacroQuest Docs](https://docs.macroquest.org/plugins/core-plugins/map/)
- [36-Toon Raid Boxing — RedGuides](https://www.redguides.com/community/threads/raid-box-36-toons.78908/)
- [Best 6-Box TLP — RedGuides](https://www.redguides.com/community/threads/best-6-box-setup-tlp-live.87518/)
- [Group Composition Discussion — RedGuides](https://www.redguides.com/community/threads/group-composition-discussion-rehash.93987/)
