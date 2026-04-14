# JMB-Style Orchestration Patterns

**Document Purpose**: Formalize Joe Multiboxer (JMB) orchestration command patterns and validate TextQuest's alignment with multibox operator expectations.

**Status**: Design & Research (M8 Roadmap) | Last Updated: 2026-04-13

**Related Code**:

- `textquest-common/src/routing.rs` — `RoutingScope` enum
- `textquest/src/cli.rs` — Command dispatch logic
- `CLAUDE.md` — Orchestrator module overview

---

## Overview

MacroQuest users (especially JMB operators) use a command relay model to control large multibox sessions. TextQuest's routing subsystem (M7) implements the foundation with `RoutingScope`, but real-world multibox play requires additional patterns:

1. **All-Client Commands** — broadcast to every client in session
2. **Group-Scoped Commands** — target a named operator-defined group
3. **Single-Client Commands** — target one specific toon by character name
4. **Conditional Relay** — execute only if condition is met (e.g., "mana > 50%")
5. **Sequential Relay** — chain commands across toons in order (e.g., "CC → heal → loot")

This document defines the specification for each pattern, compares TextQuest's current routing to the JMB model, identifies gaps, and provides an implementation roadmap to reach feature parity.

---

## Pattern 1: All-Client Commands

### Intent

Broadcast a command to **every attached client** in the session. Used for coordination events that must happen uniformly across the entire group.

### Syntax

```
/all <command>
```

### Examples

1. **Session-wide pause** (e.g., zone-line preparation)

   ```
   /all pause
   ```

   Effect: All clients pause their combat/navigation loop, hold current position, and await next command.

2. **Session-wide buff refresh** (e.g., end of camp rotation)

   ```
   /all /cast Aego
   ```

   Effect: All characters attempt to cast Aegis of Nobility (or configured pet buff).

3. **Session-wide regroup** (e.g., wipe recovery)

   ```
   /all regroup to_cleric
   ```

   Effect: All clients navigate to the cleric's current position (assuming coordinator tracks it).

4. **Session-wide mana check** (e.g., debug variant)
   ```
   /all debug mana
   ```
   Effect: All clients report their current mana/endurance to Discord/log.

### Current TextQuest Implementation

✅ **Supported via `RoutingScope::AllSession`**

```rust
pub enum RoutingScope {
    AllSession,  // ← targets all clients
    Group { group_id: u8, label: String },
    OneToon { name: String },
}
```

The TUI exposes this with `:scope all` command, and the default scope is `AllSession`. Command dispatch in `cli.rs` routes to all attached clients when this scope is active.

### Validation

- [x] Routing layer supports broadcast
- [x] TUI default scope is all-session
- [x] IPC mesh can deliver to all clients simultaneously
- [ ] Operator-facing docs explain when to use all vs group

---

## Pattern 2: Group-Scoped Commands

### Intent

Target a **named operator-defined group** (1-6 character slots). Used when only a subset of the session needs to act (e.g., DPS focus fire while clerics heal).

### Syntax

```
/group <group_id> <command>
or
/g<id> <command>          # shorthand
```

### Examples

1. **DPS-only attack order** (Group 1 = melee, Group 2 = casters)

   ```
   /group 1 /attack @@targetname[1]
   ```

   Effect: Only melee group members attempt to engage the primary target.

2. **Cleric-only defensive chain** (Group 3 = clerics)

   ```
   /group 3 /cast Complete Heal
   ```

   Effect: Only clerics attempt heals on the designated healing target.

3. **Scout recon** (Group 4 = scouts, rangers, bards)

   ```
   /group 4 /hiddengroup scout
   ```

   Effect: Only scouts check for adds/trains in nearby zones via MQ2 commands.

4. **Pull setup** (Group 1 = tank, Group 2 = DPS)
   ```
   /group 1 /navigate to_waypoint pull_point
   /group 2 /navigate to_waypoint dps_position
   ```
   Effect: Tank moves to pull position; DPS moves to DPS position. Sequential execution.

### Current TextQuest Implementation

✅ **Supported via `RoutingScope::Group`**

```rust
pub enum RoutingScope {
    Group {
        group_id: u8,    // 1-6, matching G1..G6 labels in TUI
        label: String,   // Human-readable: "Alpha", "DPS", "Clerics"
    },
    // ...
}
```

TUI exposes via `:scope group <id>` and displays group label in status bar (e.g., "G1 Alpha"). The config loads group membership from `config/textquest.toml`:

```toml
[groups]
1 = { label = "Alpha", members = ["Warrior", "Ranger"] }
2 = { label = "Clerics", members = ["Cleric", "Cleric"] }
```

### Validation

- [x] Routing layer supports group targeting
- [x] Group membership loaded from config
- [x] TUI status bar shows active group
- [x] IPC dispatches to group members only
- [ ] CLI shorthand `/g1 /attack` implemented
- [ ] Sub-groups (e.g., "all DPS" across multiple groups) not yet supported

---

## Pattern 3: Single-Client Commands

### Intent

Target **one specific client by character name**. Used for per-toon overrides, recovery actions, or single-character commands.

### Syntax

```
/toon <character_name> <command>
or
@<name> <command>         # shorthand
```

### Examples

1. **Single healer override** (interrupt buff-stacking to heal)

   ```
   /toon Cleric /cast Complete Heal
   ```

   Effect: Only Cleric attempts a heal, regardless of current group or scope.

2. **Tank emergency recovery** (out of combat, low HP)

   ```
   @Warrior /camp desktopout
   ```

   Effect: Warrior client disconnects and zones to desktop (session-preserving logout).

3. **Class-specific ability** (bard song twist)

   ```
   @Bard /twist
   ```

   Effect: Bard starts alternate song twist pattern.

4. **Debug/status check** (single toon mana snapshot)
   ```
   @Ranger /mana
   ```
   Effect: Ranger reports current mana + endurance to orchestrator log.

### Current TextQuest Implementation

✅ **Supported via `RoutingScope::OneToon`**

```rust
pub enum RoutingScope {
    OneToon {
        name: String,  // Character name (e.g., "Cleric", "Warrior")
    },
    // ...
}
```

TUI exposes via `:scope toon <name>`. The CLI dispatcher resolves character name to client ID and routes to that client only.

### Validation

- [x] Routing layer supports single-client targeting
- [x] Character name → client ID resolution works
- [x] TUI status bar shows active toon
- [ ] `@name` shorthand parsed in command bar
- [ ] Operator feedback on invalid character names

---

## Pattern 4: Conditional Relay

### Intent

Execute a command **only if a condition evaluates to true**. Used for adaptive behavior without scripting—let the coordinator evaluate conditions and suppress commands when inappropriate.

### Syntax

```
/if <condition> <command>
/when <condition> <scope> <command>
```

### Examples

1. **Mana-gated healing** (only heal if healer has sufficient mana)

   ```
   /if @Cleric.mana > 50 /group 3 /cast Complete Heal
   ```

   Effect: Group 3 (clerics) attempts heal only if the designated primary healer has >50% mana.

2. **HP-gated heal chain** (only heal tank if low HP)

   ```
   /when @Warrior.hp < 70 /toon Cleric /cast Complete Heal
   ```

   Effect: Cleric only casts heal if tank's HP drops below 70%.

3. **Target-dependent attack** (only attack if target is alive)

   ```
   /if @target.alive /group 1 /attack
   ```

   Effect: Only melee group attacks if current target is not dead (prevents zombie kills).

4. **Zone-aware regroup** (only move if in safe zone)
   ```
   /when @zone.safe /group 2 /navigate to_cleric
   ```
   Effect: DPS group regroups to cleric only if zone is marked "safe" (no aggressive spawns).

### Current TextQuest Implementation

❌ **Not yet implemented** — Reserved for M8

The orchestrator loop (M7, merged `orchestration-loop` branch) has the IPC infrastructure to query live state (mana, HP, target info), but the conditional evaluation layer is not wired up. Estimated effort: **2-3 sprints** (conditional parser + state query + dispatch logic).

### Design Notes

- Conditions are evaluated **at dispatch time** on the coordinator (not on the client).
- Condition syntax leverages the existing state snapshot: `@CharacterName.mana`, `@target.hp`, `@zone.safe`.
- Failures (e.g., character offline) suppress the command silently; the operator sees it didn't execute via TUI feedback.
- No scripting language—conditions are declarative and limited to simple comparisons.

### Validation

- [ ] Conditional parser design complete (see "Implementation Plan" section)
- [ ] State query interface defined
- [ ] TUI conditional command entry tested

---

## Pattern 5: Sequential Relay

### Intent

**Chain commands across multiple toons** in order, waiting for each to complete before proceeding. Used for orchestrated multi-phase actions like pull setup, camp rotation, or loot distribution.

### Syntax

```
/chain <step1> | <step2> | <step3>
/seq <scope1> <cmd1> → <scope2> <cmd2>
```

### Examples

1. **Pull sequence** (tank engages, then DPS follows)

   ```
   /chain /toon Tank /attack primary | /group DPS /attack primary | /all /cast buff
   ```

   Effect:
   - Tank engages primary target
   - (wait for tank to signal or timeout)
   - DPS group attacks primary target
   - (wait for melee engagement)
   - All clients cast buff

2. **Camp rotation** (setup, farm, loot distribution)

   ```
   /chain /all /navigate to_camp | /group 1 /camp_clear | /all /loot auto | /group Clerics /meditate
   ```

   Effect:
   - All navigate to camp
   - Group 1 clears initial spawns
   - All auto-loot corpses
   - Clerics meditate to restore mana

3. **Emergency wipe recovery** (stop, regroup, buff)

   ```
   /chain /all /stop | /all /navigate to_cleric | /all /cast Aego
   ```

   Effect:
   - All clients stop current action
   - All navigate to cleric position
   - All cast Aegis

4. **Mana train** (coordinated med-hail sequence)
   ```
   /chain /group Clerics /meditate_start | wait 30s | /group DPS /meditate_start | wait 20s | /all /cast Aego
   ```
   Effect: Clerics meditate for 30s, then DPS meditate for 20s, then all buff together.

### Current TextQuest Implementation

❌ **Not yet implemented** — Reserved for M8

The command queue exists at the client level (`textquest-dll` command processor), but no coordinator-side sequence orchestration layer.

### Design Notes

- Steps execute **sequentially** with optional wait/completion gates.
- Wait semantics: after step N, coordinator waits for either:
  - **Time-based**: explicit `wait Xs` directive
  - **Event-based**: signal from client (action complete, mana restored) — not yet specified
  - **No-wait**: continue immediately (pipelined dispatch, coordinator fire-and-forget)
- Failure handling: if step N fails (e.g., client offline), sequence stops; operator is notified.
- Syntax is declarative; execution logic lives in the coordinator's command queue.

### Validation

- [ ] Sequential command parser design complete
- [ ] Wait/completion semantics formalized
- [ ] Client-side signaling protocol defined
- [ ] Timeout and failure recovery specified

---

## Comparison: TextQuest vs. JMB Model

| Dimension                 | JMB (MacroQuest)                      | TextQuest (Current)               | TextQuest (M8 Target)             |
| ------------------------- | ------------------------------------- | --------------------------------- | --------------------------------- |
| **All-Client Broadcast**  | ✅ Implicit (post to all clients)     | ✅ `RoutingScope::AllSession`     | ✅ Supported                      |
| **Group Targeting**       | ✅ Group lists in INI                 | ✅ `RoutingScope::Group` + config | ✅ Supported                      |
| **Single-Toon Targeting** | ✅ Relay syntax `/relay <toon> <cmd>` | ✅ `RoutingScope::OneToon`        | ✅ Supported                      |
| **Conditional Execution** | ⚠️ Limited (hardcoded in macros)      | ❌ No condition layer             | ⏳ M8: `/if <cond> <cmd>`         |
| **Sequential Chaining**   | ⚠️ Macro-level subroutines            | ❌ No sequence orchestration      | ⏳ M8: `/chain ... \| ...`        |
| **Operator UI**           | Command bar (text input)              | TUI scope switcher                | ⏳ M8: Conditional/chain editor   |
| **State Query**           | Lua macros inspect game state         | IPC query layer (M7)              | ✅ Ready for conditioning         |
| **Failure Recovery**      | Macro-level try/catch                 | Per-client timeouts               | ⏳ M8: Coordinator-level handling |
| **Session Persistence**   | File-based character list             | Shared memory + IPC mesh          | ✅ Implemented                    |

### Key Alignment Points

✅ **Strengths** (TextQuest has this covered):

- Broadcast and group targeting already working
- Single-toon addressing functional
- IPC mesh can deliver commands reliably
- Config-driven group membership

❌ **Gaps** (M8 work):

- No condition evaluation layer
- No sequential command chaining
- TUI doesn't expose conditional/chain command entry
- No multi-step failure recovery

⚠️ **Differences** (by design):

- JMB relies on Lua macros for complex logic; TextQuest coordinates via TUI (simpler, more visible)
- TextQuest has explicit group scope (vs. JMB's implicit relay chain)
- TextQuest uses shared memory for state (lower latency); JMB uses window messages (higher latency but more resilient to process crashes)

---

## Gap Analysis

### Layer 1: Command Dispatch (✅ Mostly Complete)

| Feature                | Status | Evidence                                                | Priority |
| ---------------------- | ------ | ------------------------------------------------------- | -------- |
| Broadcast routing      | ✅     | `RoutingScope::AllSession` in routing.rs                | —        |
| Group routing          | ✅     | `RoutingScope::Group` + config parsing                  | —        |
| Single-toon routing    | ✅     | `RoutingScope::OneToon` in routing.rs                   | —        |
| IPC command delivery   | ✅     | Merged `orchestration-loop` branch; named pipes working | —        |
| Client name resolution | ✅     | CLI parser in main.rs                                   | —        |

### Layer 2: Conditional Evaluation (❌ Not Started)

| Feature                | Status | Gap                                         | Priority   |
| ---------------------- | ------ | ------------------------------------------- | ---------- |
| Condition parser       | ❌     | No parser for `/if @x.y > z` syntax         | **High**   |
| State query API        | ⚠️     | IPC layer ready but no coordinator query fn | **High**   |
| Condition evaluation   | ❌     | No eval() at dispatch time                  | **High**   |
| Client signal protocol | ❌     | Implicit in macro; need formalization       | **Medium** |
| TUI condition entry    | ❌     | No command-bar support for `/if` prefix     | **Medium** |

### Layer 3: Sequential Chaining (❌ Not Started)

| Feature                   | Status | Gap                                      | Priority   |
| ------------------------- | ------ | ---------------------------------------- | ---------- |
| Chain parser              | ❌     | No parser for `/chain ... \| ...` syntax | **High**   |
| Sequence orchestrator     | ❌     | No coordinator-side queue manager        | **High**   |
| Wait/completion semantics | ❌     | Not specified (time vs. event?)          | **High**   |
| Timeout handling          | ❌     | No coordinator-side timeouts             | **Medium** |
| Failure recovery          | ❌     | No rollback or recovery logic            | **Medium** |
| TUI chain visualization   | ❌     | No visual queue display                  | **Low**    |

### Layer 4: Operator UX (⚠️ Partial)

| Feature                   | Status | Gap                                | Priority   |
| ------------------------- | ------ | ---------------------------------- | ---------- |
| Scope switcher            | ✅     | `:scope` command in TUI            | —          |
| Conditional command entry | ❌     | No `/if` prefix in command bar     | **Medium** |
| Chain command entry       | ❌     | No `/chain` support in command bar | **Medium** |
| Command history           | ✅     | Implicit in TUI                    | —          |
| Feedback (success/fail)   | ⚠️     | Per-client, not coordinated        | **Low**    |

---

## Real-World Multibox Scenarios

### Scenario 1: Camp Setup (Pull + DPS Focus + Buff)

**Context**: PoP raid camp. Tank pulls named mob. DPS burns it down. Clerics buff and heal. Repeat.

**Command Sequence**:

```
# Phase 1: Setup (operator positions group at camp waypoint)
/all /navigate to_camp

# Phase 2: Pull (tank engages)
/toon Tank /attack @@targetname[named]

# Phase 3: DPS focus (all DPS burn the target)
/group DPS /attack primary

# Phase 4: Healing (clerics react to damage)
/if @Warrior.hp < 70 /group Clerics /cast Complete Heal
/if @Warrior.hp < 40 /group Clerics /cast Greater Healing

# Phase 5: Buff after kill (all buff)
/all /cast Aego

# Phase 6: Loot
/all /loot auto
```

**Pattern Used**: All, Group, Toon, Conditional (M8)

---

### Scenario 2: Multi-Zone Gauntlet (Sequential Movement + Combat)

**Context**: Plane of Time progression. Zone line, key pickup, camp clear, return.

**Command Sequence**:

```
/chain
  /all /navigate to_zone_line |
  /all /clickdoor zone_key |
  /toon Tank /navigate to_camp |
  /group DPS /navigate to_dps_spot |
  /all /camp_clear |
  /all /navigate back_to_zone_line |
  /all /click return_door
```

**Pattern Used**: Toon, Group, All, Sequential (M8)

---

### Scenario 3: Emergency Med-Hail (Mana-Gated Healing Chain)

**Context**: Long fight, healing mana depletes. Coordinate med-hail with tank sitting to manage incoming damage.

**Command Sequence**:

```
# If tank HP is okay and clerics are OOM, sit and mana up
/if @Warrior.hp > 50 /if @Cleric.mana < 20 /chain
  /group Clerics /sit |
  /all /combat_pause |
  wait 30s |
  /group Clerics /stand |
  /all /resume_combat

# Resume healing if tank drops low again
/if @Warrior.hp < 60 /group Clerics /cast Complete Heal
```

**Pattern Used**: Conditional (M8), Sequential (M8), Single-toon

---

### Scenario 4: Tradeskill Overnight Rotation (Timed Sequential Tasks)

**Context**: Overnight ore/gem processing. Each toon does a quest turn-in, sits to craft, repeats.

**Command Sequence**:

```
/chain
  /toon Toon1 /navigate craft_giver |
  /toon Toon1 /turnin ore |
  /toon Toon1 /sit |
  wait 120s |
  /toon Toon2 /navigate craft_giver |
  /toon Toon2 /turnin ore |
  /toon Toon2 /sit |
  wait 120s |
  ... repeat for all toons
```

**Pattern Used**: Sequential (M8), Single-toon, Time-based wait

---

### Scenario 5: Scout Reports + Adaptive Response

**Context**: Large zone farm. Scouts report spawn density. If danger is high, scatter and hide. If clear, resume camp.

**Command Sequence**:

```
# Scouts check zone and report
/group Scouts /zone_report

# Coordinator receives report and decides:
/if @zone.danger_level > 7 /chain
  /all /stop |
  /group Scouts /hide scouts_waypoint |
  /group DPS /navigate safe_waypoint |
  /group Clerics /navigate safe_waypoint |
  /all /await_signal scout_all_clear

# If safe, resume
/if @zone.danger_level < 3 /chain
  /group DPS /navigate camp_waypoint |
  /group Clerics /navigate camp_waypoint |
  /all /resume_combat
```

**Pattern Used**: Conditional (M8), Sequential (M8), Group, All, Adaptive branching

---

## Implementation Roadmap

### M8.1: Conditional Relay Foundation (2-3 sprints)

**Goal**: Enable `/if <condition> <scope> <command>` syntax

1. **Parser Layer**
   - Define condition grammar (BNF or similar)
   - Implement recursive descent parser for `@char.stat <op> <value>`
   - Supported operators: `>`, `<`, `>=`, `<=`, `==`, `!=`
   - Supported stats: `mana`, `hp`, `hps` (HP spent), `level`, `zone.safe`, `zone.danger`, `target.alive`
   - **File**: `textquest/src/orchestrator/condition_parser.rs` (new)

2. **State Query Layer**
   - Extend IPC to query live state from all clients
   - Cache snapshot in coordinator for condition eval
   - Define query protocol: name → (stat → value)
   - **File**: `textquest/src/orchestrator/state_snapshot.rs` (new)

3. **Dispatch Integration**
   - Modify `cli.rs` dispatcher to parse `/if` prefix
   - Evaluate condition before executing command
   - Log success/skip to operator
   - **File**: `textquest/src/cli.rs` (modify)

4. **Tests**
   - Unit tests for condition parser (valid/invalid syntax)
   - Integration test: condition eval + dispatch
   - **File**: `textquest/tests/integration/conditional_dispatch.rs` (new)

### M8.2: Sequential Chaining (2-3 sprints)

**Goal**: Enable `/chain <step1> | <step2> | <step3>` syntax with wait semantics

1. **Parser Layer**
   - Tokenize `/chain` syntax into steps
   - Parse wait directives (`wait 30s`, `wait_for_signal`, `immediate`)
   - Expand shorthand (e.g., `→` as pipe alias)
   - **File**: `textquest/src/orchestrator/chain_parser.rs` (new)

2. **Sequence Orchestrator**
   - Coordinator-side queue manager
   - Execute steps sequentially with wait gates
   - Maintain sequence state (current step, elapsed time, completions)
   - **File**: `textquest/src/orchestrator/sequence_executor.rs` (new)

3. **Client Signaling Protocol**
   - Define signal types: `action_complete`, `mana_restored`, `custom_signal`
   - Extend IPC to support coordinator listening on signals
   - Implement client-side signal send (optional for now, can be added per-command)
   - **File**: `textquest-common/src/ipc.rs` (extend)

4. **Tests**
   - Unit tests for chain parser
   - Integration test: execute chain with timeouts
   - Edge case: step failure, sequence rollback
   - **File**: `textquest/tests/integration/sequential_dispatch.rs` (new)

### M8.3: TUI Operator Controls (1-2 sprints)

**Goal**: Expose conditional and chain commands in TUI command bar

1. **Command Bar Extensions**
   - Recognize `/if` and `/chain` prefixes
   - Highlight syntax in real-time
   - Suggest completable conditions and operators
   - **File**: `textquest/src/tui/command_bar.rs` (modify)

2. **Feedback Display**
   - Show condition eval result ("✓ Mana > 50, executing" / "✗ Mana < 50, skipped")
   - Show chain progress ("Step 2/5: waiting 30s")
   - **File**: `textquest/src/tui/status_panel.rs` (modify)

3. **Documentation**
   - In-app help: `/help if`, `/help chain`
   - Examples in TUI footer
   - **File**: `docs/orchestration-patterns.md` (extend with operator guide)

### M8.4: Advanced Features (Future, not in initial spec)

- **Adaptive Branching**: `/if <cond> <chain_a> /else <chain_b>`
- **Named Chains**: Save and rerun common sequences
- **Condition History**: Log operator decisions for analysis
- **State Persistence**: Save/load condition and chain templates to files
- **Event-Based Triggers**: Mobs spawned, ding achieved, etc.

---

## Testing Strategy

### Unit Tests

1. **Condition Parser** (`condition_parser.rs`)
   - Valid syntax: `@Cleric.mana > 50`, `@target.alive`, `@zone.safe`
   - Invalid syntax: `@mana`, `Cleric.mana`, `target > 50` (incomplete)
   - Operator precedence (if complex conditions supported)

2. **Chain Parser** (`chain_parser.rs`)
   - Valid chain: `/chain /all /cmd1 | /group 1 /cmd2 | wait 30s | /toon Tank /cmd3`
   - Invalid chain: `/chain /cmd1 /cmd2` (missing pipe), `/chain | /cmd` (leading pipe)
   - Wait directive parsing: `wait 30s`, `wait_for_signal`, `immediate`

3. **Routing** (`routing.rs` + condition eval integration)
   - Dispatch to correct targets given scope and condition
   - Condition eval suppresses dispatch when false

### Integration Tests

1. **Conditional Dispatch** (`tests/integration/conditional_dispatch.rs`)
   - Setup mock clients with known state (mana, HP)
   - Execute `/if` command, verify dispatch to correct clients
   - Verify skipped when condition false

2. **Sequential Dispatch** (`tests/integration/sequential_dispatch.rs`)
   - Execute `/chain` with 3 steps and 2 waits
   - Verify steps execute in order
   - Verify wait gate delays between steps
   - Test timeout if step hangs

3. **End-to-End** (TUI + orchestrator)
   - Run demo clients with live TUI
   - Operator enters conditional command via command bar
   - Verify feedback display (✓/✗) matches condition eval

### QA Checklist

- [ ] Condition syntax docs match parser implementation
- [ ] All 5 real-world scenarios execute without error
- [ ] Operator can toggle scope + enter conditional in same command bar
- [ ] Chain progress visible in TUI during execution
- [ ] Client failure (e.g., offline) handled gracefully (sequence stops, logged)

---

## Specification References

This section formalizes patterns for future PRs and implementation.

### Condition Grammar (BNF)

```
condition ::= subject operator value
subject   ::= "@" identifier ( "." identifier )*
operator  ::= ">" | "<" | ">=" | "<=" | "==" | "!=" | "in" | "contains"
value     ::= number | string | boolean
identifier ::= [A-Za-z_][A-Za-z0-9_]*
number    ::= [0-9]+ | [0-9]+ "." [0-9]+
string    ::= '"' [^"]* '"'
boolean   ::= "true" | "false"
```

### Chain Grammar (BNF)

```
chain     ::= "/chain" step ( "|" step )*
step      ::= wait_directive | dispatch
dispatch  ::= scope command
wait_directive ::= "wait" duration | "wait_for_signal" signal_name | "immediate"
duration  ::= [0-9]+ "s" | [0-9]+ "m"
signal_name ::= [A-Za-z_][A-Za-z0-9_]*
```

### State Snapshot Schema

```json
{
  "timestamp": "2026-04-13T15:30:00Z",
  "clients": {
    "Warrior": {
      "hp": 1250,
      "max_hp": 1500,
      "mana": 100,
      "max_mana": 100,
      "level": 65,
      "zone": "Pofire",
      "alive": true
    },
    "Cleric": {
      "hp": 850,
      "max_hp": 1000,
      "mana": 50,
      "max_mana": 800,
      "level": 65,
      "zone": "Pofire",
      "alive": true
    }
  },
  "target": {
    "name": "a_named_mob",
    "hp": 5000,
    "max_hp": 10000,
    "alive": true
  }
}
```

---

## Comparison to Existing Systems

### MacroQuest/JMB Patterns

JMB operators rely on a combination of:

- **INI files** for group definitions (similar to TextQuest's `config/textquest.toml`)
- **Lua macros** for conditional logic and sequences (TextQuest uses TUI + orchestrator)
- **Relay command** (`/relay <toon> <cmd>`) for single-toon addressing (TextQuest uses `:scope toon`)
- **Implicit broadcast** (post command to all clients by default)

TextQuest's approach is **more declarative**: the operator specifies the target scope in the TUI, then commands implicitly route to that scope. JMB's approach is **command-embedded**: scope is part of the command syntax (e.g., `/relay tank /attack`).

### KissAssist Integration

KissAssist (advanced MQ2 plugin) adds:

- **Trigger system**: event-based condition checks (mobs spawned, named encountered, etc.)
- **Named macros**: reusable sequences stored in files
- **Lua scripting**: full turing-complete logic (beyond TextQuest's scope)

TextQuest's sequential chaining is simpler: time-based waits and client signals, no event subscriptions. This is intentional—fewer moving parts means fewer failure modes.

---

## Success Criteria

✅ **This document is complete when**:

1. All 5 patterns documented with 3+ examples each
2. Comparison to JMB is concrete (not vague)
3. Gap analysis identifies specific files and estimated effort
4. Real-world scenarios demonstrate all 5 patterns in action
5. Implementation roadmap breaks M8 into 2-3 sequenced sprints
6. Specification section formalizes grammar and state schemas
7. Testing strategy covers unit, integration, and QA

✅ **Acceptance criteria for M8 implementation**:

- Conditional evaluation integrated into dispatcher
- Sequential chaining orchestrator deployed
- TUI command bar supports `/if` and `/chain` prefixes
- All 5 real-world scenarios execute without error
- No performance regression (command latency <100ms)

---

## Future Enhancements (Out of Scope for M8)

1. **Event-Based Triggers**: React to spawn alerts, DPS checks, mana train readiness
2. **Named Sequences**: Save multi-step combos with names (`save_chain wipe_recovery`, `exec wipe_recovery`)
3. **Branching Logic**: `/if <cond> <chain_a> /else <chain_b>`
4. **Adaptive Targeting**: Auto-select healer based on lowest HP (dynamic scope resolution)
5. **Raid Integration**: Discord webhook feedback on command success/fail
6. **Macro Recording**: TUI gesture → auto-generate `/chain` command
7. **Latency Compensation**: Predict command completion time, account for network delay

---

## Document History

| Date       | Author            | Change                          |
| ---------- | ----------------- | ------------------------------- |
| 2026-04-13 | Claude (Autoship) | Initial design and gap analysis |

---

## Approval & Sign-Off

- **Design Review**: Pending
- **Architect Review**: Pending
- **Implementation Start**: After approval
- **Target Merge**: M8.1 sprint completion
