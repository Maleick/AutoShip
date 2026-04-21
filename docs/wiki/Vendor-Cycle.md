# Vendor Cycle: Loot Routing

## Overview

The vendor cycle converts low-priority inventory into plat during camp downtime. It is implemented in three layers:

- `textquest/src/loot/vendor_cycle.rs` (pure planning: which items are sellable)
- `textquest/src/camp/vendor.rs` (sell FSM and runtime execution)
- `textquest/src/orchestrator/mod.rs` (activation timing in camp mode)

This page documents operator-visible behavior and configuration surfaces used by those components.

## Operator Guide: How it works

### What gets sold

`VendorCyclePlanner::plan` evaluates each `VendorInventoryItem` and queues it when any rule matches:

- `is_trash` is true
- item age is older than `backlog_days`
- item is a duplicate copy of another non-protected entry (only the newest copy per name is kept)
- allowlist mode is active and the item is on the allowlist

The planner does not sell:

- protected items (`item.protected == true`)
- items whose normalized name is in the keep-list
- items with no active sell reason

`VendorPlan` includes `items` plus `estimated_gross_plat`, the sum of each selected item's `estimated_vendor_value`.

### When it activates

Activation is hard-coded by orchestrator flow:

1. A sell cycle must be configured by calling `Orchestrator::start_sell_cycle(...)`.
2. Camp mode must be active and the current camp state must be `Idle` or `Medding`.
3. The tick-based timer must be satisfied: `current_tick - last_sell_tick >= sell_interval_ticks` and state must be `SellState::Idle`.
4. If `sell_queue` is empty and `vendor_inventory` is not, the controller builds a fresh plan with `SellCycle::prepare_sell_plan`.
5. The state machine enters `Navigating -> Selling -> Returning`.

The cycle still uses `seller_pid` from the active camp member list and only emits commands when those members are in routing scope.

## Configuration Reference

### Planner surface (`VendorCyclePlanner`)

Located in `textquest/src/loot/vendor_cycle.rs`.

- `VendorCyclePlanner::new(keep_items)`
  - Initializes the keep-list.
  - Name normalization is `trim().to_ascii_lowercase()`.
  - Protected items are never selected for sale.
- `with_allowlist(sell_items)`
  - Non-empty allowlist switches planner into allowlist mode.
  - With allowlist active, non-matching names are skipped before any other selection logic.
- `with_backlog_days(days)`
  - Sets age-based cutoff in days.
  - Negative values are clamped to `0`.
  - Duplicate detection and trash checks still apply.

### Runtime cycle surface (`VendorConfig`)

Defined in `textquest/src/camp/vendor.rs` and consumed by `SellCycle::new`.

- `vendor_name`: NPC used for targeting/interaction commands
- `sell_interval_ticks`: minimum ticks between automatic cycles
- `keep_items`: names passed into `VendorCyclePlanner::new`
- `sellable_items`: names passed into `with_allowlist`
- `travel_ticks`: fallback travel delay before opening the vendor window
- `sell_step_delay`: delay between seller FSM sub-steps
- `return_spell`: optional return spell/gem string
- `navigation_timeout_ticks`: timeout while navigating to vendor
- `vendor_retry_ticks` and `max_busy_retries`: handling for vendor window busy states
- `backlog_days`: forwarded into planner
- `watch_items`: vendor browse items (price watcher)

### Inventory input paths

- `VendorInventoryItem::try_from(&ContainerSlotInfo)` for live IPC inventory conversion.
- `VendorInventoryItem::from_loot_history(row, estimated_value)` when replaying persisted loot rows.
- `VendorInventoryItem::loot(...)` / `VendorInventoryItem::trash(...)` for constructed snapshots.
- `VendorInventoryItem::protected()` to force keep behavior.

## Overnight farming integration

There is no separate overnight controller in this module. The integration point is the camp downtime cadence:

- During long camps, the cycle runs at most every `sell_interval_ticks` when in downtime.
- This is the intended path for overnight farm cleanup and plat conversion.

Use this to reduce post-run manual sorting while preserving downtime availability for recovery tasks.

## Tradeskill trophy integration

There is no dedicated trophy-specific branch in the planner.

Operators should protect trophies through normal keep-list behavior:

- Add known trophy names to `keep_items` while farming for stock.
- Remove or change the keep-list once you intentionally want liquidation.
- If using explicit sell logic, configure `sellable_items` to avoid accidental conversion of active trophy inventory.

This pattern also applies to other persistent tradeskill materials.

## Related runtime behavior notes

- `SellCycle` executes with fixed phases and bounded retries (`navigation_timeout_ticks`, `vendor_retry_ticks`, `max_busy_retries`).
- `orchestrator::tick_sell_cycle` returns no commands when camp is not in downtime.
- Vendor browsing alerts use `VendorConfig.watch_items` and appear in vendor scan processing.

## Known limitations

- No dynamic market price lookup; `estimated_vendor_value` is static metadata.
- No bazaar posting path in this planner/controller.
- Merchant choice is external: operator controls the configured merchant target.

## Test hooks

- Planning behavior is covered by planner tests in `textquest/src/loot/mod.rs`.
- Execution and integration behavior is covered by tests in `textquest/src/camp/vendor.rs` and `textquest/src/orchestrator/mod.rs`.

### Suggested test command

```bash
cargo test -p textquest --lib loot::vendor_cycle
```

This repository also has dedicated integration tests under `textquest/src/camp/vendor.rs` and `textquest/src/orchestrator/mod.rs`.

## References

- Source: `textquest/src/loot/vendor_cycle.rs`
- Runtime vendor controller: `textquest/src/camp/vendor.rs`
- Orchestrator integration and timing: `textquest/src/orchestrator/mod.rs`
- [Combat and Camp Loop](Combat-and-Camp-Loop)
