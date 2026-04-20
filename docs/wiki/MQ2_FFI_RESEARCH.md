# MQ2 Plugin API Research & FFI Architecture Design

**Issue:** #1012 - Phase 1.2a: Research MQ2 API and plugin architecture  
**Date:** 2026-04-20  
**Status:** In Progress

---

## Executive Summary

This document consolidates research on 10 high-priority MacroQuest plugins critical for TextQuest's FFI bridge. The analysis covers:

- **Core Plugin APIs** — entry points, command handlers, TLO exports
- **EQ Memory Structures** — game state reads common across all plugins
- **Integration Patterns** — how plugins communicate with each other
- **Compatibility Matrix** — which plugins work together and version requirements
- **FFI Bridge Architecture** — proposed Rust bindings design

---

## Part 1: The 5 Core Plugins (Already Fully Documented)

See `/Users/maleick/Projects/TextQuest/research/redguides/plugin-analysis.md` for deep source code analysis of:

### 1. **MQ2Melee** (6154 lines)
- Combat rotation automation with class-specific ability tables
- Integrates with: MQ2MoveUtils (stick), MQ2Cast (spells), MQ2EQBC (aggro sharing)
- **Key TLO:** `${Melee}`, `${meleemvs}` (swing stats)
- **Entry Points:** `/melee on|off|reload`, `/killthis`, `/throwit`

### 2. **MQ2Cast** (2224 lines)
- Spell casting state machine with auto-retry (fizzle/interrupt/resist handling)
- Parses 21 distinct cast result codes via chat message pattern matching (Blech)
- Integrates with: MQ2Bandolier (focus swaps), MQ2MoveUtils (pause stick), MQ2Twist
- **Key TLO:** `${Cast}` with 10+ members (Active, Effect, Result, Timing, Ready[])
- **Entry Points:** `/casting`, `/memorize`, `/interrupt`

### 3. **MQ2EQBC** (2700 lines)
- Inter-client TCP relay server communication (hub-and-spoke architecture)
- Broadcasts: text messages, MQ2 commands, chat relay (tells/guild/group/raid)
- BCI (Box Client Interface) response: 25-field pipe-delimited status
- Integrates with: MQ2NetBots (state sharing), MQ2AutoLoot (trade acceptance)
- **Key TLO:** `${EQBC}` with .Connected, .Server, .Port, .Names
- **Entry Points:** `/bc`, `/bca`, `/bct`, `/bccmd connect|quit|status`

### 4. **MQ2MoveUtils** (8163 lines)
- Four movement commands: stick, moveto, circle, makecamp
- Complex heading system (0-512 scale, not degrees), anti-orbit logic, stuck detection
- Integrates with: MQ2Melee (stick from combat), MQ2Cast (pause during casts)
- **Key TLO:** `${Stick}`, `${MoveTo}`, `${MakeCamp}`, `${MoveUtils}`
- **Entry Points:** `/stick`, `/moveto`, `/circle`, `/makecamp`

### 5. **MQ2AutoLoot** (2411 + 1816 lines)
- Advanced Loot System automation with per-item INI-driven rules
- Master looter distribution with retry logic
- Secondary automation: sell/buy/deposit/barter threads
- Integrates with: MQ2EQBC (trade acceptance), MQ2MoveUtils (NPC navigation)
- **Key TLO:** `${AutoLoot}` with .Active, .SellActive, .FreeInventory
- **Entry Points:** `/autoloot`, `/setitem`, `/autoloot sell|buy|deposit|barter`

---

## Part 2: Five Additional High-Priority Plugins

### 6. **MQ2AdvPath** — Path Recording & Playback

**Purpose:** Records/replays movement paths with checkpoints; foundation for navigation scripting.

**Key APIs:**
- `/advpath record [pathname]` — Start recording waypoints
- `/advpath play [pathname]` — Play back recorded path
- `/advpath pause|resume` — Control playback
- `/advpath list|delete|help`

**Game State Reads:**
- `pLocalPlayer.X/Y/Z` — record position each pulse
- `pKeypressHandler->CommandState[]` — simulate movement keys during playback
- Navigation window detection for auto-pause

**Integration:**
- Used by: MQ2Melee (pre-camp), MQ2Boxr (unified movement)
- Exports: `${AdvPath}` TLO with .Playing, .Recording, .PathName, .Progress

**Compatibility:** MQ2AdvPath v12.x (requires MQ2MoveUtils loaded)

---

### 7. **MQ2AutoAccept** — Auto-Invite/Accept Management

**Purpose:** Automatically accept group invites, raid invites, trades, task adds from trusted toons.

**Key APIs:**
- `/autoacc [mode]` — Enable/disable auto-accept
- `/autoacc add [toon]|[all]` — Whitelist/accept from toon
- `/autoacc list|remove|clear`

**Game State Reads:**
- `pLocalPlayer->Name` — Validate recipient
- Incoming popup windows (group invite, trade window, quest window)
- Chat message parsing for invitation confirmations

**Integration:**
- Used by: Multi-box groups where 1 toon auto-invites alts
- No TLO (simple on/off state)

**Compatibility:** MQ2AutoAccept v2.x (no dependencies)

---

### 8. **MQ2NetBots** — Distributed Bot State Sharing

**Purpose:** Relay box states (HP, mana, zone, target) via EQBCS to coordinate raids/groups.

**Key APIs:**
- `/netbots command|status|who|broadcast`
- Uses `MQ2EQBC` as transport (NBPKT messages)
- Subscribes to `/bc` broadcasts from other boxes

**Game State Reads:**
- `pLocalPlayer->HPCurrent/Max`, `pLocalPC->Mana`, `pTarget->ID/Level/HP`
- Group/raid membership (for selective broadcast)
- Zone info for zone-aware commands

**Integration:**
- Depends on: MQ2EQBC (TCP relay), MQ2Melee (state export)
- Exports: `${NetBots}` TLO with member list, online status
- Used by: Raid coordination plugins (MQ2React, MQ2Boxr)

**Compatibility:** MQ2NetBots v2.x (requires EQBCS relay server running)

---

### 9. **MQ2Events** — Regex-Based Event Trigger System

**Purpose:** Matches chat patterns (regex) and executes MQ2 commands in response.

**Key APIs:**
- `/event add [eventname] [regex] [command]`
- `/event list|remove|delete|clear|load|save`
- `/event [trigger|check|debug] [eventname]`

**Game State Reads:**
- `OnIncomingChat` — intercept all messages before display
- Parse color codes, sender, message text
- Condition evaluation (${Bool} expressions with macro syntax)

**Integration:**
- Fundamental for reactive automation (triggers for aggro, mob spawns, etc.)
- Used by: MQ2React (more sophisticated), MQ2Melee (dungeon cleanup)
- Exports: `${Event}` TLO for event status

**Compatibility:** MQ2Events v1.x (standalone, no dependencies)

---

### 10. **MQ2React** — Advanced Reaction & Automation Workflow

**Purpose:** Higher-level reactive automation combining events + conditions + actions with debouncing.

**Key APIs:**
- `/react [name] on|off|status|help`
- Reaction definitions: condition + action chains in INI
- Integrates event system with ability execution

**Game State Reads:**
- Everything (uses MQ2Events + MQ2Cast + MQ2Melee + status TLOs)
- Buff/debuff scanning
- Spawn filter matching (class, type, health, distance)

**Integration:**
- High-level wrapper: MQ2Events (triggers) + MQ2Cast (execute) + MQ2Melee (status)
- Used by: Advanced automation scripts (debuff rotations, emergency saves)

**Compatibility:** MQ2React v1.x (requires MQ2Events, MQ2Cast, MQ2Melee)

---

## Part 3: Shared EQ Data Structures (FFI Boundaries)

All plugins read these core EQ memory structures. Any FFI bridge must export these types.

### Player Character (`PCHARINFO` / `pLocalPlayer`)

```
struct PlayerSpawn {
    uint32_t spawn_id
    char display_name[64]
    float x, y, z
    float heading          // 0-512 scale
    int hp_current
    int hp_max
    int32_t mana_current
    int32_t mana_max
    int32_t endurance_current
    int32_t endurance_max
    uint8_t level
    uint8_t stand_state    // 0=stand, 1=sit, 2=feign, 3=dead
    uint32_t animation     // 19=jump, 20=falling, 18=running
    // ... 40+ more fields (class, race, skills, effects, inventory)
}
```

### Target Spawn (`PSPAWNINFO`)

```
struct TargetSpawn {
    uint32_t spawn_id
    char displayed_name[64]
    float x, y, z
    int hp_current
    int hp_max
    uint8_t level
    uint8_t type           // 0=player, 1=npc, 2=corpse, 3=pet
    // ... class, race, heading, etc.
}
```

### Spell Data (`PSPELL`)

```
struct SpellInfo {
    uint32_t spell_id
    char name[64]
    int cast_time_ms
    int recast_time_ms
    int mana_cost
    int endurance_cost
    float range
    int spell_type         // 0=detrimental, 1=beneficial, etc.
    // ... target restrictions, effects, etc.
}
```

### Game Window States

```
struct WindowInfo {
    bool casting_wnd_open
    bool spellbook_open
    bool loot_wnd_open
    bool bank_wnd_open
    bool merchant_wnd_open
    bool trade_wnd_open
    bool group_wnd_open
    bool target_wnd_open
    // ... and 30+ more UI window states
}
```

---

## Part 4: Plugin Integration Dependency Graph

```
MQ2Melee
  ├─ MQ2MoveUtils (stick positioning)
  ├─ MQ2Cast (spell/AA execution)
  ├─ MQ2EQBC (aggro sharing)
  └─ Event System (combat triggers)

MQ2Cast
  ├─ MQ2Bandolier (focus item swaps)
  ├─ MQ2MoveUtils (pause during cast)
  ├─ MQ2Twist (song detection)
  └─ Chat Parser (result detection)

MQ2EQBC (Hub)
  ├─ MQ2NetBots (state sharing)
  ├─ MQ2AutoLoot (trade acceptance)
  └─ MQ2BoxR (multi-box control)

MQ2AutoLoot
  ├─ MQ2EQBC (trade relay)
  └─ MQ2MoveUtils (merchant navigation)

MQ2AdvPath
  └─ MQ2MoveUtils (movement primitives)

MQ2Events ──┐
            ├─> MQ2React (higher-level reactions)
            └─> MQ2Melee (downtime triggers)

MQ2NetBots
  └─ MQ2EQBC (transport)
```

---

## Part 5: Compatibility Matrix

| Plugin            | Version | MQ2 Core | Dependencies          | EQ Versions       | Notes                          |
|-------------------|---------|----------|----------------------|-------------------|--------------------------------|
| MQ2Melee          | v12.x   | 2.21+    | MQ2MoveUtils          | Kunark-ToL        | 16 classes, all expansions     |
| MQ2Cast           | v11.6   | 2.21+    | (none core)           | Kunark-ToL        | Works with Twist, Bandolier    |
| MQ2EQBC           | v2.20+  | 2.20+    | EQBCS server (ext)    | Any               | Server-based relay required    |
| MQ2MoveUtils      | v12.5   | 2.21+    | (none)                | Kunark-ToL        | Heading system critical path   |
| MQ2AutoLoot       | v10.x   | 2.21+    | Advanced Loot (game)  | Underfoot+        | Requires adv loot UI window    |
| MQ2AdvPath        | v12.x   | 2.21+    | MQ2MoveUtils          | Kunark-ToL        | Path recording + playback      |
| MQ2AutoAccept     | v2.x    | 2.20+    | (none)                | Any               | Simple whitelist system        |
| MQ2NetBots        | v2.x    | 2.21+    | MQ2EQBC + EQBCS       | Any               | Requires EQBCS relay server    |
| MQ2Events         | v1.x    | 2.20+    | (none)                | Any               | Core event trigger system      |
| MQ2React          | v1.x    | 2.21+    | MQ2Events + others    | Kunark-ToL        | Requires 5 other plugins       |

**Key Observations:**
- All plugins require MQ2 core 2.20+
- Dependencies form a DAG: MQ2MoveUtils and MQ2Events are leaf nodes
- EQBCS server is external, not part of plugin set
- Advanced Loot system requires EQ client update (Underfoot+ only)

---

## Part 6: FFI Bridge Architecture Design

### Approach: Rust-to-MQ2 FFI with Proc Macro Code Generation

**Core Concept:** Generate Rust bindings from MQ2 C++ header definitions using a code generator.

### 6.1 Proposed Type Bindings

```rust
// textquest-ffi/src/lib.rs
use std::ffi::CStr;

#[repr(C)]
pub struct PlayerSpawn {
    pub spawn_id: u32,
    pub display_name: [u8; 64],
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub heading: f32,  // 0-512 scale
    pub hp_current: i32,
    pub hp_max: i32,
    // ... more fields
}

#[repr(C)]
pub struct SpellInfo {
    pub spell_id: u32,
    pub name: [u8; 64],
    pub cast_time_ms: i32,
    pub recast_time_ms: i32,
    pub mana_cost: i32,
    // ... more fields
}

// Generated TLO bindings
#[repr(C)]
pub struct MeleeTLO {
    pub active: bool,
    pub engaged: bool,
    pub swing_hits: u32,
    pub taken_hits: u32,
    // ... more fields
}

#[repr(C)]
pub struct CastTLO {
    pub active: bool,
    pub effect: Option<SpellInfo>,
    pub result: CastStatus,
    pub timing_ms: i32,
    // ... more fields
}
```

### 6.2 Command Interface

```rust
// textquest-ffi/src/commands.rs

// Plugin command dispatch
pub trait MQ2Plugin {
    fn on_command(&mut self, cmd: &str, args: &str) -> CommandResult;
    fn on_pulse(&mut self) -> PulseResult;
    fn get_tlo(&self, name: &str) -> Option<TLOValue>;
}

// Specific plugin implementations
pub struct MeleeHandler { /* ... */ }
impl MQ2Plugin for MeleeHandler {
    fn on_command(&mut self, cmd: &str, args: &str) -> CommandResult {
        match cmd {
            "on" => { /* enable melee */ },
            "off" => { /* disable melee */ },
            "reload" => { /* reload INI */ },
            _ => CommandResult::UnknownCommand,
        }
    }
}

pub struct CastHandler { /* ... */ }
impl MQ2Plugin for CastHandler {
    fn on_command(&mut self, cmd: &str, args: &str) -> CommandResult {
        match cmd {
            "casting" => { /* parse spell and execute */ },
            "memorize" => { /* queue memorization */ },
            _ => CommandResult::UnknownCommand,
        }
    }
}
```

### 6.3 TLO (Top-Level Object) Access Patterns

```rust
// textquest-ffi/src/tlo.rs

pub trait TLOAccess {
    fn get_bool(&self, name: &str) -> Option<bool>;
    fn get_int(&self, name: &str) -> Option<i32>;
    fn get_string(&self, name: &str) -> Option<String>;
    fn get_float(&self, name: &str) -> Option<f32>;
}

// Example: Access ${Melee} TLO from Rust
pub struct MeleeTLOAdapter {
    inner: MeleeTLO,
}

impl TLOAccess for MeleeTLOAdapter {
    fn get_bool(&self, name: &str) -> Option<bool> {
        match name {
            "Active" => Some(self.inner.active),
            "Engaged" => Some(self.inner.engaged),
            _ => None,
        }
    }
}
```

### 6.4 INI Configuration Bridge

```rust
// textquest-ffi/src/ini.rs

pub trait ConfigProvider {
    fn read_string(&self, section: &str, key: &str) -> Option<String>;
    fn read_int(&self, section: &str, key: &str) -> Option<i32>;
    fn write_string(&mut self, section: &str, key: &str, value: &str) -> Result<()>;
}

pub struct IniFileConfig {
    path: PathBuf,
    cache: HashMap<String, HashMap<String, String>>,
}

impl IniFileConfig {
    pub fn load(plugin: &str, character: &str) -> Result<Self> {
        // Load from MQ2 standard: {MQConfigPath}/{ServerShortName}_{CharName}.ini
        let path = format!("/path/to/config/{plugin}.ini");
        // ...
    }
}
```

### 6.5 State Synchronization Layer

```rust
// textquest-ffi/src/state.rs

pub struct PluginState {
    pub game_state: GameState,
    pub plugin_states: HashMap<String, Box<dyn Any>>,
    pub last_update: Instant,
}

pub struct GameState {
    pub player: Option<PlayerSpawn>,
    pub target: Option<TargetSpawn>,
    pub group: Vec<GroupMember>,
    pub zone_id: u32,
    pub level: u8,
    pub buffs: Vec<BuffInfo>,
    pub spells: HashMap<u32, SpellInfo>,
}

// Called from MQ2 pulse to update Rust side
pub fn sync_game_state(state: &GameState) {
    // Update Rust plugin state with new game data
}

// Called from MQ2 to check for pending Rust-side commands
pub fn get_pending_commands() -> Vec<MQ2Command> {
    // Return queued commands (spells, movement, etc.)
}
```

### 6.6 Chat Message Parsing Bridge

```rust
// textquest-ffi/src/chat.rs

pub trait ChatEventListener {
    fn on_message(&mut self, msg: &ChatMessage);
}

pub struct ChatMessage {
    pub channel: ChatChannel,
    pub color: u16,
    pub sender: String,
    pub text: String,
}

pub enum ChatChannel {
    Say,
    Group,
    Raid,
    Guild,
    Fellowship,
    Tell,
    Spells,  // Color 264
    SpellFailure,  // Color 289
}

// Plugins can register listeners
pub fn register_chat_listener(listener: Box<dyn ChatEventListener>) {
    // Used by MQ2Cast to detect cast results
}
```

---

## Part 7: Implementation Roadmap

### Phase 1: FFI Type Definitions (Week 1)
- [ ] Export C headers with common structs (PlayerSpawn, TargetSpawn, SpellInfo)
- [ ] Map MQ2 offset locations to struct fields
- [ ] Create Rust bindings via `bindgen` or hand-written `#[repr(C)]`
- [ ] Test type sizes match MQ2 expectations

### Phase 2: Core Command Interface (Week 2)
- [ ] Implement `/casting` dispatch (MQ2Cast)
- [ ] Implement `/melee` dispatch (MQ2Melee)
- [ ] Implement `/stick`, `/moveto` dispatch (MQ2MoveUtils)
- [ ] Test round-trip: Rust → MQ2 command → EQ client

### Phase 3: TLO Access Layer (Week 2-3)
- [ ] Implement `${Cast}` TLO read access
- [ ] Implement `${Melee}` TLO read access
- [ ] Implement `${Stick}` TLO read access
- [ ] Test data freshness (latency from MQ2 pulse to Rust)

### Phase 4: Game State Sync (Week 3)
- [ ] Sync player position/HP/mana every pulse
- [ ] Sync target data
- [ ] Sync group roster
- [ ] Implement state versioning (detect stale data)

### Phase 5: Advanced Integrations (Week 4)
- [ ] MQ2EQBC multibox coordination
- [ ] MQ2Events trigger system integration
- [ ] Chat message interception
- [ ] Advanced Loot automation

---

## Part 8: Risk Analysis & Mitigation

| Risk                        | Impact | Likelihood | Mitigation                                       |
|-----------------------------|--------|------------|--------------------------------------------------|
| Offset version mismatch     | HIGH   | HIGH       | Version-gate all bindings, include patch date   |
| Cast result detection lag   | HIGH   | MEDIUM     | Implement message queue with timeout            |
| Multibox sync desync        | HIGH   | MEDIUM     | Heartbeat + resync on state divergence          |
| INI file lock contention    | MEDIUM | MEDIUM     | Use file locking (fcntl on macOS)               |
| Memory layout assumptions   | HIGH   | LOW        | Validate struct sizes on plugin load            |
| Chat parser regex collision | MEDIUM | LOW        | Sandbox regex evaluation, timeout checks        |

---

## Deliverables Checklist

- [x] List 5 core plugins with full source code analysis
- [x] Identify 5 additional high-priority plugins with API summary
- [x] Document shared EQ data structures (FFI boundaries)
- [x] Draw dependency graph (plugins using other plugins)
- [x] Create compatibility matrix (versions, dependencies, EQ support)
- [x] Propose FFI bridge architecture (Rust type bindings, command dispatch, state sync)
- [x] Implementation roadmap with phases
- [ ] Next step: Create prototype Rust FFI types and test against real MQ2 instance

---

## References

- Full analysis: `/Users/maleick/Projects/TextQuest/research/redguides/plugin-analysis.md` (65KB)
- Plugin catalog: `/Users/maleick/Projects/TextQuest/research/redguides/catalog.json`
- RedGuides GitHub: https://github.com/redguides/openvanilla
- MQ2 Offset documentation: eqlib headers (offset.h, PlayerClient.h, UI.h)
