# RedGuides & MacroQuest Automation Research

*Research date: 2026-03-28*

This document covers the EQ automation ecosystem (RedGuides, CWTN, KissAssist, MQ2Nav) and designs an auto-XP camp loop for Frostreaver's 6-box group.

---

## Table of Contents

1. [RedGuides Very Vanilla (VV)](#1-redguides-very-vanilla-vv)
2. [CWTN Class Plugins](#2-cwtn-class-plugins)
3. [KissAssist Macro](#3-kissassist-macro)
4. [MQ2Nav Navigation](#4-mq2nav-navigation)
5. [Auto-XP Camp Loop Design](#5-auto-xp-camp-loop-design)
6. [EQ Slash Command Reference](#6-eq-slash-command-reference)

---

## 1. RedGuides Very Vanilla (VV)

### What It Is

Very Vanilla is RedGuides' pre-compiled, auto-updating distribution of MacroQuest. It bundles ~158 plugins, 280+ macros, and 251 Lua scripts into a turnkey package. It is not a separate product from MQ -- it's MQ with curated packaging.

### Core Automation Features

- **Auto-login**: MQ2AutoLogin -- log all characters with one click
- **Navigation**: MQ2Nav -- mesh-based pathfinding per zone
- **Combat automation**: RGMercs or KissAssist macros, or CWTN plugins
- **Spawn notifications**: MQ2SpawnMaster for rare/named mob alerts
- **AA automation**: Automated AA spending
- **DPS metering**: Custom DPS output
- **Full scripting**: Macro + Lua engines for custom automation

### Subscription Tiers

| Tier | Cost | Includes |
|------|------|----------|
| Level 1 (Free) | $0 | Core MQ engine, 69 public plugins, 90 public Lua scripts |
| Level 2 | ~$6.65-10/mo | VV auto-updater, 89 private plugins, 280 macros (KissAssist, RGMercs), MQ2Nav, MQ2AutoLogin |
| CWTN Plugins | $30/yr per class | Per-class combat automation (on top of Level 2) |

### Frostreaver TLP Compatibility

**Good news**: The Frostreaver TLP community voted **NO Truebox** -- clients per computer are unrestricted. MQ/VV will work from day one. This is rare for TLPs and means the population will include many MQ users.

### Relevance to Frostreaver (Our Project)

| MQ/VV Feature | Frostreaver Equivalent |
|---|---|
| MQ2Nav navmesh | M3 Navigation engine |
| RGMercs/KissAssist | M4 Combat automation |
| MQ2AutoLogin | M2.5 Login automation |
| MQ2EQBC | M2 IPC (shared memory + pipes) |
| DLL injection | M2 Custom Rust DLL injection |

Our custom DLL has the advantage of being unknown to detection -- the entire MQ signature database is irrelevant to us.

---

## 2. CWTN Class Plugins

### Overview

CWTN plugins are premium, per-class combat automation plugins for MacroQuest. They are widely considered the **gold standard** for EQ multibox combat automation, winning RedGuides EQ Software Awards 2019-2021. Written and maintained by developer "CWTN."

All plugins share **MQ2CWTNCommons** -- a unified pull engine, camp system, burn framework, and med/rest logic. Each class plugin layers class-specific rotations on top.

### Supported Classes (All 16)

| Plugin | Class | Prefix | Plugin | Class | Prefix |
|---|---|---|---|---|---|
| MQ2War | Warrior | `/war` | MQ2Wizard | Wizard | `/wiz` |
| MQ2Eskay | Shadow Knight | `/shd` | MQ2Necro | Necromancer | `/nec` |
| MQ2Paladin | Paladin | `/pal` | MQ2Mage | Magician | `/mag` |
| MQ2Cleric | Cleric | `/clr` | MQ2BerZerker | Berserker | `/ber` |
| MQ2Shaman | Shaman | `/shm` | MQ2Monk | Monk | `/mnk` |
| MQ2Druid | Druid | `/dru` | MQ2Rogue | Rogue | `/rog` |
| MQ2Enchanter | Enchanter | `/enc` | MQ2Ranger | Ranger | `/rng` |
| MQ2Bard | Bard | `/brd` | MQ2Bst | Beastlord | `/bst` |

### Operating Modes

| Mode | Name | Behavior |
|------|------|----------|
| 0 | Manual | You drive movement, plugin does DPS rotation when engaged |
| 1 | Assist | Camp at position, assist MA, return to camp when idle, sit/med |
| 2 | ChaseAssist | Follow MA around, assist on targets |
| 3 | Vorpal | Assist MA, no camp/chase logic |
| 4 | Tank | Camp at position, tank incoming mobs |
| 5 | PullerTank | Pull mobs to camp, then tank them |
| 6 | PullerAssist | Pull mobs to camp, then assist MA |
| 7 | SicTank | Camp-based tanking, no sit/rest |
| 8 | HunterTank | Roam and kill autonomously |

### Key Commands (Universal -- swap `<prefix>`)

**Core:**
```
/<prefix> mode [0-8]         -- set operating mode
/<prefix> pause true/false   -- pause plugin
/<prefix> reload             -- re-read spells/abilities from INI
/<prefix> missing            -- list missing spells/AAs
```

**Pull/Camp:**
```
/<prefix> Radius ##          -- pull radius (distance)
/<prefix> zRadius ##         -- vertical pull range
/<prefix> CampRadius ##      -- camp tether radius
/<prefix> PullArc ##         -- directional cone (degrees)
/<prefix> LevelMin/Max ##    -- NPC level filter
/<prefix> GoToCamp           -- navigate back to camp
/<prefix> Ignore/Unignore    -- manage pull ignore list
```

**Burn:**
```
/<prefix> BurnNow            -- force burn cooldowns
/<prefix> StopBurnNow        -- cancel burn
/<prefix> BurnCount ##       -- mob count to trigger burns
/<prefix> BurnAllNamed true  -- always burn on named mobs
```

**Combat:**
```
/<prefix> AutoAssistAt ##    -- HP% to start DPS
/<prefix> UseAoE true/false  -- AoE toggle
/<prefix> AOECount ##        -- mob count for AoE
/<prefix> SwitchWithMA true  -- follow MA target switches
/<prefix> StickSelection #   -- melee positioning (front/behind/side)
```

**Med/Rest:**
```
/<prefix> ManaMedStart/End ## -- mana thresholds for sitting
/<prefix> HpMedStart/End ##   -- HP thresholds
/<prefix> EndMedStart/End ##  -- endurance thresholds
/<prefix> GroupWatch [0-4]    -- who to check before pulling
```

### How Combat Rotations Work

CWTN uses **automatic ability discovery**, not scripted rotations:

1. On `/reload`, plugin scans all available spells, AAs, disciplines, tomes
2. Automatically selects the best rank of each ability line
3. Fires abilities via priority queue with internal timers
4. Context-aware: single target vs AoE, burn vs sustain, defensive cooldowns by HP threshold
5. **Zero config** -- no INI rotation editing needed (unlike KissAssist)

### Pricing

- **$30/year per class plugin** (covers unlimited characters of that class)
- Requires Level 2 RedGuides membership (~$6.65-10/mo)
- Free on test/emu servers
- Full 36-box, all 16 classes: ~$480/year for plugins + membership

### Mapping to Frostreaver M4

| CWTN Concept | Frostreaver Equivalent |
|---|---|
| Per-class plugin | `ClassStrategy` trait + per-class implementations |
| MQ2CWTNCommons | Combat coordinator + shared combat types |
| Mode 5/6 pull | `puller.rs` pull FSM |
| Camp + CampRadius | Camp positioning system |
| Burn triggers | `HolyShit` conditional ability system |
| Timer-based priority | `gcd.rs` GCD tracker |
| ManaMedStart/End | `mana.rs` ManaGovernor |
| MQ2Boxr broadcast | IPC command broadcasting |

---

## 3. KissAssist Macro

### Overview

KissAssist is the most popular free automation macro for MacroQuest. Written by Maskoi, it's a single macro file with per-character INI configuration. It runs a continuous priority loop: healing > cures > aggro > DPS > buffs > pulling > meditation.

### Dependencies

- **MQ2Cast** -- spell casting
- **MQ2Exchange** -- item swapping
- **MQ2Melee** -- melee abilities
- **MQ2MoveUtils** -- movement/stick
- **MQ2Rez** -- resurrection
- **MQ2Twist** -- bard songs
- **ninjadvloot.inc** -- looting

### Startup Syntax

```
/mac kissassist                          -- default, uses INI role
/mac kissassist puller                   -- start as puller
/mac kissassist assist TankName          -- assist specific character
/mac kissassist manual                   -- buffs/clickies only
/mac kissassist [Role] [AssistName] [HP%]
```

### Roles

| Role | Description |
|------|-------------|
| Assist | Follow MA's target (default) |
| Tank | Self as MA, hold aggro |
| Puller | Pull mobs to camp |
| PullerTank | Pull + tank |
| PetTank | Pet tanks |
| PullerPetTank | Pull + pet tanks |
| Hunter | Actively seek mobs (no camp) |
| HunterPetTank | Hunt + pet tanks |
| Petassist | Pet assists MA |
| Manual | Buffs/clickies only |

### Pull Cycle

1. **Idle at camp** -- puller waits at camp position
2. **Scan for mobs** -- searches within `MaxRadius` (horizontal) + `MaxZRange` (vertical), filtered by:
   - `MobsToPull` list (partial names or `ALL`)
   - `MobsToIgnore` list
   - `PullLevel` range (min/max)
   - `PullArcWidth` (directional cone)
   - Line-of-sight checks
3. **Pull the mob** -- uses `PullWith` (spell, item, or melee approach)
4. **Return to camp** -- runs back to camp position
5. **Chain pulling** -- if `ChainPull=1`, grabs next mob when current target drops below `ChainPullHP`%
6. **Pacing** -- `PullWait` (seconds between pulls), `PullPause` (e.g., `30|2` = pause 30s every 2 pulls)

**Key Pull INI Settings:**
```ini
[Pull]
PullWith=Spell Name        # pull ability
MaxRadius=350              # horizontal pull distance
MaxZRange=50               # vertical range
PullRadiusToUse=90         # approach distance before pulling
PullWait=5                 # seconds between pulls
ChainPull=0                # chain pull toggle
ChainPullHP=90             # HP% for chain pull trigger
PullPause=30|2             # pause duration|every N pulls
PullLevel=0|0              # min|max mob level
PullArcWidth=0             # directional arc (0=360)

[ZoneName]
MobsToPull=rat,snake,bear  # or ALL
MobsToIgnore=a_quest_npc
MezImmune=mob1,mob2
MobsToBurn=Named_Mob_1
```

### Camp Radius

```ini
[General]
CampRadius=80              # operational zone around camp
CampRadiusExceed=400       # hard leash -- force return if exceeded
ReturnToCamp=1             # return to camp after combat
ChaseAssist=0              # follow MA instead of camping
ChaseDistance=25            # follow distance when chasing
```

- `/camphere` resets camp location or toggles ReturnToCamp

### Assist Target Management

```ini
[Melee]
AssistAt=100               # mob HP% to start assisting (100=immediately)
MeleeDistance=75            # stick distance
StickHow=snaproll           # MQ2MoveUtils stick method
FaceMobOn=1                # auto-face target
```

Only ONE character should be Tank role. All others Assist or Puller.

### Ability Priority System

Abilities are defined in numbered lists. **Lower numbers = higher priority.**

**DPS (Cascading mode):**
```ini
[DPS]
DPSOn=1
DPSSize=32                 # number of DPS slots
DPSSkip=20                 # skip DPS if mob HP below this %
DPS1=Barrage of Claws|95   # spell|HP_threshold
DPS2=Wallop|94
DPS3=Reflexive Slashing|93
DPS4=Some Spell|95|Cond1   # conditional: only if KCondition Cond1
DPS5=Some Spell|95|Once    # cast only once per fight
```

**Heals:**
```ini
[Heals]
HealsOn=1
HealsSize=5
Heals1=Big Heal|50         # cast when target HP < 50%
Heals2=Pet Heal|80|pet     # heal pet
Heals3=Group Heal|70|Me    # self-heal only
XTarHeal=0                 # heal extended targets
```

**Buffs:**
```ini
[Buffs]
BuffsOn=1
BuffsSize=20
Buffs1=Self Buff|Me        # self only
Buffs2=Group Buff|Dual|Alt Spell  # try alt if primary fails
Buffs3=Mana Buff|mana|40|40      # cast when mana below threshold
CheckBuffsTimer=10         # seconds between buff checks
```

**Burns:**
```ini
[Burn]
BurnAllNamed=0             # auto-burn on named
BurnSize=15
Burn1=Burn Disc 1
Burn2=Burn AA 1
```

**Conditional Logic (KConditions):**
```ini
[KConditions]
Cond1=${Me.XTarget} > 1 || ${Target.Named}   # multiple mobs or named
Cond2=${Target.Named}                          # named only
Cond3=${Group.Injured[50]} >= 2                # 2+ group members below 50%
```

Referenced by DPS/Burn entries with `|Cond1` suffix.

### Med/Sit Cycle

```ini
[General]
MedOn=1
MedStart=20                # start sitting at this mana %
MedStop=100                # stop sitting at this mana %
MedCombat=0                # don't med during combat
```

- Sits when mana < `MedStart`, stands at `MedStop`
- Still checks buffs and non-combat tasks while medding
- Auto-stands for combat/heals

### Runtime Slash Commands

| Command | Description |
|---------|-------------|
| `/camphere` | Reset camp / toggle ReturnToCamp |
| `/chase 0/1 [Name]` | Toggle chase mode |
| `/pullon` | Toggle pulling |
| `/meleeon` | Toggle melee |
| `/dpson 0/1` | Toggle DPS |
| `/buffson` | Toggle buffs |
| `/healson` | Toggle heals |
| `/burn` | Trigger burn cycle |
| `/switchma [Name]` | Switch main assist |
| `/maxradius [N]` | Set pull radius |

---

## 4. MQ2Nav Navigation

### How It Works

MQ2Nav is built on **RecastNavigation** (same library as Unity/Unreal). Two components:

- **MQ2Nav.dll** -- in-game plugin: loads navmeshes, computes paths, drives movement
- **MeshGenerator.exe** -- standalone GUI for generating/editing navmeshes

At runtime, loads a `.navmesh` file for the current zone from `Resources/MQ2Nav/`. Uses Recast/Detour for shortest-path computation across navmesh polygons.

### Navmesh Generation

1. Zone geometry extracted from EQ's `.s3d` / `.eqg` archive files via **EQEmu zone-utilities**
2. MeshGenerator loads raw triangle geometry + dynamic object configs
3. RecastNavigation pipeline: voxelization -> region generation -> contour tracing -> polygon mesh -> detail mesh
4. Saved as `.navmesh` file (protobuf + zlib compressed)
5. Tiled system allows partial updates

**Pre-built meshes** available at [mqmesh.com](https://mqmesh.com/) -- covers 34 expansions, ~2.04 GB total.

### Commands

| Command | Description |
|---------|-------------|
| `/nav target` | Navigate to current target |
| `/nav id #` | Navigate to spawn by ID |
| `/nav spawn <search>` | Navigate to spawn via search |
| `/nav loc Y X Z` | Navigate to coordinates (YXZ = /loc format) |
| `/nav locxyz X Y Z` | Navigate to XYZ coordinates |
| `/nav door [click]` | Navigate to door, optionally click it |
| `/nav door id # [click]` | Navigate to door by ID |
| `/nav item [click]` | Navigate to ground item |
| `/nav waypoint <name>` | Navigate to saved waypoint |
| `/nav stop` | Stop navigation |
| `/nav pause` | Toggle pause |
| `/nav reload` | Force reload mesh |
| `/nav recordwaypoint <name> <desc>` | Save current position as waypoint |

**Modifiers:** `distance=X` (stop within X distance), `lineofsight=off`

### Cross-Zone Travel (MQ2EasyFind)

MQ2Nav is **single-zone only**. Cross-zone handled by **MQ2EasyFind**:

- `/travelto <zonename>` -- plots multi-zone route automatically
- Uses EQ's Zone Guide connection database
- For each zone: nav to connection -> interact -> zone -> repeat
- Handles PoK stones, translocators, zone lines

### Door Handling

- Auto-clicks doors within distance < 20 units while navigating
- `IgnoreScriptedDoors` prevents accidental teleporter/portal activation
- Lifts/elevators handled via off-mesh connections in the mesh editor

### Mesh Format

- Extension: `.navmesh`
- Location: `Resources/MQ2Nav/` (one per zone, named by short name)
- Format: zlib compressed, protobuf serialized, contains Detour navmesh tile data + off-mesh connections + area flags
- Versions: v4 (legacy) and v5 (current, with header size for forward compat)

### Relevance to Frostreaver M3

Our M3 navigation already implements waypoint-based pathfinding. To match MQ2Nav:
- Need per-zone navmesh files (generate or download from mqmesh.com)
- RecastNavigation/Detour via Rust FFI (`recast-rs` or `detour-rs` crates)
- Zone geometry from S3D/EQG files via EQEmu zone-utilities
- Off-mesh connections for teleporters/elevators
- Separate zone routing layer for cross-zone travel

---

## 5. Auto-XP Camp Loop Design

Based on the research above, here's a design for Frostreaver's first automated XP camp using our existing DLL injection + InterpretCmd capability.

### Group Composition (6-Box)

| Slot | Role | Class | Frostreaver Mode |
|------|------|-------|-----------------|
| 1 | Tank/MA | Warrior | PullerTank |
| 2 | Healer | Cleric | Assist (heal focus) |
| 3 | CC | Enchanter | Assist (mez adds) |
| 4 | DPS | Rogue/Monk | Assist |
| 5 | DPS | Wizard/Mage | Assist |
| 6 | DPS | Ranger/Bard | Assist |

### State Machine: Camp Loop

```
                    +--------+
                    | IDLE   |  (all at camp, medding)
                    +---+----+
                        |
                   mana > MedStop for all casters
                        |
                    +---v----+
                    | PULL   |  (tank searches + pulls)
                    +---+----+
                        |
                   mob arrives at camp
                        |
                    +---v----+
                    | FIGHT  |  (group engages)
                    +---+----+
                        |
                   mob dead
                        |
                    +---v----+
                    | LOOT   |  (loot corpse)
                    +---+----+
                        |
                   loot complete
                        |
                    +---v----+
                    | BUFF   |  (rebuff if needed)
                    +---+----+
                        |
                   buffs checked
                        |
                    +---v----+
                    | MED    |  (sit if mana low)
                    +---+----+
                        |
                   mana > threshold OR no casters low
                        |
                    +--------+
                    | IDLE   |  (loop back to PULL)
                    +--------+
```

### Phase Details

#### Phase 1: PULL (Tank)

```
1. Tank scans for NPC within CampRadius (e.g., 200 units)
   - Filter: level range, not on ignore list, alive, targetable
2. Tank faces mob:     /face
3. Tank targets mob:   /target <mob_name>
4. Tank pulls:         /cast 1   (ranged pull spell/item)
                   or  run toward + /attack on
5. Tank returns to camp position
6. Broadcast to group: "incoming"
```

**Slash commands used:**
- `/target <name>` -- target the pull mob
- `/face` -- face the target
- `/attack on` -- engage melee (if melee pull)
- `/cast <gem>` -- ranged pull (bow shot, spell)

#### Phase 2: FIGHT (All)

```
All DPS + CC:
1. /assist <TankName>            -- acquire tank's target
2. Wait until mob HP < AssistAt% (e.g., 98%)
3. /attack on                     -- melee DPS engage
4. /cast <gem>                    -- casters begin rotation
5. /disc <name>                   -- use disciplines
6. /pet attack                    -- pet classes send pets

Healer:
1. Monitor tank HP
2. If tank HP < 70%: /cast 1     (fast heal)
3. If tank HP < 40%: /cast 2     (big heal)
4. If group HP low:  /cast 3     (group heal)

CC (Enchanter):
1. Monitor extended targets / spawn list
2. If add detected:  /target <add>  ->  /cast <mez_gem>
3. Return assist:    /assist <TankName>
```

**Slash commands used:**
- `/assist <name>` -- target assist
- `/attack on/off` -- melee toggle
- `/cast <gem>` -- spell casting
- `/disc <name>` -- discipline activation
- `/pet attack` -- pet commands
- `/stopcast` -- interrupt cast for emergency heals

#### Phase 3: LOOT

```
1. After mob dies, wait 1-2 seconds
2. /target <corpse>              -- target the corpse
3. /loot                         -- open loot window
4. Loot all items (or use advanced loot rules)
5. /autosplit                    -- ensure coin is split
```

**Slash commands used:**
- `/target <mob>'s corpse` -- target corpse
- `/loot` -- open loot
- `/autosplit` -- split coin
- `/destroy` -- destroy junk items (MQ command)

#### Phase 4: BUFF (All)

```
1. Check buff timers (read from game memory)
2. If key buffs missing:
   - Cleric:     /cast <HP_buff_gem>  on group members
   - Enchanter:  /cast <haste_gem>    on melee
   - Enchanter:  /cast <clarity_gem>  on casters
3. Use clickies: /useitem "Clicky Name"
4. Check spell set: /memspellset <set> if gems need refresh
```

**Slash commands used:**
- `/cast <gem>` -- buff casting
- `/target <group_member>` -- target buff recipient
- `/useitem "<name>"` -- use clickie items
- `/memspellset <name>` -- refresh spell loadout

#### Phase 5: MED (Casters)

```
1. If mana < MedStart (e.g., 20%):
   /sit                           -- sit to regen mana
2. While sitting:
   - Still check heals (stand for emergency)
   - Still check buffs
3. When mana > MedStop (e.g., 80%):
   /stand                         -- ready for next pull
4. Signal tank: "ready"
```

**Slash commands used:**
- `/sit` -- meditate
- `/stand` -- ready up

### Configuration Parameters

Based on KissAssist/CWTN patterns, our camp loop needs:

```toml
[camp]
camp_radius = 200          # engagement zone (game units)
camp_leash = 400           # hard return distance
pull_radius = 300          # how far tank searches for mobs
pull_z_range = 50          # vertical pull range
pull_level_min = 1         # minimum mob level
pull_level_max = 10        # maximum mob level
pull_wait_seconds = 3      # delay between pulls

[combat]
assist_at_hp = 98          # HP% to start DPS
burn_on_named = true       # auto-burn named mobs
aoe_mob_count = 3          # minimum mobs for AoE

[med]
mana_med_start = 20        # sit when mana drops below
mana_med_stop = 80         # stand when mana reaches
hp_med_start = 50          # sit for HP regen below
hp_med_stop = 90           # stand for HP regen at

[loot]
auto_loot = true           # loot corpses
destroy_junk = true        # destroy no-value items
auto_split = true          # split coin
```

### Implementation Path (Using InterpretCmd)

Since we can already send slash commands via DLL injection, the minimal implementation is:

1. **Camp anchor**: Store camp XYZ when loop starts
2. **Pull scanner**: Read spawn list (already working in M1), find valid targets
3. **Command sequencer**: Send slash commands with appropriate delays
4. **State tracker**: Read game state (HP, mana, target, combat status) to drive transitions
5. **Broadcast**: Use IPC to coordinate commands across all 6 clients

This can be built **entirely on InterpretCmd** without needing internal function hooks for combat. The slash command approach is simpler and more maintainable for a first iteration.

---

## 6. EQ Slash Command Reference

### Targeting

| Command | Syntax | Description |
|---------|--------|-------------|
| `/target` | `/target [name]` | Target nearest match (underscores for spaces) |
| `/target` | `/target [name]'s corpse` | Target specific corpse |
| `/assist` | `/assist [name]` | Target what named player is fighting |
| `/consider` | `/con` | Show target danger level + faction |
| F1-F6 | Keys | Target self (F1) or group members (F2-F6) |
| F8 | Key | Target nearest NPC |

### Combat

| Command | Syntax | Description |
|---------|--------|-------------|
| `/attack` | `/attack on/off` | Toggle melee auto-attack |
| `/autofire` | `/autofire` | Toggle ranged auto-attack (archery) |
| `/cast` | `/cast [1-13]` | Cast spell in gem slot |
| `/disc` | `/disc [name]` | Activate discipline by name |
| `/doability` | `/doability [1-10]` | Execute ability (1-6 Abilities tab, 7-10 Combat tab) |
| `/alt act` | `/alt act [####]` | Use AA ability by ID |
| `/stopcast` | `/stopcast` | Interrupt current spell cast |
| `/stopsong` | `/stopsong` | Stop bard song |

### Pet Commands

| Command | Description |
|---------|-------------|
| `/pet attack` | Attack current target |
| `/pet back off` | Stop attacking |
| `/pet follow me` | Follow owner |
| `/pet guard here` | Guard current location |
| `/pet guard me` | Follow + attack aggro |
| `/pet sit down` | Sit and rest |
| `/pet stand up` | Stand |
| `/pet taunt on/off` | Toggle taunting |
| `/pet hold on/off` | Hold -- don't attack until ordered |
| `/pet ghold on/off` | Greater hold -- ONLY attack explicit targets |
| `/pet focus` | Focus current target, ignore others |
| `/pet get lost` | Dismiss pet |

### Movement

| Command | Syntax | Description |
|---------|--------|-------------|
| `/follow` | `/follow` | Auto-follow targeted group member |
| `/sit` | `/sit` | Sit (increases mana/HP regen) |
| `/stand` | `/stand` | Stand up |
| `/loc` | `/loc` | Display current Y, X, Z coordinates |
| `/face` | `/face` | Face toward current target |
| `/dismount` | `/dismount` | Dismount from a mount |

### Looting & Economy

| Command | Syntax | Description |
|---------|--------|-------------|
| `/loot` | `/loot` | Open loot window on targeted corpse |
| `/autosplit` | `/autosplit` | Toggle auto-split coin with group |
| `/split` | `/split [pp] [gp] [sp] [cp]` | Manually split coin |
| `/consent` | `/consent [name]` | Give corpse-drag permission |
| `/consent group` | `/consent group` | Give group corpse permission |
| `/corpse` | `/corpse` | Summon your corpse within 50 feet |
| `/corpsedrag` | `/corpsedrag` | Begin dragging targeted corpse |
| `/corpsedrop` | `/corpsedrop` | Stop dragging |
| `/hidecorpse` | `/hidecorpse [all/npc/looted/none]` | Control corpse visibility |
| `/advloot` | `/advloot` | Open Advanced Loot System |
| `/lootnodrop` | `/lootnodrop` | Toggle No Drop loot confirmation |
| `/autoinventory` | `/autoinventory` | Move cursor item to inventory |

### Group & Raid

| Command | Syntax | Description |
|---------|--------|-------------|
| `/invite` | `/invite [name]` | Invite to group (cross-zone) |
| `/disband` | `/disband` | Leave group (or kick if leader) |
| `/makeleader` | `/makeleader [name]` | Transfer group leadership |
| `/gsay` | `/g [text]` | Group chat |
| `/raidinvite` | `/raidinvite [name]` | Invite to raid |
| `/raidaccept` | `/raidaccept` | Accept raid invite |
| `/gmarknpc` | `/gmarknpc [#]` | Mark NPC for group |

### Buffs & Spells

| Command | Syntax | Description |
|---------|--------|-------------|
| `/cast` | `/cast [1-13]` | Cast spell in gem slot |
| `/memspellset` | `/memspellset [name]` | Load saved spell set |
| `/savespellset` | `/savespellset [name]` | Save current spell loadout |
| `/alt act` | `/alt act [id]` | Activate AA ability |
| `/disc` | `/disc [name]` | Activate discipline |
| `/useitem` | `/useitem [slot#]` or `/useitem "Name"` | Use clickable item |
| `/bandolier` | `/bandolier activate [set]` | Swap weapon set |
| `/targetgroupbuff` | `/tgb on/off` | Cast group buffs on out-of-group |

### Utility

| Command | Syntax | Description |
|---------|--------|-------------|
| `/camp` | `/camp` | Camp to character select (30s) |
| `/camp desktop` | `/camp desktop` | Camp and close EQ |
| `/who` | `/who [filters]` | List players in zone |
| `/loc` | `/loc` | Show coordinates |
| `/time` | `/time` | Show Norrath + real time |
| `/played` | `/played` | Show total play time |
| `/log` | `/log on/off` | Toggle chat logging |
| `/afk` | `/afk [message]` | Toggle AFK with auto-reply |
| `/hotbutton` | `/hotbutton [name] [/cmd]` | Create hotbutton |

### Communication

| Command | Syntax | Description |
|---------|--------|-------------|
| `/say` | `/say [text]` | Local chat |
| `/tell` | `/t [name] [text]` | Private message |
| `/gsay` | `/g [text]` | Group chat |
| `/shout` | `/shout [text]` | Zone-wide shout |
| `/ooc` | `/ooc [text]` | Out of Character (zone-wide) |
| `/auction` | `/auction [text]` | Auction channel |
| `/gu` | `/gu [text]` | Guild chat |

### MacroQuest-Specific Commands

**Core MQ:**

| Command | Syntax | Description |
|---------|--------|-------------|
| `/mqtarget` | `/mqtarget [search]` | Target via spawn search filters |
| `/face` | `/face [fast/nolook]` | Face target (`fast` = instant) |
| `/lootall` | `/lootall` | Auto-loot all non-No-Trade |
| `/removebuff` | `/removebuff [name]` | Remove specific buff |
| `/click` | `/click left/right` | Simulate mouse click |
| `/keypress` | `/keypress [keybind]` | Simulate key press |
| `/notify` | `/notify [win] [ctrl] [act]` | UI element interaction |
| `/destroy` | `/destroy` | Destroy item on cursor |
| `/multiline` | `/multiline ; cmd1 ; cmd2` | Multi-command execution |
| `/timed` | `/timed [decisecs] [cmd]` | Delayed command execution |
| `/squelch` | `/squelch [cmd]` | Execute silently |

**MQ2EQBC (Multi-Box Chat):**

| Command | Syntax | Description |
|---------|--------|-------------|
| `/bc` | `/bc [text]` | Broadcast chat to all clients |
| `/bca` | `/bca //[cmd]` | Send command to ALL other clients |
| `/bcaa` | `/bcaa //[cmd]` | Send command to ALL clients including self |
| `/bct` | `/bct [Name] //[cmd]` | Send command to specific character |

**MQ2MoveUtils:**

| Command | Syntax | Description |
|---------|--------|-------------|
| `/stick` | `/stick [dist] [behind/front/pin]` | Stick to target at distance |
| `/stick hold` | `/stick hold` | Lock onto current target |
| `/stick off` | `/stick off` | Stop sticking |
| `/moveto` | `/moveto loc [Y] [X] [Z]` | Move to coordinates |
| `/moveto` | `/moveto id [#]` | Move to spawn by ID |
| `/makecamp` | `/makecamp [on/off]` | Set/clear camp return point |
| `/makecamp` | `/makecamp radius [#]` | Set leash radius |
| `/rootme` | `/rootme [off]` | Root player in place |

**MQ2Nav:**

| Command | Syntax | Description |
|---------|--------|-------------|
| `/nav target` | `/nav target` | Navigate to target |
| `/nav id #` | `/nav id #` | Navigate to spawn ID |
| `/nav loc Y X Z` | `/nav loc Y X Z` | Navigate to coordinates |
| `/nav door [click]` | `/nav door [click]` | Navigate to door |
| `/nav waypoint` | `/nav wp <name>` | Navigate to waypoint |
| `/nav stop` | `/nav stop` | Stop navigation |

**MQ2AutoLogin:**

| Command | Syntax | Description |
|---------|--------|-------------|
| `/loginchar` | `/loginchar [server:char]` | Log into specific character |
| `/switchchar` | `/switchchar [name]` | Switch character on same account |
| `/relog` | `/relog [seconds]` | Camp and relog |

---

## Key Takeaways for Frostreaver

1. **InterpretCmd is sufficient** for a first-iteration XP camp loop. All the commands above work via `/cmd` -- no need for internal function hooks for basic automation.

2. **CWTN's "zero config" approach** is worth emulating long-term -- auto-discover spells/AAs rather than hard-coding rotations. But for a newbie zone 6-box, hard-coded simple rotations are fine.

3. **Pull cycle is the core loop** -- everything else (DPS, heals, buffs, med) reacts to the pull state. Get pulling working first, then layer on the rest.

4. **Camp radius + leash** is the #1 safety mechanism. Characters must always return to camp. Without this, characters wander off and die.

5. **MQ2Nav's navmesh files are downloadable** from mqmesh.com. We could potentially load these directly rather than generating our own, though the format is custom (protobuf + zlib).

6. **The Frostreaver TLP has no Truebox** -- MQ/VV will be everywhere. Our custom DLL gives us a detection advantage over MQ users, but the automation bar is already high.

7. **Priority for implementation**: Pull loop > Camp tether > Assist/DPS > Heal logic > Med cycle > Loot > Buffs. This matches both KissAssist and CWTN's architectural priority.
