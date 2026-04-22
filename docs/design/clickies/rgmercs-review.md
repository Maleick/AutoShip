# Clickies Module — RGMercs Review & Design Comparison

## Overview

This document analyzes the **rgmercs** Lua-based clickies implementation and compares it with TextQuest's proposed Rust architecture. RGMercs is the industry-standard EQ multi-boxing framework, with clickies as a core module for automating consumable usage.

**Reference**: [RGMercs - RedGuides Community](https://www.redguides.com/community/forums/rgmercs-lua.196/)

---

## RGMercs Clickies: Key Functions & Logic

### Configuration Pattern (Lua)

RGMercs clickies are configured as Lua tables in per-character files:

```lua
-- rgmercs/config/character.lua
clickies = {
    {
        name = "Healing Potion",
        item = "Healing Potion",
        condition = function(state) return state.hp < 50 end,
        cooldown = 6,
        target = "self",
        phase = "combat",
        priority = 100,
    },
    {
        name = "Mana Potion",
        item = "Mana Potion",
        condition = function(state) return state.mana < 30 and state.in_combat end,
        cooldown = 8,
        target = "self",
        phase = "combat",
        priority = 80,
    },
}
```

### Core Functions in clickies.lua

#### 1. **Clicky Evaluation Loop**

```lua
function ClickieManager:evaluate()
    local queued_items = {}
    local current_time = os.time()
    
    for idx, clicky in ipairs(self.clickies) do
        if not self:on_cooldown(idx, current_time) then
            if clicky.condition(self.state) then
                table.insert(queued_items, clicky)
            end
        end
    end
    
    -- Sort by priority
    table.sort(queued_items, function(a, b)
        return a.priority > b.priority
    end)
    
    return queued_items
end
```

**Key aspects:**
- **Runtime condition evaluation**: Conditions are Lua functions evaluated each frame
- **Cooldown tracking**: Per-item timestamps stored in a lookup table
- **Priority sorting**: Items sorted by priority before execution
- **No static type system**: Flexible but runtime-error prone

#### 2. **Item Usage Logic**

```lua
function ClickieManager:use_item(item_index)
    local clicky = self.clickies[item_index]
    
    -- Verify item in inventory
    if not self:find_inventory_slot(clicky.item) then
        mq.cmd('/autoinventory')
        return false
    end
    
    -- Click the item
    local slot = self:find_inventory_slot(clicky.item)
    mq.cmd('/itemnotify ' .. slot .. ' leftmouseup')
    
    -- Record cooldown
    self.last_used[item_index] = os.time()
    
    return true
end
```

**Key aspects:**
- **Inventory lookup at use time**: Searches inventory every click
- **MQ2 command interface**: Uses `/itemnotify` for item clicking
- **Manual cooldown tracking**: Last-use timestamps per index
- **Side effects**: Modifies internal state after click

#### 3. **Cooldown Tracking**

```lua
function ClickieManager:on_cooldown(idx, current_time)
    local last_use = self.last_used[idx] or 0
    local elapsed = current_time - last_use
    local cooldown_secs = self.clickies[idx].cooldown
    
    return elapsed < cooldown_secs
end
```

**Key aspects:**
- **Simple timestamp-based tracking**: `os.time()` for seconds
- **Per-item state**: Individual cooldown for each configured item
- **Linear search**: No hash-based optimization
- **Manual expiration**: No automatic cleanup

#### 4. **Condition System**

RGMercs uses **inline Lua functions** as conditions:

```lua
-- Complex condition example
condition = function(state)
    return state.hp < 40 
        and state.in_combat 
        and not state:has_debuff("root")
        and state:count_group_below_hp(50) > 1
end
```

**Key aspects:**
- **Closure-based**: Each condition is a Lua closure with access to outer scope
- **Turing-complete**: Can express arbitrary logic
- **Runtime evaluation**: No pre-parsing or optimization
- **Error handling**: Relies on Lua `pcall` for safety

#### 5. **Phase Management**

```lua
function ClickieManager:set_phase(phase)
    self.current_phase = phase  -- "combat", "downtime", "precomm"
end

function ClickieManager:get_active_clickies()
    local result = {}
    for idx, clicky in ipairs(self.clickies) do
        if clicky.phase == self.current_phase or clicky.phase == "always" then
            table.insert(result, clicky)
        end
    end
    return result
end
```

**Key aspects:**
- **Phase filtering**: Items only active in matching phases
- **"always" mode**: Some items usable in any phase
- **Manual phase transitions**: Parent combat loop drives phase changes

#### 6. **Clicky Panel (UI)**

RGMercs includes an ImGui-based clicky panel showing:
- Current item cooldowns (visual progress bar)
- Last use time and target
- Enable/disable per-item
- Real-time condition evaluation (debug mode)

```lua
-- Panel updates every frame
function ClickiePanel:render()
    for idx, clicky in ipairs(manager.clickies) do
        local is_ready = not manager:on_cooldown(idx)
        local button_text = clicky.name .. (is_ready and " ✓" or " " .. remaining_secs .. "s")
        
        if ImGui.Button(button_text) then
            manager:use_item(idx)
        end
    end
end
```

### RGMercs Integration Points

| Location | Function | Trigger |
|----------|----------|---------|
| **Combat loop** | `evaluate()` | Every frame (~6/sec) |
| **State update** | `update_state()` | HP/Mana changes |
| **Phase transitions** | `set_phase()` | Combat start/end |
| **UI rendering** | `render_panel()` | Every draw frame |

---

## Design Decisions: RGMercs vs. TextQuest

### 1. **Configuration Format**

| Aspect | RGMercs | TextQuest |
|--------|---------|-----------|
| **Format** | Lua tables (runtime) | TOML files (compile-time) |
| **Type safety** | None | Strong (Rust serde) |
| **Validation** | At evaluation time | At load time |
| **Hot-reload** | `dofile()` call | IPC message + re-parse |
| **Maintainability** | Flexible, prone to errors | Strict, IDE-assisted |

### 2. **Condition Evaluation**

| Aspect | RGMercs | TextQuest |
|--------|---------|-----------|
| **Syntax** | Lua functions (Turing-complete) | String DSL + AST (limited) |
| **Evaluation** | At runtime per frame | Compiled condition tree |
| **Safety** | `pcall()` wrapping | Type-safe matching arms |
| **Performance** | Closure overhead per-frame | Single recursive descent |
| **Debugging** | Print statements, ImGui | Structured error messages |

**TextQuest advantage**: Conditions are validated at config load, not discovered at runtime.

### 3. **Cooldown Tracking**

| Aspect | RGMercs | TextQuest |
|--------|---------|-----------|
| **Storage** | Table of timestamps | HashMap<item_name, instant> |
| **Lookup** | Linear search by index | O(1) hash lookup |
| **Granularity** | 1-second resolution | Millisecond precision |
| **Persistence** | In-memory only | Can serialize to disk |
| **Multi-character** | Per-character file | Per-character IPC channel |

### 4. **Target Selection**

| Aspect | RGMercs | TextQuest |
|--------|---------|-----------|
| **Hardcoded targets** | Enum: "self", "target", "group" | `ClickieTarget` enum in code |
| **Smart targeting** | Scripted helpers (e.g., `find_healer()`) | Orchestrator with full group state |
| **Group awareness** | Via MQ2 `${Group}` macro data | Direct GroupState struct |
| **Failsafes** | Condition checks prevent bad clicks | DLL validates target PID first |

### 5. **Inventory Management**

| Aspect | RGMercs | TextQuest |
|--------|---------|-----------|
| **Lookup** | At-click via `/itemnotify` + slot search | DLL maintains inventory cache |
| **Availability** | Runtime search (slow) | Proactive cache update |
| **Fallbacks** | `/autoinventory` on miss | Skip if not found |
| **Error handling** | Silent failures on missing item | Logged warning + skip |

### 6. **Execution Model**

| Aspect | RGMercs | TextQuest |
|--------|---------|-----------|
| **Where evaluated** | DLL (MQ2 script runner) | Split: DLL (eval) + Orch (exec) |
| **Asynchronous** | Synchronous per-frame | Async: queue → execute |
| **Queuing** | Single "next item to use" | Priority queue with depth limit |
| **Ordering** | FIFO by priority | Highest priority first |
| **Blocking** | Can block MQ2 loop if slow | Non-blocking, orchestrator manages timing |

---

## Lua-to-Rust Mapping Table

Complete mapping of RGMercs concepts to TextQuest equivalents:

| RGMercs (Lua) | TextQuest (Rust) | File Location | Notes |
|---|---|---|---|
| **clicky table** | `ClickieRule` struct | `textquest-dll/src/combat/clickies.rs` | Config loaded from TOML |
| **condition fn** | `ConditionExpr` enum | `textquest-dll/src/combat/state.rs` | Parsed DSL, not Lua |
| **cooldown tracking** | `CooldownTracker` struct | `textquest-dll/src/combat/ability_cooldowns.rs` | Uses `HashMap<String, Instant>` |
| **evaluate loop** | `ClickieEvaluator::evaluate()` | `textquest-dll/src/combat/clickies.rs` | Runs every frame in DLL |
| **use_item()** | IPC message to orchestrator | `textquest-common/src/protocol.rs` | DLL sends recommendation, Orch executes |
| **inventory lookup** | `ClickieManager::item_available()` | `textquest/src/camp/clickies.rs` | Precomputed in orchestrator |
| **phase filtering** | `CampPhase` enum + filter | `textquest/src/camp/state.rs` | Pulled from orchestrator state |
| **UI panel** | REST API + TUI dashboard | `textquest-web/src/api.rs` + TUI | Real-time state via IPC |
| **cooldown UI** | TUI clickies panel | `textquest/src/tui/ui/clickies.rs` | Shows per-item cooldown status |
| **hot reload** | `ReloadRules(ClickieRules)` IPC | `textquest-common/src/protocol.rs` | Sent from orchestrator to DLL |

### Detailed Mappings

#### Condition Functions → Condition DSL

**RGMercs:**
```lua
condition = function(state)
    return state.hp < 40 and state.in_combat
end
```

**TextQuest:**
```toml
condition = "(self_hp_pct < 40) AND in_combat"
```

**Parsing:**
- RGMercs: Interpreted at runtime (no parsing)
- TextQuest: Parsed into AST at config load, cached

#### Cooldown Tracking

**RGMercs:**
```lua
self.last_used = {}  -- idx → timestamp
function on_cooldown(idx, current_time)
    local elapsed = current_time - (self.last_used[idx] or 0)
    return elapsed < cooldown_secs
end
```

**TextQuest:**
```rust
cooldowns: HashMap<String, Instant>
fn is_ready(&self, item_name: &str) -> bool {
    self.cooldowns.get(item_name)
        .map(|exp| Instant::now() >= exp)
        .unwrap_or(true)
}
```

#### Inventory Queries

**RGMercs:**
```lua
local slot = self:find_inventory_slot(item_name)
mq.cmd('/itemnotify ' .. slot .. ' leftmouseup')
```

**TextQuest (DLL side):**
```rust
// DLL evaluates conditions and queues recommendations
pub fn evaluate(&mut self, state: &CombatState) -> Vec<String> {
    // Returns item names (orchestrator verifies availability)
}
```

**TextQuest (Orchestrator side):**
```rust
// Orchestrator checks before execution
fn item_available(&self, item_name: &str) -> bool {
    self.inventory_tracker.get_item(item_name).is_some()
}
```

#### Phase Management

**RGMercs:**
```lua
if clicky.phase == self.current_phase or clicky.phase == "always" then
    -- include in active list
end
```

**TextQuest:**
```rust
pub enum CampPhase {
    Pulling,
    Fighting,
    Recovery,
    Waiting,
}

// Filter conditions include phase check
condition = "(phase == 'fighting') AND (hp_pct < 50)"
```

---

## Implementation Comparison: Key Insights

### RGMercs Strengths

✅ **Flexibility**: Lua closures allow complex, arbitrary logic  
✅ **Simplicity**: Single file, all in one place  
✅ **Live reload**: Can edit and reload without restart  
✅ **Community tested**: Years of production use  

### RGMercs Weaknesses

❌ **No type safety**: Errors discovered at runtime  
❌ **String-based inventory**: Searches by name, not reliable with stacks  
❌ **Single-threaded**: Blocks MQ2 loop if slow  
❌ **Hard to optimize**: Condition evaluation costs per-frame  
❌ **Limited to MQ2**: Can't control clients from outside process  

### TextQuest Strengths

✅ **Type-safe configuration**: Errors caught at load time  
✅ **Split architecture**: DLL (recommend) + Orch (execute) = async  
✅ **Millisecond precision**: Cooldown tracking is sub-second  
✅ **Pre-computed conditions**: Parsed once, cached forever  
✅ **Inventory cache**: Proactive tracking, not at-click lookup  
✅ **Cross-client coordination**: Orchestrator sees all group state  
✅ **Extensible**: Web API, TUI, plugins can contribute conditions  

### TextQuest Tradeoffs

⚠️ **More complex**: Multiple crates, IPC protocol  
⚠️ **Less flexible**: Conditions limited to DSL  
⚠️ **Stricter validation**: Typos in config = hard error  
⚠️ **Rust learning curve**: Not as accessible as Lua  

---

## Design Decision: Why TextQuest's Approach

1. **Type Safety Over Flexibility**
   - RGMercs can evaluate any Lua code; TextQuest validates upfront
   - TextQuest catches config errors at load, not during combat

2. **Async Execution**
   - RGMercs blocks MQ2 loop; TextQuest queues and executes asynchronously
   - Prevents one slow item use from delaying entire automation loop

3. **Multi-Client Coordination**
   - RGMercs is single-client (one character per script)
   - TextQuest's orchestrator sees all group state, enables group-wide logic

4. **Inventory Caching**
   - RGMercs searches inventory at click time (expensive)
   - TextQuest maintains inventory cache in DLL (O(1) lookups)

5. **Millisecond Precision**
   - RGMercs tracks cooldowns in 1-second granularity
   - TextQuest tracks in milliseconds for tight combat loops

---

## Migration Path: RGMercs → TextQuest

For users migrating from RGMercs:

1. **Convert Lua conditions to TOML DSL**
   ```lua
   -- RGMercs
   condition = function(state) return state.hp < 40 and state.in_combat end
   ```
   ```toml
   # TextQuest
   condition = "(self_hp_pct < 40) AND in_combat"
   ```

2. **Map Lua tables to TOML arrays**
   ```lua
   -- RGMercs
   clickies = { { name = "...", priority = 100 }, ... }
   ```
   ```toml
   # TextQuest
   [[rotation.clickies]]
   name = "..."
   priority = 100
   ```

3. **Adjust cooldown precision**
   - RGMercs: `cooldown = 6` (seconds)
   - TextQuest: `cooldown_ms = 6000` (milliseconds)

4. **Test with live data**
   - Export RGMercs logs to compare usage patterns
   - Adjust thresholds based on actual combat

---

## Summary

TextQuest's Rust-based clickies module represents a **evolution** of the RGMercs pattern:

| Dimension | Evolution |
|-----------|-----------|
| **Type system** | Lua (untyped) → Rust (strongly typed) |
| **Execution** | Single-threaded → Async split DLL/Orch |
| **Config** | Runtime Lua → Compile-time TOML |
| **Precision** | Seconds → Milliseconds |
| **Scope** | Single character → Full group |

Both approaches solve the same problem (automating consumable use), but TextQuest optimizes for **correctness, performance, and multi-client coordination** over the **flexibility and accessibility** of Lua.

**Verdict**: For EQ automation at scale (multi-boxing, raid composition, economy), TextQuest's approach is superior. For simple cases or one-off scripts, RGMercs remains excellent.

---

## References

- **RGMercs**: https://www.redguides.com/community/forums/rgmercs-lua.196/
- **RGMercs Resource**: https://www.redguides.com/community/resources/rgmercs-lua-edition.3040/
- **aqobot (Lua reference)**: https://github.com/aquietone/aqobot/

