# MacroQuest Ecosystem & Integration Research

Consolidated reference for MQ2 core, KissAssist, RedGuides, RGMercs, and plugin ecosystem research. This page aggregates the complete external-reference analysis for TextQuest launch-window automation parity.

---

## Table of Contents

1. [MQ2 Core Deep Dive](#mq2-core-deep-dive)
2. [Combat Macros: KissAssist & RGMercs](#combat-macros-kissassist--rgmercs)
3. [RedGuides Ecosystem](#redguides-ecosystem)
4. [MQ2 Plugin Survey](#mq2-plugin-survey)
5. [Gap Analysis vs TextQuest](#gap-analysis-vs-textquest)
6. [Launch Ordering & Recommendations](#launch-ordering--recommendations)

---

## MQ2 Core Deep Dive

### MQ2Nav — Navigation Mesh Pathfinding

**Overview:** MQ2Nav is a navmesh-powered pathfinding plugin built on the **RecastNavigation** library (the same library used by Unity, Unreal Engine, and most AAA game engines for NPC pathing).

**Components:**

- **MQ2Nav.dll** — The MacroQuest plugin that loads navmeshes at runtime, computes paths, and drives character movement
- **MeshGenerator.exe** — A standalone tool (separate Win32/x64 builds) for creating and editing zone navigation meshes

**Navmesh Format:**

- Zone meshes stored as `.navmesh` files (one per zone, e.g., `gfaydark.navmesh`)
- Custom binary format with versioning (current v5, backward-compatible with v4)
- v5 added decompressed data size in the header for faster loading
- Meshes compressed with **zlib**; serialized with **Protocol Buffers**
- Community mesh packs available at [mqmesh.com](https://mqmesh.com/) covering most zones
- Stored in `Resources/MQ2Nav/` directory

**Path Computation:**

- Uses RecastNavigation's **Detour** library for A\* pathfinding on the navmesh
- `dtNavMeshQuery` finds the nearest polygon, then computes a corridor of polygons to the destination
- Path is smoothed into a series of waypoints using the string-pulling algorithm (funnel algorithm)
- Off-mesh connections enable traversal through doors, up stairs, and across teleport points

**Key Commands:**

- `/nav target` — Navigate to current target
- `/nav loc X Y Z` — Navigate to coordinates
- `/nav door` — Navigate to nearest door and open it
- `/nav item [click]` — Navigate to ground item, optionally pick up
- `/nav waypoint <name>` — Navigate to saved waypoint
- `/nav stop` / `/nav pause` / `/nav reload` — Control navigation
- `/nav recordwaypoint <name> <tag>` — Save current position as waypoint

**Macro Data (TLO):**

- `${Navigation.Active}`, `${Navigation.MeshLoaded}`, `${Navigation.PathExists}`, `${Navigation.PathLength}`, `${Navigation.Velocity}`

**Relevance to TextQuest:**

- TextQuest M3 has waypoint-based pathfinding; we lack dynamic navmesh path computation.
- Recommended approach: Build a Rust `.navmesh` loader that reads MQ2Nav's v5 format, wraps `dtNavMeshQuery` via FFI (or a Rust port of Detour), and feeds waypoints to our existing Navigator FSM.

### MQ2MoveUtils — Movement: Stick, Follow, Camp

**Core Commands:**

| Command                   | Purpose                                                                |
| ------------------------- | ---------------------------------------------------------------------- |
| `/stick [distance]`       | Maintain distance and angle relative to target (melee positioning)     |
| `/moveto loc Y X [Z]`     | Navigate to specific location with arrival detection                   |
| `/makecamp [on] [radius]` | Establish camp location with automatic return and boundary enforcement |
| `/circle on [radius]`     | Kiting in circles around a location                                    |

**Stick Positioning Modes:**

- `behind` — Position at target's rear arc
- `!front` — Anywhere except frontal arc
- `front` — Frontal arc (for tanking)
- `pin` — Side positioning
- `behindonce` — Initial rear position, then distance only
- `snaproll [left|right|face|rear]` — Run behind target, rotate to specified direction

**Key Modifiers:**

- `loose` — Human-like incremental turning (not instant snapping)
- `truehead` — Actual keypress heading changes (not memory writes)
- `healer` — Suppress face adjustments while in stick range
- `hold` — Keep sticking to current target despite retargeting
- `moveback` — Walk backward if target approaches closer than distance
- `randomize` — Random arc values for `behind` and `!front`

**Stuck Detection:**

- `diststuck #.##` — Minimum movement distance threshold
- `pulsecheck #` — Pulses for averaging movement rate
- `trytojump` — Include jumping in recovery attempts
- `turnhalf` — Reverse heading after 180 degrees without progress

**Relevance to TextQuest:**

- We have Navigator FSM covering `/moveto` equivalent and StuckDetector with escalating recovery
- We lack: full `/stick` implementation with arc configuration, healer stick mode, circle strafing, player follow (`/makecamp player`)
- Recommended approach: Enhance our Navigator/Combatant positioning to support MQ2MoveUtils-style arc parameters and stick modes.

### MQ2Cast — Reliable Spell Casting

**Core Command:** `/casting "Name" [type] [options]`

**Types:**

- Spell by gem: `/casting "Complete Heal" gem1`
- Spell by ID: `/casting 13 gem4`
- Item: `/casting "Fungi Tunic" item`
- Item by slot: `/casting "Clicky" leftear`
- Alt ability: `/casting "Harm Touch" alt`

**Cast Result States** (18 distinct outcomes):

- `CAST_SUCCESS` — Spell landed
- `CAST_FIZZLE` — Cast fizzled (retryable)
- `CAST_COLLAPSE` — Gate collapsed (retryable)
- `CAST_RESIST`, `CAST_IMMUNE` — Target resisted or immune
- `CAST_INTERRUPTED` — Cast was interrupted (retryable)
- `CAST_ABORTED` — Manually aborted
- `CAST_NOTREADY`, `CAST_OUTOFMANA`, `CAST_OUTOFRANGE`, `CAST_CANNOTSEE`, `CAST_NOTARGET` — Various preconditions
- `CAST_STANDING`, `CAST_STUNNED`, `CAST_DISTRACTED`, `CAST_INVISIBLE` — Character state issues
- `CAST_COMPONENTS` — Missing spell components
- `CAST_TAKEHOLD` — Spell didn't take hold (already buffed)

**Auto-Features:**

1. **Fizzle/interrupt recovery** — Auto-retries on CAST_FIZZLE and CAST_COLLAPSE
2. **Immobilization** — Stops movement before casting
3. **Plugin coordination** — Pauses MQ2MoveUtils stick and AdvPath during casting
4. **Item management** — Equips clickable items from bags, returns them after use
5. **Spell memorization** — Auto-memorizes spells to gem5 if not already loaded
6. **Spell set management** — Save/load/delete named spell loadouts

**Relevance to TextQuest:**

- We have spell casting hooks and GCD tracker; we lack the 18-state result machine and fizzle/interrupt retry logic
- Recommended approach: Build a `CastEngine` struct that wraps our `CAST_SPELL` calls with a state machine tracking all MQ2Cast result states. Add retry logic, immobilization, and coordination with the Navigator FSM.

---

## Combat Macros: KissAssist & RGMercs

### KissAssist Macro

**Overview:** The most popular free automation macro for MacroQuest. Single macro file with per-character INI configuration. Runs a continuous priority loop: healing > cures > aggro > DPS > buffs > pulling > meditation.

**Startup Syntax:**

```
/mac kissassist                          -- default, uses INI role
/mac kissassist puller                   -- start as puller
/mac kissassist assist TankName          -- assist specific character
/mac kissassist [Role] [AssistName] [HP%]
```

**Roles:** Assist | Tank | Puller | PullerTank | PetTank | PullerPetTank | Hunter | HunterPetTank | Petassist | Manual

**INI Structure:**

- **Healing:** `HealSpell1-N`, `HealOverTimeSpell1-N`, `PanicHeal` with HP thresholds
- **Buffs:** `Buff1-N`, `CombatBuff1-N`, `Aura1-N` with rebuff timers
- **Debuffs:** `Debuff1-N` — applied to target (slow, cripple, tash, malo, etc.)
- **Nukes/DPS:** `Nuke1-N`, `DoT1-N`, `MeleeAbility1-N` with mana thresholds
- **Crowd Control:** `MezSpell`, `MezAE`, `RootSpell` — targets adds from extended target window
- **Pulling:** `PullSpell1-N` with pull logic (target > pull spell > run to camp > hand off to tank)
- **Medding:** `MedStart` and `MedStop` thresholds for mana regeneration

**Pull Cycle:**

1. Idle at camp
2. Scan for mobs within `MaxRadius` (horizontal) + `MaxZRange` (vertical), filtered by:
   - `MobsToPull` list (partial names or `ALL`)
   - `MobsToIgnore` list
   - `PullLevel` range (min/max)
   - `PullArcWidth` (directional cone)
   - Line-of-sight checks
3. Pull the mob using `PullWith` (spell, item, or melee approach)
4. Return to camp position
5. Optional chain pulling when current target drops below `ChainPullHP`%

**Camp System:**

- `/camphere` resets camp location or toggles ReturnToCamp
- `CampRadius=80` — operational zone around camp
- `CampRadiusExceed=400` — hard leash
- `ReturnToCamp=1` — return to camp after combat
- `ChaseAssist=0` or `1` — follow MA instead of camping

**Assist Mechanics:**

- **Main Assist (MA)** — One character designated as assist target picker
- **Assist HP%** — Don't engage until target below this HP (e.g., 98% for fast response, 80% for safer)
- `/assist <MA_name>` — target acquisition
- `hold` flag prevents target switching during combat

**Relevance to TextQuest:**

- We have `ClassStrategy` trait, GCD tracker, ManaGovernor, PullCycle FSM, combat coordinator with MA broadcasting
- We lack: INI-based configurable abilities, numbered ability priority lists, cure logic, extended target window integration, buff rebuff tracking, pet management, level-specific configs
- Recommended approach: Add a TOML-based ability configuration system that mirrors KissAssist's INI structure. Each character gets a `config/toons/<name>.toml` with numbered ability sections.

### RGMercs Analysis

**Overview:** Lua-based automation framework running inside MacroQuest. Structured as modular combat system with data-driven rotations.

**Core Architecture:**

```
rgmercs/
├── init.lua              # Entry point, main loop
├── modules/              # Feature modules (class, pull, mez, move, charm, loot)
├── utils/                # Shared utilities (combat, casting, movement, rotation, targeting)
├── class_configs/        # Per-class priority tables (Live, EMU variants)
├── ui/                   # ImGui-based overlay UI
├── lib/dannet/           # Cross-client communication
└── namedlist/            # Named mob detection lists per server
```

**Key Design Principles:**

1. **Data-driven rotations** — Class behavior defined entirely in Lua tables
2. **Resolved action maps** — At startup, spell/ability "sets" are resolved to best available version for character's level
3. **Condition-gated execution** — Every rotation entry has a `cond` function that gates execution
4. **Module isolation** — Each feature (pull, mez, charm, movement, class combat) is separate

**Movement System:**

| Feature             | Details                                                                                               |
| ------------------- | ----------------------------------------------------------------------------------------------------- |
| **Stick**           | `DoStick(targetId)` with tank (10 units, front) vs DPS (behind, moveback, 20+ units for tall targets) |
| **Navigation**      | `DoNav()` wraps MQ2Nav for mesh-based pathfinding; `NavInCombat()` for combat navigation              |
| **Circle-strafe**   | `NavAroundCircle()` — 36 positions (10° increments) to find valid navigable point with LoS            |
| **Camp/Chase**      | Tether to location or follow designated chase target with configurable distance thresholds            |
| **Stuck detection** | Tracks position changes, detects when stuck                                                           |

**Combat System:**

| Component             | Purpose                                                                                                   |
| --------------------- | --------------------------------------------------------------------------------------------------------- |
| **Main Assist (MA)**  | Multi-tier selection: assist list → raid assist → group main assist → self-fallback                       |
| **Target Engagement** | HP threshold check, range check, stance management, class-specific handling (rogue backstab opener, etc.) |
| **Target Scanning**   | XTarget list within radius/zradius with priority (named vs non-named, lowest vs highest HP%)              |
| **Aggro Scan**        | Detects mobs not fully aggro'd (< 100% aggro, not mezzed)                                                 |
| **Safe Targeting**    | Skips mobs already fighting other groups                                                                  |

**Casting System:**

| Spell Type  | Method                                                    |
| ----------- | --------------------------------------------------------- |
| Spells      | `UseSpell()` with gem memorization                        |
| Songs       | `UseSong()` for bard casting                              |
| Disciplines | `UseDisc()` with timer check                              |
| AAs         | `UseAA()` alternate advancement                           |
| Abilities   | `UseAbility()` combat abilities (kick, bash, taunt, etc.) |
| Items       | `UseItem()` item clickies                                 |

**Buff Checking:**

- `LocalBuffCheck(spellId)` — checks if buff needed, not already active, stacks with existing, checks triggers
- `LocalPetBuffCheck()`, `SelfBuffCheck()`, `GroupBuffCheck()` — convenience wrappers
- `DanNet` integration for remote group member buff coordination

**Rotation Engine:**

The core execution model:

```
For each entry in rotation table:
  1. Check if entry is enabled
  2. Check rotation-level condition
  3. Check chase/follow priority
  4. For each target in target list:
     a. Test entry condition
     b. If pass → execute entry
     c. If group spell → break after first target
  5. Track steps taken, break if step limit reached
```

**Rotation Types:**

- `RotationOrder` — ordered list of rotation groups with conditions and target IDs
- `Rotations` — named tables of entries (spell, song, disc, AA, ability, item, clickyitem, customfunc)
- `HealRotationOrder` + `HealRotations` — separate heal priority cascade for healers

**Pull Module:**

- Full pull state machine with 11 states: PULL_IDLE → PULL_GROUPWATCH_WAIT → PULL_SCAN → PULL_NAV_TO_TARGET → PULL_PULLING → PULL_WAITING_ON_MOB → PULL_RETURN_TO_CAMP
- Pull abilities: Pet pull, taunt, auto attack, ranged, kick, face pull, item pulls
- Pull modes: Normal, Chain, Hunt, Farm
- Features: Waypoint paths for hunt/farm, pull radius with camp return, group readiness checking

**Mez & Charm:**

- **Mez Module:** ST mez, AE mez, AA mez, mez immune tracking, duration-based re-mez logic
- **Charm Module:** Charm target selection, charm break detection, re-charm logic

**Per-Class Configs:**

- 16 classes all supported with per-mode configurations (Tank, DPS, Heal, Hybrid, etc.)
- Level-gated ability loading via `load_cond`
- Each class has ItemSets, AbilitySets, HelperFunctions, RotationOrder, Rotations, HealRotations, SpellList, DefaultConfig

**Relevance to TextQuest:**

- Biggest gap: RGMercs is data-driven with multi-step rotation execution; we pick one action per frame
- RGMercs has dynamic spell memorization; we don't manage spell gems yet
- RGMercs has sophisticated MA target scan with named/trash priority and aggro percentage tracking
- Recommended approach: Port core rotation engine concepts into native TextQuest; data-driven ability lists with conditions per ability.

---

## RedGuides Ecosystem

### RedGuides Very Vanilla (VV)

**What It Is:** Pre-compiled, auto-updating distribution of MacroQuest. Bundles ~158 plugins, 280+ macros, and 251 Lua scripts into a turnkey package.

**Core Features:**

- **Auto-login:** MQ2AutoLogin — log all characters with one click
- **Navigation:** MQ2Nav — mesh-based pathfinding per zone
- **Combat automation:** RGMercs or KissAssist macros, or CWTN plugins
- **Spawn notifications:** MQ2SpawnMaster for rare/named mob alerts
- **AA automation:** Automated AA spending
- **DPS metering:** Custom DPS output
- **Full scripting:** Macro + Lua engines for custom automation

**Subscription Tiers:**

| Tier           | Cost             | Includes                                                                                    |
| -------------- | ---------------- | ------------------------------------------------------------------------------------------- |
| Level 1 (Free) | $0               | Core MQ engine, 69 public plugins, 90 public Lua scripts                                    |
| Level 2        | ~$6.65-10/mo     | VV auto-updater, 89 private plugins, 280 macros (KissAssist, RGMercs), MQ2Nav, MQ2AutoLogin |
| CWTN Plugins   | $30/yr per class | Per-class combat automation (on top of Level 2)                                             |

### CWTN Class Plugins

**Overview:** Premium, per-class combat automation plugins for MacroQuest. Winner of RedGuides EQ Software Awards 2019-2021.

**Supported Classes:** All 16 (Warrior, Shadowknight, Paladin, Cleric, Druid, Shaman, Enchanter, Bard, Wizard, Magician, Necromancer, Ranger, Rogue, Monk, Berserker, Beastlord)

**Operating Modes:**

| Mode | Name         | Behavior                                                       |
| ---- | ------------ | -------------------------------------------------------------- |
| 0    | Manual       | You drive movement, plugin does DPS rotation when engaged      |
| 1    | Assist       | Camp at position, assist MA, return to camp when idle, sit/med |
| 2    | ChaseAssist  | Follow MA around, assist on targets                            |
| 3    | Vorpal       | Assist MA, no camp/chase logic                                 |
| 4    | Tank         | Camp at position, tank incoming mobs                           |
| 5    | PullerTank   | Pull mobs to camp, then tank them                              |
| 6    | PullerAssist | Pull mobs to camp, then assist MA                              |
| 7    | SicTank      | Camp-based tanking, no sit/rest                                |
| 8    | HunterTank   | Roam and kill autonomously                                     |

**Automatic Ability Discovery:**

- On `/reload`, plugin scans all available spells, AAs, disciplines, tomes
- Automatically selects the best rank of each ability line
- Fires abilities via priority queue with internal timers
- Context-aware: single target vs AoE, burn vs sustain, defensive cooldowns by HP threshold
- **Zero config** — no INI rotation editing needed (unlike KissAssist)

**Auto-XP Camp Loop Design:**

**Group Composition (6-Box):**

| Slot | Role    | Class       |
| ---- | ------- | ----------- |
| 1    | Tank/MA | Warrior     |
| 2    | Healer  | Cleric      |
| 3    | CC      | Enchanter   |
| 4    | DPS     | Rogue/Monk  |
| 5    | DPS     | Wizard/Mage |
| 6    | DPS     | Ranger/Bard |

**State Machine: Camp Loop**

```
IDLE (all at camp, medding)
  ↓ [mana > MedStop for all casters]
PULL (tank searches + pulls)
  ↓ [mob arrives at camp]
FIGHT (group engages)
  ↓ [mob dead]
LOOT (loot corpse)
  ↓ [loot complete]
BUFF (rebuff if needed)
  ↓ [buffs checked]
MED (sit if mana low)
  ↓ [mana > threshold OR no casters low]
IDLE (loop back to PULL)
```

---

## MQ2 Plugin Survey

### Findings by Plugin Area

#### 1. `MQ2Bzsrch` — Bazaar Search

**What Matters:**

- The plugin detours `CBazaarSearchWnd::HandleSearchResults`, reads the serialized result buffer into `BazaarSearchItem`, and enriches each item with trader names
- Useful part for TextQuest is not the `/bzsrch` command surface, but the data path

**What Doesn't Fit:**

- Depends on MQ2 plugin runtime, MQ2 type system, and MQ2 window helpers
- Actively manipulates bazaar UI widgets (e.g., `BZR_QueryButton`, `BZR_ItemNameInput`)

**Recommendation:**

- Treat `MQ2Bzsrch` as a structural reference for result parsing and window layout assumptions
- Implement the TextQuest version as a passive DLL reader and expose the data over the existing shared-memory plus IPC path
- Keep any future operator surface in the TUI or web UI, not as an MQ2-style command parser

#### 2. `Bazaar.mac` — Automated Bazaar Repricing

**What Matters:**

- The macro as an automated bazaar updater that adjusts trader or buyer prices and can export CSV price logs
- Proves operational value of price intelligence capture and trader repricing automation

**What Doesn't Fit:**

- This is macro-layer automation, not a reusable low-level bazaar data engine
- Assumes MQ2 macro execution, direct bazaar-window driving, and trader-specific flows that TextQuest does not expose today

**Recommendation:**

- Reuse the operator workflow ideas, especially price-history export and comparison reporting
- Do not port the macro
- Sequence the work behind passive bazaar capture and durable chat-based price monitoring

#### 3. Epic Quest Automation

**What Matters:**

- Real ecosystem: `Epic Laziness` (Lua repo layout), `EpicReq.mac` (macro lineage)
- Encode quest order, item checks, and movement/dialog expectations

**What Doesn't Fit:**

- The automation is quest-specific and brittle by nature
- TextQuest does not yet have a generalized quest-scenario execution layer

**Recommendation:**

- Do not build a generic "epic plugin host"
- Open a focused follow-up issue to define a quest-scenario adapter and select the first quest worth porting
- Use the external scripts as runbooks and scenario fixtures, not as code to embed

#### 4. Leveling and Combat Macros (KissAssist, RGMercs, MuleAssist)

**What Matters:**

- Already solve the operator problem TextQuest cares about most before launch: stable assist, pulling, camp control, and recoverable group automation
- Highest-value reusable content is:
  - Operator command semantics
  - Pull and camp state visibility
  - Configurable role and threshold surfaces
  - "Why am I waiting?" explanations for combat and camp loops

**Current TextQuest Position:**

- Already has the right architectural bones: combat and camp-loop orchestration, puller FSMs, route planning, authenticated multi-client IPC, native TUI and planned web UI
- Remaining gap is translation quality, not total absence

**Recommendation:**

- Continue adapting the workflow model into native TextQuest controls and state reporting
- Do not spend time building compatibility with the MQ2 macro language itself
- This is the most launch-relevant area because it directly governs leveling throughput, wipe avoidance, and operator attention load

#### 5. Charm Management

**What Matters:**

- External tools clearly treat charm as a first-class automation concern
- TextQuest is already partway there: charm-break detection and crowd-control response scaffolding exists

**What Fits Well:**

- Clean native adaptation candidate because the existing TextQuest control boundary is already in-process and stateful
- External material mainly contributes safety patterns:
  - Detect the break quickly
  - Interrupt or stun if needed
  - Retry charm with guardrails
  - Fall back to kill mode if the loop fails

**Recommendation:**

- Keep the implementation in the DLL and camp or CC state machine
- Use external charm workflows only to validate retry limits, fallback rules, and operator override expectations

---

## Gap Analysis vs TextQuest

### What External Tools Have That We Don't

| Feature                               | External                                | TextQuest Status              | Priority          |
| ------------------------------------- | --------------------------------------- | ----------------------------- | ----------------- |
| **Data-driven rotation tables**       | Full Lua/INI table system               | Hardcoded `select_spell()`    | HIGH              |
| **Spell resolution (best available)** | Auto-picks best spell per level         | Static config list            | HIGH              |
| **Dynamic spell memorization**        | Swap gems on the fly                    | Not implemented               | MEDIUM            |
| **Condition-gated abilities**         | Rich `cond` functions per entry         | Basic priority selection      | HIGH              |
| **Multi-rotation framework**          | Downtime, Combat, Emergency, Burn, Heal | Single `select_spell()`       | HIGH              |
| **Buff stacking checks**              | Trigger analysis, no-stack detection    | Not implemented               | MEDIUM            |
| **Named mob priority**                | Named/trash target preference           | Not implemented               | LOW               |
| **Aggro % scanning**                  | XTarget aggro detection                 | Not implemented               | HIGH for tanks    |
| **Safe targeting**                    | Skip mobs fighting other groups         | Not implemented               | MEDIUM            |
| **Circle strafing**                   | `NavAroundCircle()`                     | Not implemented               | LOW               |
| **Bard song weaving**                 | `doFullRotation` + song types           | Basic `twist.rs` exists       | HIGH              |
| **Pull modes** (chain/hunt/farm)      | Full FSM with waypoints                 | Basic puller FSM              | MEDIUM            |
| **Mez immune tracking**               | Persistent immune list                  | Not implemented               | MEDIUM            |
| **Charm automation**                  | Full charm module                       | Not implemented               | LOW for TLP start |
| **Clicky item management**            | Cooldown tracking + rotation            | Not implemented               | LOW               |
| **Cross-group heal coordination**     | Via DanNet                              | IPC exists but no arbitration | HIGH for 36-box   |
| **Group readiness checking**          | GroupWatch before pulls                 | Not implemented               | MEDIUM            |

### What TextQuest Has That External Tools Don't

- Native Rust DLL injection (unknown to detection signature database)
- Authenticated IPC with encrypted multi-client coordination
- TUI dashboard with real-time state visibility
- Planned web backend for remote operator control

---

## Launch Ordering & Recommendations

### Priority Sequence

1. **Leveling and combat workflow parity** (HIGHEST)
   - Affects the main pre-launch leveling loop every session
   - Most of the work is already tracked; survey confirms those issues are the right launch focus

2. **Charm automation** (HIGH)
   - High payoff for charm-based leveling groups
   - Low marginal architecture risk because the CC foundation already exists

3. **Passive bazaar intelligence** (MEDIUM)
   - Useful for the early economy window
   - Safest value is in passive capture and later reporting rather than active query spam

4. **Epic quest automation** (MEDIUM)
   - Valuable, but too quest-specific to outrank combat, pull, charm, or bazaar intelligence

5. **Direct `Bazaar.mac`-style repricing automation** (DEFER)
   - Defer until passive bazaar capture, logging, and trader workflows prove worth the operator complexity

### Final Recommendation

The launch-ready path is **selective borrowing:**

- Borrow `MQ2Bzsrch` data structures and result-shape assumptions
- Borrow `Bazaar.mac` logging ideas
- Borrow `KissAssist` / `RGMercs` operator semantics
- Borrow charm safety patterns
- **But keep every implementation native to TextQuest's DLL plus orchestrator architecture**

**See also:** [Research-MQ2-Parity-Matrix.md](Research-MQ2-Parity-Matrix.md) for the canonical feature comparison table.

---

## Sources

- MQ2 Wiki & Source: https://github.com/macroquest/macroquest
- RedGuides: https://www.redguides.com/
- RGMercs Github: https://github.com/DerpleDude/rgmercs
- KissAssist Docs: https://www.redguides.com/docs/projects/kissassist/
- Project 1999 Wiki: https://wiki.project1999.com/
- EQProgression: https://www.eqprogression.com/
