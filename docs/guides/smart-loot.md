# Smart Loot Guide

> **Phase 3.2d** — Smart loot filtering system for the TextQuest automation platform.
> Implementation reference: [`textquest/src/camp/loot.rs`](../../textquest/src/camp/loot.rs)

## Overview

TextQuest's smart loot system automates corpse looting after kills. It uses a
finite-state machine (FSM) to target each corpse, approach, open the loot
window, pick up items, and clean up — all driven by slash commands that flow
through the IPC → DLL → InterpretCmd pipeline.

Timing is humanized per character via `PersonalityProfile` so that characters
with different `reaction_speed` values loot at different rates, avoiding
synchronized bot-like behavior.

---

## Core Types

### `LootRules`

Controls *what* gets picked up.

| Field | Type | Default | Purpose |
|---|---|---|---|
| `keep_items` | `HashSet<String>` | empty | Always pick up and keep. |
| `sell_items` | `HashSet<String>` | empty | Pick up for vendor sale. |
| `destroy_items` | `HashSet<String>` | empty | Pick up and destroy (clears junk). |
| `loot_all` | `bool` | `true` | If true, loot everything not in `destroy_items`. |
| `auto_split` | `bool` | `true` | Auto-split coin with group. |

**Priority order** (highest to lowest):

1. `destroy_items` — always wins, even if the item is also in `keep_items`
2. `keep_items`
3. `sell_items`
4. Item-score fallback (`classify_item_with_score`) → Keep if upgrade, Sell if not
5. `loot_all` → Keep everything remaining
6. Ignore

### `LootConfig`

Controls *how fast* looting happens (all delays are in ticks).

```toml
[loot]
item_pickup_delay  = 2   # wait between item pickups
target_delay       = 1   # wait after targeting corpse
approach_delay     = 3   # wait while walking to corpse
loot_open_delay    = 2   # wait after opening loot window
close_delay        = 1   # wait after looting before moving on
hide_looted_corpses = true
```

### `LootCycle`

The FSM driving the full corpse sequence.

```
TargetCorpse
  → ApproachCorpse  (waits approach_delay ticks)
  → OpenLoot        (waits loot_open_delay ticks)
  → LootItems       (waits item_pickup_delay ticks; issues /lootall or individual pickups)
  → CloseLoot       (waits close_delay ticks; hides corpse if configured)
  → NextCorpse      (advances queue index)
  → Done
```

---

## Item Classification

### `classify_item(item_name, rules) → ItemAction`

Pure function — no DB lookup needed. Checks explicit lists in priority order:

```rust
let action = classify_item("Fine Steel Dagger", &rules);
// ItemAction::Keep  (in keep_items)

let action = classify_item("Spider Legs", &rules);
// ItemAction::Destroy  (in destroy_items)

let action = classify_item("Unknown Widget", &rules);
// ItemAction::Ignore  (not found, loot_all=false)
```

### `classify_item_with_score(item_name, rules, comparison) → ItemAction`

Falls back to item-score data when explicit rules produce `Ignore`:

```rust
// Item not in any list, but the score engine says it's an upgrade
let action = classify_item_with_score("Gleaming Sword", &rules, Some(&score));
// ItemAction::Keep  (is_upgrade = true)

// Item not in any list, not an upgrade — sell for plat
let action = classify_item_with_score("Rusty Buckler", &rules, Some(&score));
// ItemAction::Sell  (is_upgrade = false)

// No score available at all
let action = classify_item_with_score("Mystery Orb", &rules, None);
// ItemAction::Ignore
```

---

## Example Configurations

### 1. Keep valuable drops, sell the rest (camp farming)

```json
{
  "loot_all": false,
  "auto_split": true,
  "keep_items": [
    "Bone Chips",
    "Bat Wing",
    "Rune of Conception"
  ],
  "sell_items": [
    "Cracked Staff",
    "Rusty Dagger",
    "Tattered Cloth Robe"
  ],
  "destroy_items": [
    "Spider Legs",
    "Snake Scales"
  ]
}
```

### 2. Loot everything (new zone, unknown drops)

```json
{
  "loot_all": true,
  "auto_split": true,
  "keep_items": [],
  "sell_items": [],
  "destroy_items": ["Rat Whiskers"]
}
```

### 3. Score-driven selective looting (gear upgrade pass)

Set `loot_all = false` and leave item lists empty. The system will call
`classify_item_with_score` for each item and keep only confirmed upgrades,
selling everything else.

```json
{
  "loot_all": false,
  "auto_split": true,
  "keep_items": [],
  "sell_items": [],
  "destroy_items": []
}
```

---

## Loot and Scoot Timing

Each delay field is a *base tick count* scaled by the character's
`reaction_speed` from `PersonalityProfile`:

```
adjusted_delay = round(base_delay * reaction_speed)
```

| reaction_speed | approach_delay=3 → actual |
|---|---|
| 0.7 (fast) | 2 ticks |
| 1.0 (neutral) | 3 ticks |
| 1.5 (slow) | 5 ticks |

This prevents all characters from looting in lockstep. For fast camp-clearing
(e.g., after an AoE pull), lower the base delays:

```toml
approach_delay   = 1
loot_open_delay  = 1
item_pickup_delay = 1
close_delay      = 1
```

---

## Leave-for-Puller / Coin-Split Behavior

- When `auto_split = true`, a `/autosplit` command is issued during
  `LootItems` so coin is shared with the group automatically.
- Set `auto_split = false` in solo sessions or when coin should not be split.
- To leave corpses visible (e.g., for the puller to check), set
  `hide_looted_corpses = false`.

---

## Testing

Run the loot-specific unit tests:

```bash
cargo test --lib -p textquest camp::loot
```

Run the item-score integration tests (requires SQLite fixture):

```bash
cargo test --test item_score
```

---

## Architecture Notes

- All slash commands are issued as `(pid, command)` pairs and delivered via
  the IPC channel — the FSM itself has no direct EQ dependency.
- `LootCycle::tick()` is a pure side-effect-free function: same inputs always
  produce the same commands and state transitions.
- `LootRules` is serde-serializable; store it in the per-session config JSON
  that the orchestrator loads at startup.
- Concurrent looting by multiple characters is handled at the orchestrator
  level by assigning one `LootCycle` per `looter_pid`.
