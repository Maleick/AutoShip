# Clickies Module Architecture — Phase 2.1a Design

## Overview

The **Clickies** module automates consumption of one-time-use items during combat and recovery phases. Clickies are consumable items (potions, food, drinks, scrolls, augment items) with per-character cooldowns and recast timers.

**Scope:**
- Item type classification (potions, food, drinks, scrolls, clicky items)
- Condition-based item usage (HP/mana thresholds, debuff presence, raid state)
- TOML configuration (per-character item rules)
- API for orchestrator + plugins
- Integration with camp loop (recovery, combat)

---

## Supported Item Types

### Primary Categories

1. **Potions** (HP/Mana restore)
   - Potion of Replenishment (mana)
   - Healing Potion (HP)
   - Clarity Potion (mana regeneration buff)

2. **Food/Drink** (sustenance + buffs)
   - Food: STR/DEX/CON buffs
   - Drink: healing/mana regeneration over time
   - Special: stat food, resists food

3. **Scrolls** (temporary buff consumables)
   - Combat Inspiration Scroll (CHA buff)
   - Fortitude Scroll (AC buff)
   - Protection Scroll (damage reduction)

4. **Clicky Items** (on-use abilities with cooldown)
   - Healing click (targeted or self)
   - DPS click (nuke or DoT)
   - Dispel/Cure click
   - Buff click (group or self)

5. **Augment Items** (passive stat/resist gear)
   - Augmentations slotted into equipment
   - Triggered on specific conditions (no recast, passive)

---

## Condition System

Each clickie rule specifies **WHEN** to use an item.

### Condition Types

```
Trigger Conditions:
├─ HP based
│  ├─ self_hp_pct < 50
│  ├─ any_group_hp_pct < 30
│  └─ target_hp_pct > 75
├─ Mana based
│  ├─ self_mana_pct < 20
│  ├─ self_endurance_pct < 30
│  └─ any_caster_mana_pct < 25
├─ Debuff based
│  ├─ has_debuff("poisons")
│  ├─ missing_buff("clarity")
│  └─ debuff_stacks > 2
├─ Combat state
│  ├─ in_combat
│  ├─ group_in_combat
│  └─ target_is_named
├─ Combat phase
│  ├─ phase == "pulling"
│  ├─ phase == "fighting"
│  ├─ phase == "recovery"
│  └─ phase == "waiting"
├─ Group composition
│  ├─ group_size >= 4
│  ├─ has_role("tank")
│  └─ raid_mode
└─ Time based
   ├─ last_used > 30_seconds
   └─ cooldown_ready
```

### Condition Operators

- **Comparison**: `<`, `<=`, `==`, `>`, `>=`, `!=`
- **Logical**: `AND`, `OR`, `NOT`
- **List**: `IN`, `NOT_IN`

### Condition Evaluation Example

```
# Use mana potion when below 20% mana AND not currently casting
condition = "(self_mana_pct < 20) AND NOT is_casting"

# Group heal scroll only when multiple group members below 30% HP
condition = "(count_group_below_hp_pct(30) > 1) AND in_combat"

# Clicky heal on tank when tank below 50% AND potion cooldown ready
condition = "(target_role == 'tank') AND (target_hp_pct < 50) AND cooldown_ready"
```

---

## Configuration Structure (TOML)

### File Layout

```
config/
└─ clickies/
   ├─ clickies.toml          # Global item catalog
   ├─ shared_rules.toml      # Shared condition rules
   └─ <character_name>.toml   # Per-character item usage rules
```

### Global Item Catalog (`clickies.toml`)

```toml
[items]

# Potions
[[items.entries]]
name = "Potion of Healing"
item_type = "potion"
effect = { type = "heal_self", amount = 300 }
recast_seconds = 6
stack_size = 20

[[items.entries]]
name = "Greater Healing Potion"
item_type = "potion"
effect = { type = "heal_self", amount = 600 }
recast_seconds = 10
stack_size = 20

# Clicky items
[[items.entries]]
name = "Spell: Healing Wave"
item_type = "clicky"
effect = { type = "cast_spell", spell_name = "Healing Wave" }
recast_seconds = 30
max_charges = 50
charges_per_use = 1
class_restrictions = ["cleric", "druid", "shaman"]

[[items.entries]]
name = "Clarity Draught"
item_type = "drink"
effect = { type = "buff", buff_name = "Clarity", duration_seconds = 3600 }
recast_seconds = 3600
stack_size = 10

# Scrolls
[[items.entries]]
name = "Combat Inspiration Scroll"
item_type = "scroll"
effect = { type = "buff", buff_name = "Inspiration", duration_seconds = 600 }
recast_seconds = 600
stack_size = 20
```

### Per-Character Rules (`druid_name.toml`)

```toml
[rotation]

[[rotation.clickies]]
name = "Greater Healing Potion"
priority = 100
condition = "self_hp_pct < 40"
target = "self"
disabled = false

[[rotation.clickies]]
name = "Potion of Replenishment"
priority = 80
condition = "(self_mana_pct < 30) AND in_combat"
target = "self"
disabled = false

[[rotation.clickies]]
name = "Clarity Draught"
priority = 60
condition = "(NOT has_buff('Clarity')) AND (in_combat OR recently_in_combat)"
target = "self"
disabled = false

[settings]
auto_pickup_types = ["potion", "drink", "scroll"]
skip_if_healer_available = false
max_potion_stacks = 5
exclude_keywords = ["broken", "cracked", "inferior"]
```

---

## DLL Implementation (`textquest-dll/src/combat/clickies.rs`)

```rust
pub struct ClickieRule {
    pub name: String,
    pub item_name: String,
    pub priority: u8,
    pub condition: ConditionExpr,
    pub target: ClickieTarget,
    pub max_uses_per_combat: Option<u32>,
}

pub enum ClickieTarget {
    Self_,
    GroupMember { index: usize },
    GroupByRole { role: String },
    HighestHpDeficit,
    Tank,
}

pub struct ClickieEvaluator {
    rules: Vec<ClickieRule>,
    usage_tracking: HashMap<String, UsageStats>,
}

impl ClickieEvaluator {
    pub fn evaluate(
        &mut self,
        state: &CombatState,
        group_state: &GroupState,
    ) -> Vec<(u8, String, u32, u32)> { ... }
    
    fn eval_condition(&self, expr: &ConditionExpr, state: &CombatState) -> bool { ... }
    fn record_usage(&mut self, item_name: &str, combat_id: u64) { ... }
}

pub enum ConditionExpr {
    Comparison { left: Value, op: CompOp, right: Value },
    Logical { op: LogOp, left: Box<ConditionExpr>, right: Box<ConditionExpr> },
    Negation { inner: Box<ConditionExpr> },
    GroupCount { predicate: String, count: usize },
}

pub enum Value {
    SelfHpPct,
    SelfManaPct,
    SelfEndurancePct,
    TargetHpPct,
    GroupHpAvg,
    HasDebuff(String),
    HasBuff(String),
    InCombat,
    CooldownReady,
}
```

**Key Design Decisions:**
- Conditions evaluated every frame; actions queued to orchestrator
- Priority queue: highest priority item wins
- Cooldown tracking per-character
- DLL queries inventory before recommending items
- No inventory changes in DLL; orchestrator handles pickup/drop

---

## Orchestrator Implementation (`textquest/src/camp/clickies.rs`)

```rust
pub struct ClickiesConfig {
    pub global_catalog: ItemCatalog,
    pub shared_rules: SharedRules,
    pub character_rules: HashMap<String, CharacterClickieRules>,
}

pub struct ClickieManager {
    config: ClickiesConfig,
    inventory_tracker: ClickieInventory,
    cooldown_tracker: CooldownTracker,
}

impl ClickieManager {
    pub fn item_available(&self, item_name: &str) -> bool { ... }
    pub fn use_item_command(&self, item_name: &str, target_pid: Option<u32>) -> String { ... }
    pub fn update_cooldowns(&mut self, ipc_msg: CooldownUpdate) { ... }
    pub fn load_config(character_name: &str) -> Result<Self> { ... }
}
```

**Camp Loop Integration:**
- Clickies executor runs in recovery and combat phases
- Receives recommendations from DLL via IPC
- Validates item availability before execution
- Updates cooldown state after use

---

## IPC Protocol

```rust
pub enum ClickieMessage {
    // DLL → Orchestrator
    Recommendations(Vec<(String, Option<u32>)>), // (item_name, target_pid)
    
    // Orchestrator → DLL
    CooldownUpdate(HashMap<String, u64>), // item_name → cooldown_expires_at
    ReloadRules(CharacterClickieRules),
}
```

---

## TUI/Web Dashboard Integration

**REST Endpoints:**
```
GET  /api/clickies/inventory         → Current inventory + cooldown state
GET  /api/clickies/rules/:character  → Character clickie rules
POST /api/clickies/rules/:character  → Update/reload rules
POST /api/clickies/test-condition    → Evaluate condition against mock state
GET  /api/clickies/catalog           → Global item catalog
```

**Clickies Panel (TUI):**
```
┌──────────────────────────────────┐
│ CLICKIES (Active)                │
├──────────────────────────────────┤
│ Name              │ Charges │ CD │
├──────────────────────────────────┤
│ Greater Heal Pot  │ 5/5     │ ✓  │
│ Clarity Draught   │ 1/1     │ 24s│
│ Healing Wave      │ 45/50   │ ✓  │
└──────────────────────────────────┘

Recent Usage:
  23:45:12 - Used Greater Heal Pot (self HP 38% → 64%)
  23:44:58 - Used Clarity Draught
```

---

## Integration Points

### Camp Loop Phases

| Phase | Clickie Role | Example |
|-------|--------------|---------|
| **Pulling** | Pre-pull buffs | Clarity Draught |
| **Fighting** | Combat support | Heal potions, DPS clickies |
| **Recovery** | Post-pull healing | Mana potions, food |
| **Waiting** | Buff maintenance | Clarity Draught refresh |

### Event Triggers System

```toml
[[event_triggers]]
name = "cleric_group_heal"
trigger = "group_member_hp < 30%"
action = "use_item Healing Wave"
condition = "in_combat"
```

---

## Configuration Examples

### Cleric (Healer)
```toml
[[rotation.clickies]]
name = "Flash of Light"
priority = 100
condition = "target_hp_pct < 40"
target = "group"

[[rotation.clickies]]
name = "Greater Healing Potion"
priority = 90
condition = "(self_hp_pct < 50) AND cooldown_ready"
target = "self"
```

### Warrior (Tank)
```toml
[[rotation.clickies]]
name = "Healing Potion"
priority = 100
condition = "self_hp_pct < 60"
target = "self"

[[rotation.clickies]]
name = "Protection Scroll"
priority = 70
condition = "(group_in_combat) AND (NOT has_buff('Protection'))"
target = "self"
```

---

## RGMercs Comparison & Design Review

For a detailed analysis of how TextQuest's approach differs from the industry-standard RGMercs (Lua-based) clickies module, see **[rgmercs-review.md](rgmercs-review.md)**.

This review includes:
- Key functions and logic from rgmercs clickies.lua
- Design decisions where TextQuest diverges (Rust vs Lua, TOML vs Lua functions, async split, etc.)
- Lua-to-Rust mapping table for concepts and patterns
- Migration guide for RGMercs users

---

## Future Enhancements (Phase 2.2+)

1. Dynamic item discovery — scan inventory + bazaar
2. Smart inventory management — auto-buy/sell items
3. Group coordination — avoid waste/overcasting
4. Buff stacking logic — detect conflicts, prioritize best
5. Damage prediction — pre-use before spike damage
6. Survival mode — escalate to riskier items

---

## Summary

✅ **Flexible condition system** — compose from simple operators  
✅ **Per-character TOML config** — no code changes to customize  
✅ **DLL-orchestrator split** — recommendations in DLL, execution in orchestrator  
✅ **Inventory tracking** — verify before use, cooldown management  
✅ **Camp loop integration** — phase-aware usage  
✅ **TUI visibility** — real-time dashboard monitoring  
✅ **Extensible API** — plugins can define custom items  

**Blocks:** #1032 (Core implementation)  
**Effort:** 8-12 hours implementation  
