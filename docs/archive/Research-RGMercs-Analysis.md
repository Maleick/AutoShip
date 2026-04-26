# RGMercs Analysis & MQ2 Plugin Ecosystem Research

> Research date: 2026-04-04
> Source: https://github.com/DerpleDude/rgmercs (v2551+)
> Authors: Derple, Algar

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Core Module Breakdown](#core-module-breakdown)
3. [Rotation & Priority System](#rotation--priority-system)
4. [Class Config Structure](#class-config-structure)
5. [Per-Class Priority Analysis](#per-class-priority-analysis)
6. [MQ2 Plugin Ecosystem](#mq2-plugin-ecosystem)
7. [Gap Analysis vs TextQuest](#gap-analysis-vs-textquest)
8. [Recommended Adoption Priorities](#recommended-adoption-priorities)

---

## Architecture Overview

RGMercs is a Lua-based automation framework running inside MacroQuest (MQ2). It's structured as:

```
rgmercs/
├── init.lua              # Entry point, main loop
├── modules/              # Feature modules (class, pull, mez, move, charm, loot)
├── utils/                # Shared utilities (combat, casting, movement, rotation, targeting)
├── class_configs/        # Per-class priority tables (Live/, EQ Might/, Project Lazarus/)
│   ├── Live/             # 16 class configs for Live/TLP servers
│   ├── EQ Might/         # EMU server variant
│   └── Project Lazarus/  # EMU server variant
├── ui/                   # ImGui-based overlay UI
├── lib/dannet/           # DanNet cross-client communication
└── namedlist/            # Named mob detection lists per server
```

### Key Design Principles

1. **Data-driven rotations**: Class behavior is defined entirely in Lua tables (class configs), not in procedural code. The engine iterates these tables.
2. **Resolved action maps**: At startup, spell/ability "sets" are resolved to the best available version for the character's level. E.g., `['Mantle']` resolves to the highest-level mantle disc the character has scribed.
3. **Condition-gated execution**: Every rotation entry has a `cond` function that gates execution. The engine tests conditions before attempting any action.
4. **Module isolation**: Each feature (pull, mez, charm, movement, class combat) is a separate module with its own config, state, and lifecycle hooks.

---

## Core Module Breakdown

### Movement (`utils/movement.lua` + `modules/move.lua`)

**Stick system** (melee positioning):

- `DoStick(targetId)` — positions character relative to target
  - Tank: `stick 10 id <id> [moveback] uw` (10 units, front)
  - DPS: `stick <stickDist> id <id> behindonce moveback uw` (behind, move back)
  - Stick distance adjusts based on target height (>15 height → 20 unit stick distance)
- 1-second debounce between stick commands to prevent spam
- Delegates to MQ2MoveUtils (`/stick` command)

**Navigation** (pathfinding):

- `DoNav()` — wraps MQ2Nav for mesh-based pathfinding
- `NavInCombat()` — navigate to target during combat, then stick
  - Checks MQ2Nav path exists first, falls back to `/moveto` if no navmesh
  - Waits for navigation to complete, then sticks
- `NavAroundCircle()` — circle-strafe around a target at specified radius
  - Iterates 36 positions (10° increments) to find a valid navigable point with LoS
  - Used for positional requirements (behind mob, etc.)

**Camp/Chase** (`modules/move.lua`):

- **Camp mode**: Tether to a location, return when too far
- **Chase mode**: Follow a designated chase target using MQ2Nav
  - Configurable chase distance (start/stop thresholds)
  - Can run even while paused
- **Campfire**: Fellowship campfire management with kit types
- **Stuck detection**: Tracks position changes, detects when stuck

**Comparison to TextQuest**: Our Navigator FSM in `textquest-dll/src/nav/` handles similar concepts (waypoint queue, stuck detection, humanized movement). Key differences:

- RGMercs delegates to MQ2Nav/MQ2MoveUtils plugins; we implement our own navmesh walking
- RGMercs has circle-strafing for positional combat; we don't yet
- RGMercs chase/camp is config-driven with a UI; ours is IPC-command-driven

### Combat (`utils/combat.lua`)

**Main Assist (MA) system**:

- `SetMainAssist()` — multi-tier MA selection:
  1. Assist list (configured priority list of player names)
  2. Raid assist (from raid window)
  3. Group main assist
  4. Self-fallback (configurable per group/raid/always)
- Handles assist list with spawn validation (alive, in zone)

**Target engagement** (`EngageTarget`):

- Checks: AutoEngage enabled, feign death handling, valid target, range check
- HP threshold: Only engage when target HP ≤ `AutoAssistAt` (or if self is MA)
- Melee engagement flow:
  1. Stand up if sitting
  2. If target too far → `NavInCombat()` to close distance
  3. If in range → stop nav, start `/stick`
  4. Special class handling: Rogue waits to close distance before backstab opener
  5. `/attack on`
- Non-melee: Caster "belly cast" stick for dragon named mobs, ranger autofire

**Target scanning** (`MATargetScan`):

- Scans XTarget list within radius/zradius
- Priority ordering options:
  - Named vs Non-Named preference
  - Lowest HP% vs Highest HP% preference
- **Aggro scan**: Detects mobs not fully aggro'd (< 100% aggro, not mezzed) and prioritizes them for tanks
- Safe targeting: Skips mobs already fighting other groups
- Fallback: Area spawn search if XTarget list insufficient

**Comparison to TextQuest**: Our Combatant FSM + aggro detection handles similar flow but is simpler. Key gaps:

- We lack the sophisticated MA target scan with named/trash priority
- No aggro percentage tracking (requires reading XTarget aggro data)
- No safe targeting (checking if mob is fighting strangers)

### Casting (`utils/casting.lua`)

A massive utility (~37k tokens) handling all spell/ability execution:

**Spell types supported**:

- `UseSpell()` — standard spell casting with gem memorization
- `UseSong()` — bard song casting
- `UseDisc()` — discipline activation
- `UseAA()` — alternate advancement ability
- `UseAbility()` — combat abilities (kick, bash, taunt, etc.)
- `UseItem()` — item clickies

**Buff checking system**:

- `LocalBuffCheck(spellId)` — checks if a buff is needed:
  1. Not on blocked buff list
  2. Not already active (FindBuff)
  3. Stacks with existing buffs
  4. Checks spell triggers (some buffs have triggered effects)
- `LocalPetBuffCheck()` — same for pet buffs
- `SelfBuffCheck()`, `SelfBuffAACheck()`, `SelfBuffItemCheck()` — convenience wrappers
- `GroupBuffCheck()` — uses DanNet to check remote group members' buffs

**Readiness checks**:

- `SpellReady()` — spell is ready (not in cooldown, gem available, can memorize)
- `DiscReady()` — disc timer check
- `AAReady()` — AA timer check
- `ItemReady()` — clicky timer check
- `AbilityReady()` — combat ability ready + range check

**Comparison to TextQuest**: Our GCD tracker + spell queue is much simpler. We don't have:

- Dynamic spell memorization (swapping gems on the fly)
- Buff stacking checks
- DanNet-equivalent group buff coordination
- The breadth of readiness checking (our spell system is priority-list-only)

### Rotation Engine (`utils/rotation.lua`)

The core of rgmercs — executes ordered lists of actions with conditions.

**Action resolution** (`ResolveActions`):

- Takes `ItemSets` and `AbilitySets` from class config
- For items: finds first item in set that exists in inventory
- For spells: finds highest-level spell from set that character has scribed
- Returns a `resolvedActionMap` keyed by set name

**Rotation execution** (`Rotation.Run`):

```
For each entry in rotation table:
  1. Check if entry is enabled (user toggle)
  2. Check rotation-level condition (fnRotationCond)
  3. Check chase/follow priority
  4. For each target in target list:
     a. Test entry condition (entry.cond)
     b. If pass → execute entry (ExecEntry)
     c. If group spell → break after first target
  5. Track steps taken, break if step limit reached
  6. After rotation: restore previous spell in last gem if configured
```

**Entry execution** (`ExecEntry`):

- Checks mez break prevention (target mezzed → skip)
- Type dispatch: spell, song, disc, AA, ability, item, clickyitem, customfunc
- Pre-activate and post-activate hooks per entry
- Returns success/failure and whether it was a group spell

**Spell loadout management**:

- `SetSpellLoadOutByPriority()` — modern system: spell lists with conditions
- `SetSpellLoadOutByGem()` — legacy system: per-gem spell assignments
- `LoadSpellLoadOut()` — memorizes spells into gem slots

**Comparison to TextQuest**: This is the biggest architectural difference:

- RGMercs: Data-driven rotation tables with condition functions
- TextQuest: `ClassStrategy::select_spell()` returns a single `Option<SpellEntry>`
- RGMercs has multi-step rotation execution with step limits; we pick one action per frame
- RGMercs memorizes spells dynamically; we don't manage spell gems yet

### Targeting (`utils/targeting.lua`)

- Named mob detection via named lists (per-server)
- XTarget hater count tracking
- Safe target caching (is mob fighting someone else?)
- Target validation (alive, aggressive, not a temp pet, not ignored)
- Heal target selection: group/raid member HP scanning

### Pull Module (`modules/pull.lua`)

Full pull state machine with 11 states:

```
PULL_IDLE → PULL_GROUPWATCH_WAIT → PULL_SCAN → PULL_NAV_TO_TARGET
→ PULL_PULLING → PULL_WAITING_ON_MOB → PULL_RETURN_TO_CAMP
```

**Pull abilities**: Pet pull, taunt, auto attack, ranged, kick, face pull, item pulls
**Pull modes**: Normal, Chain, Hunt, Farm
**Features**:

- Waypoint paths for hunt/farm modes
- Pull radius with camp return
- Group watch (wait for group to be ready before pulling)
- Named mob detection during pulls

**Comparison to TextQuest**: Our puller FSM in `textquest-dll/src/combat/puller.rs` is similar but doesn't have:

- Multiple pull modes (chain, hunt, farm)
- Waypoint path following during pulls
- Group readiness checking before pulls

### Mez Module (`modules/mez.lua`)

Dedicated crowd control module:

- ST mez, AE mez, AA mez
- Mez immune tracking
- Mez tracker (which mobs are mezzed, remaining duration)
- Configurable: mez start count, AE mez count, max mez count

**Comparison to TextQuest**: Our `mez_queue.rs` is simpler — we should adopt:

- Mez immune tracking
- Duration-based re-mez logic
- AE mez threshold logic

### Charm Module (`modules/charm.lua`)

Charm automation for enchanter/bard:

- Charm target selection
- Charm break detection
- Re-charm logic

**Comparison to TextQuest**: We have no charm automation yet.

---

## Rotation & Priority System

### How Rotations Work

Each class config defines:

1. **`RotationOrder`** — ordered list of rotation groups, each with:
   - `name` — matches a key in `Rotations`
   - `targetId` — function returning target(s) for this rotation
   - `cond` — condition to run this rotation (combat state, HP threshold, etc.)
   - `steps` — max actions to take from this rotation per frame
   - `state` — resume position tracking
   - `doFullRotation` — if true, always restart from entry 1 (bard weaving)
   - `load_cond` — static condition checked once at load time

2. **`Rotations`** — named tables of entries, each with:
   - `name` — references an AbilitySet or literal AA/item name
   - `type` — "Spell", "Song", "Disc", "AA", "Ability", "Item", "ClickyItem", "CustomFunc"
   - `cond` — runtime condition function
   - `active_cond` — check if effect is currently active
   - `pre_activate` / `post_activate` — hooks

3. **`HealRotationOrder`** + **`HealRotations`** — separate heal priority cascade for healer classes

### Warrior Example Rotation Order

```
1. Downtime (self buffs when out of combat)
2. HateTools(AggroTarget) — lock down XTarget haters losing aggro
3. HateTools(AutoTarget) — maintain hate on main target
4. EmergencyDefenses — triggered by HP < EmergencyStart
5. Weapon Management — bandolier swaps
6. Defenses — proactive defensive discs
7. Burn — offensive burst (burn check conditions)
8. Combat — DPS discs and abilities
```

### Cleric Heal Rotation Order

```
1. GroupHeal(98+) — DichoHeal, Beacon of Life, GroupFastHeal, Celestial Regen
2. GroupHeal(1-97) — GroupHealNoCure, GroupElixir
3. BigHeal(77+) — ClutchHeal (<35%), Sanctuary, DichoHeal, Divine Arbitration
4. BigHeal(59-76) — lower-level emergency heals
5. MainHeal(101+) — standard healing rotation
6. MainHeal(80-100) — mid-level healing
7. MainHeal(1-79) — classic-era healing
```

Level-gated via `load_cond` so only appropriate rotations load.

---

## Class Config Structure

Every class config follows this template:

```lua
_ClassConfig = {
    _version, _author,
    ['ModeChecks']      -- IsTanking, IsHealing, CanMez, etc.
    ['Modes']           -- e.g., {'Tank', 'DPS'} or {'Heal', 'Hybrid'}
    ['Themes']          -- UI color themes per mode
    ['ItemSets']        -- item priority lists (epics, chest clicks, etc.)
    ['AbilitySets']     -- spell/disc priority lists per ability slot
    ['HelperFunctions'] -- reusable condition helpers
    ['RotationOrder']   -- ordered rotation groups with conditions
    ['Rotations']       -- named rotation tables with entries
    ['HealRotationOrder'] -- (healers) heal priority cascade
    ['HealRotations']   -- (healers) heal rotation tables
    ['SpellList']       -- spell gem loadout priority
    ['DefaultConfig']   -- settings with defaults, tooltips, validation
}
```

### Ability Categories Supported

| Category     | Type String    | Resolution                               | Example             |
| ------------ | -------------- | ---------------------------------------- | ------------------- |
| Spells       | `"Spell"`      | AbilitySets → highest level in spellbook | Heal lines          |
| Songs        | `"Song"`       | AbilitySets → highest level in songbook  | Bard melodies       |
| Discs        | `"Disc"`       | AbilitySets → highest level known        | Warrior defenses    |
| AAs          | `"AA"`         | Direct by name (no set needed)           | "Blast of Anger"    |
| Abilities    | `"Ability"`    | Direct by name                           | Kick, Bash, Taunt   |
| Items        | `"Item"`       | ItemSets → first item found in inventory | Epics, chest clicks |
| Clicky Items | `"ClickyItem"` | User-configured item name                | Custom clickies     |
| Custom       | `"CustomFunc"` | Arbitrary Lua function                   | Complex mechanics   |

---

## Per-Class Priority Analysis

### Classes with configs (Live server — TLP-relevant):

| Class   | Modes        | Key Rotations                                                                  | Notable Features                                   |
| ------- | ------------ | ------------------------------------------------------------------------------ | -------------------------------------------------- |
| **WAR** | Tank         | Downtime, HateTools(Aggro), HateTools(Auto), Emergency, Defenses, Burn, Combat | Aggro scan, AE taunt safety, bandolier swaps       |
| **PAL** | Tank, DPS    | HateTools, Defenses, Heals, Combat, Downtime                                   | Self-heal cascade, stun rotation                   |
| **SHD** | Tank, DPS    | HateTools, Defenses, Lifetaps, Combat, Downtime                                | Dark Lord's Unity, lifetap priority                |
| **CLR** | (single)     | GroupHeal, BigHeal, MainHeal, Rez, Buffs, Combat                               | Level-gated heal cascades, group buff coordination |
| **DRU** | (single)     | GroupHeal, MainHeal, DoT, Nuke, Buffs                                          | Hybrid heal/DPS, debuff rotation                   |
| **SHM** | Heal, Hybrid | Slow, Heal, DoT, Debuff, Pet, Buffs                                            | Slow priority, canni, pet management               |
| **ENC** | (single)     | Mez, Tash, Slow, Nuke, Buffs, Pet                                              | Charm management, mez immune tracking              |
| **BRD** | (single)     | Song weaving (doFullRotation=true)                                             | Dynamic melody based on group comp, AE slow        |
| **WIZ** | (single)     | Burn, Nuke, Harvest, Downtime                                                  | Mana management, harvest rotation                  |
| **MAG** | (single)     | Pet, Nuke, Burn, Downtime                                                      | Pet management, RS, mali                           |
| **NEC** | (single)     | DoT, Lifetap, Pet, Burn, Downtime                                              | DoT tracking, lich, feign death                    |
| **RNG** | (single)     | Melee/Ranged toggle, DoT, Nuke, Burn                                           | Distance-based melee/ranged switch                 |
| **ROG** | (single)     | Backstab opener, Combat, Burn, Downtime                                        | Positional requirements, SOS                       |
| **MNK** | (single)     | Combat, Burn, FD, Downtime                                                     | Flying kick priority, mend                         |
| **BER** | (single)     | Combat, Burn, Downtime                                                         | Frenzy, volley                                     |
| **BST** | (single)     | Pet, Combat, Heal, Burn, Downtime                                              | Pet management, self-heal                          |

### Bard Song Weaving (Critical for TextQuest)

RGMercs bard uses `doFullRotation = true` on combat rotations, meaning:

- Every frame, the rotation restarts from entry 1
- Each song that passes its condition gets sung in order
- Songs naturally cycle because `SongReady()` checks if the song needs refreshing
- Song priority is determined by the order in the rotation table
- Different songs activate based on group composition and combat state

Song categories in rgmercs bard config:

- War March (haste/ATK)
- Aria (spell damage focus)
- Suffering (melee proc)
- Spiteful (AC/aggro proc)
- Crescendo (HP/Mana)
- Arcane (melee + spell proc)
- Insult (DD — cast during weave gaps)
- DoT songs (fire, disease, poison, ice)
- Slow songs (ST and AE)
- Regen songs (HP/Mana recovery)
- Run speed
- Cure song

---

## MQ2 Plugin Ecosystem

### MQ2Melee — Combat Automation

MQ2Melee is the foundational melee automation plugin. RGMercs largely replaces it but draws from its concepts:

- **Stick**: Maintain distance/position relative to target
  - Behind, front, pin (beside), custom angle
  - Move-back behavior (retreat if too close)
  - Underwater awareness (`uw` flag)
- **Follow**: Follow a designated character
- **Combat abilities**: Kick, bash, backstab, flying kick, etc.
- **Taunt**: Auto-taunt management
- **Holy shit**: Emergency ability when HP drops below threshold

**What TextQuest should adopt**: Our positioning module handles some of this, but we should add:

- Configurable stick positions (behind, front, pin)
- Move-back behavior
- Height-aware distance calculations

### MQ2Cast — Spell Casting State Machine

MQ2Cast handles the complexities of EQ spell casting:

- **Cast states**: Idle → Memorizing → Casting → Recovery
- **Interrupt handling**: Duck-to-interrupt, `/stopcast`
- **Spell memorization**: Dynamic gem swapping with priority
- **Fizzle/resist/interrupt retry**: Configurable retry count
- **GCD tracking**: Global cooldown awareness
- **Custom events**: "CAST_SUCCESS", "CAST_IMMUNE", "CAST_RESIST"

**What TextQuest should adopt**:

- Our `gcd.rs` tracks GCD but we lack interrupt detection
- We should add spell memorization management
- Fizzle/resist retry logic is important for TLP

### MQ2NetHeal — Cross-Group Healing Coordination

For multibox healing across groups:

- **Health broadcasting**: Share HP data across clients via DanNet
- **Heal target prioritization**: Cross-group lowest-HP selection
- **Heal assignment**: Prevent multiple healers from healing same target
- **Over-heal prevention**: Cancel heals if target HP recovered

**What TextQuest should adopt**: This is critical for 36-box. Our IPC system can carry health data but we need:

- Cross-group heal target arbitration
- Heal assignment/deconfliction
- CH chain coordination (our cleric has `ch_chain_slot` but no orchestration yet)

### MQ2Map — Map Overlay

- Camp radius visualization
- Pull radius visualization
- Named mob highlighting
- Spawn filtering

**What TextQuest should adopt**: Our TUI map already has some of this. Consider adding camp/pull radius visualization.

---

## Gap Analysis vs TextQuest

### What RGMercs Has That We Don't

| Feature                               | RGMercs                                  | TextQuest Status                         | Priority                                        |
| ------------------------------------- | ---------------------------------------- | ----------------------------------- | ----------------------------------------------- |
| **Data-driven rotation tables**       | Full Lua table system                    | Hardcoded `select_spell()`          | HIGH — our class strategies are skeletal        |
| **Spell resolution (best available)** | `ResolveActions()` auto-picks best spell | `config.spells` static list         | HIGH — TLP level progression needs this         |
| **Dynamic spell memorization**        | Swap gems on the fly                     | Not implemented                     | MEDIUM — important for limited gem slots on TLP |
| **Condition-gated abilities**         | Rich `cond` functions per entry          | Basic priority selection            | HIGH — class behavior needs conditions          |
| **Multi-rotation framework**          | Downtime, Combat, Emergency, Burn, Heal  | Single `select_spell()`             | HIGH — fundamental architecture gap             |
| **Buff stacking checks**              | `LocalBuffCheck` with trigger analysis   | Not implemented                     | MEDIUM — prevents wasted mana                   |
| **Named mob priority**                | Named/trash target preference            | Not implemented                     | LOW — nice to have for TLP                      |
| **Aggro % scanning**                  | XTarget aggro detection                  | Not implemented                     | HIGH for tanks                                  |
| **Safe targeting**                    | Skip mobs fighting other groups          | Not implemented                     | MEDIUM — prevents training                      |
| **Circle strafing**                   | `NavAroundCircle()`                      | Not implemented                     | LOW — niche use                                 |
| **Bard song weaving**                 | `doFullRotation` + song types            | Basic `twist.rs` exists             | HIGH — identified gap                           |
| **Pull modes** (chain/hunt/farm)      | Full pull FSM with waypoints             | Basic puller FSM                    | MEDIUM                                          |
| **Mez immune tracking**               | Persistent immune list                   | Not implemented                     | MEDIUM for enchanters                           |
| **Charm automation**                  | Full charm module                        | Not implemented                     | LOW for TLP start                               |
| **Clicky item management**            | Item cooldown tracking + rotation        | Not implemented                     | LOW                                             |
| **Cross-group heal coordination**     | Via DanNet/NetHeal                       | IPC exists but no heal arbitration  | HIGH for 36-box                                 |
| **Group readiness checking**          | GroupWatch before pulls                  | Not implemented                     | MEDIUM                                          |
| **DanNet communication**              | Lua-based cross-client messaging         | Rust IPC (named pipes + shared mem) | EQUIVALENT — different impl                     |

### What TextQuest Has That RGMercs Doesn't

| Feature                       | TextQuest                                 | Notes                                                  |
| ----------------------------- | ------------------------------------ | ------------------------------------------------------ |
| **Compiled Rust performance** | Native DLL, zero Lua overhead        | Significant for 36 clients                             |
| **HolyShit system**           | Emergency preemption framework       | RGMercs has basic HP checks but less structured        |
| **Mana governor**             | Throttle casting based on mana curve | RGMercs has min_mana per spell but no governor         |
| **Combat humanization**       | Randomized delays, jitter            | Anti-detection                                         |
| **DOT tracker**               | Prevent dot stacking/overwriting     | RGMercs doesn't track DoT durations as precisely       |
| **Skill cooldown tracking**   | Precise CD management                | RGMercs checks readiness but doesn't track proactively |
| **Self-healing monitor**      | Process watchdog + restart           | Not applicable to RGMercs                              |
| **CH chain orchestration**    | `ch_chain_slot` in CombatContext     | RGMercs leaves this to manual coordination             |
| **Login automation**          | Full login FSM                       | Not applicable                                         |
| **Anti-detection stack**      | M5 stealth                           | Not applicable                                         |

---

## Recommended Adoption Priorities

### Phase 1: Core Framework (Matt's Priority — Do First)

1. **Multi-rotation architecture** — Refactor `ClassStrategy::select_spell()` into a rotation system with:
   - Named rotation groups (Downtime, Combat, Emergency, Burn, Heal)
   - Per-rotation conditions (combat state, HP thresholds)
   - Step limits per rotation per frame
   - State tracking for rotation position

2. **Spell resolution system** — Add `ResolveActions` equivalent:
   - `AbilitySets` with spell lines ordered by level
   - At init, resolve each set to best available spell for character level
   - Auto-update on level up

3. **Condition-gated entries** — Each rotation entry gets:
   - `cond` function (runtime gate)
   - `active_cond` function (is this already active?)
   - Type dispatch (spell, disc, AA, ability, item)

4. **Engage/disengage flow** — Enhance `EngageTarget` with:
   - Assist HP threshold (`AutoAssistAt`)
   - Range-based nav-to-target before sticking
   - Class-specific openers (rogue backstab, etc.)

### Phase 2: Combat Intelligence

5. **MA target scanning** — Port `MATargetScan` logic:
   - Named vs trash priority
   - Lowest/highest HP preference
   - Aggro % scanning for tanks
   - Safe targeting (skip mobs fighting others)

6. **Bard song weaving** — Implement `doFullRotation` equivalent:
   - Song priority list with conditions
   - Full rotation restart each frame
   - Song duration awareness for refresh timing

7. **Buff management** — Add buff checking:
   - `LocalBuffCheck` — is buff needed? (not active, stacks, triggers)
   - Downtime buff rotation
   - Group buff coordination via IPC

### Phase 3: Healer & CC Enhancement

8. **Heal rotation cascade** — For CLR/DRU/SHM:
   - Level-gated heal rotations
   - Group heal vs single heal decision
   - Emergency → Big → Main → Maintenance tiers
   - Cross-group heal arbitration (NetHeal equivalent)

9. **Mez system** — Enhance `mez_queue.rs`:
   - Mez immune tracking (persistent)
   - AE mez threshold
   - Mez duration tracking with re-mez timing
   - Max mez count

10. **Pull system enhancement** — Add modes:
    - Chain pull (pull next before current dies)
    - Hunt mode (waypoint path pulling)
    - Group readiness check before pulling

### Phase 4: Quality of Life

11. **Spell memorization** — Dynamic gem management for limited slots
12. **Clicky item rotation** — Track item cooldowns, use in downtime
13. **Named mob detection** — Import named lists or pattern matching
14. **Circle strafing** — For positional classes

### Data Structure Proposal

Based on rgmercs patterns, here's a proposed Rust equivalent for rotation tables:

```rust
/// A named rotation group with execution conditions.
pub struct RotationGroup {
    pub name: String,
    pub target_selector: TargetSelector,
    pub condition: Box<dyn Fn(&CombatContext) -> bool>,
    pub steps_per_frame: u8,
    pub full_rotation: bool, // restart from 0 each frame (bard)
    pub entries: Vec<RotationEntry>,
    pub current_step: usize,
}

/// A single action in a rotation.
pub struct RotationEntry {
    pub name: String,
    pub action_type: ActionType,
    pub condition: Box<dyn Fn(&CombatContext, &ResolvedAction) -> bool>,
    pub active_condition: Option<Box<dyn Fn(&CombatContext, &ResolvedAction) -> bool>>,
    pub enabled: bool,
}

pub enum ActionType {
    Spell(String),     // references AbilitySet name
    Disc(String),
    AA(String),        // direct AA name
    Ability(String),   // kick, bash, etc.
    Item(String),      // item name or ItemSet reference
    Song(String),
}

pub enum TargetSelector {
    Self_,
    AutoTarget,
    AggroTarget,
    LowestHpGroupMember,
    Custom(Box<dyn Fn(&CombatContext) -> Vec<u32>>),
}
```

This would replace the current `select_spell() -> Option<SpellEntry>` with a much richer execution model that matches what rgmercs provides.
