# Event Hooks Integration Guide

This document describes how to emit combat, movement, and loot events to the metrics collector.

## Overview

The `EventHookContext` provides a zero-copy interface for emitting events from game systems (combat, movement, loot) directly to the metrics collector. Events flow through a bounded MPSC channel into the collector for real-time fleet analytics.

## Architecture

```
Combat/Movement/Loot Systems
            ↓
    EventHookContext (thread-safe)
            ↓
    FleetEvent (typed enum)
            ↓
    MetricsCollector (MPSC channel)
            ↓
    FleetMetrics (aggregation + storage)
```

## Usage

### 1. Obtain the Event Hook Context

From the orchestrator loop or any component with access to `MetricsCollector`:

```rust
let metrics_collector = /* get from orchestrator */;
let hook_ctx = metrics_collector.event_hook_context();
```

### 2. Emit Events

#### Combat Round Event
Fired when a player completes a combat exchange (damage dealt and taken).

```rust
hook_ctx.emit_combat_round(
    pid,                    // Process ID of the game client
    damage_dealt,           // Total damage dealt in this round
    damage_taken,           // Total damage taken in this round
    duration_ms,            // Duration of the combat round in milliseconds
);
```

#### Kill Event
Fired when the player defeats a mob or player.

```rust
hook_ctx.emit_kill(
    source_pid,             // PID of the killing player
    target_name.to_string(),// Name of the defeated mob/player
    target_level,           // Level of the target
    zone.to_string(),       // Zone where the kill occurred
);
```

#### Death Event
Fired when the player is defeated.

```rust
hook_ctx.emit_death(
    pid,
    character_name.to_string(),
    zone.to_string(),
);
```

#### Loot Drop Event
Fired when the player picks up loot.

```rust
hook_ctx.emit_loot_drop(
    pid,
    item_name.to_string(),  // Name of the item looted
    item_id,                // Unique item ID
    zone.to_string(),       // Zone where loot was picked up
);
```

#### Zone Change Event
Fired when the player moves between zones (movement tracking).

```rust
hook_ctx.emit_zone_change(
    pid,
    from_zone.to_string(),
    to_zone.to_string(),
);
```

## Performance Considerations

- Events are sent over a bounded MPSC channel (default 4,096 capacity)
- Failed sends (full channel or disconnected) are silently ignored
- Timestamps are captured at emission time (Unix epoch seconds)
- No blocking operations — try_send() is non-blocking

## Fleet Metrics Updated

The collector automatically aggregates these events into fleet-level metrics:

- `total_damage_dealt` — Sum of all damage across all players
- `total_damage_taken` — Sum of all damage taken
- `total_kills` — Kill count for the fleet
- `total_deaths` — Death count for the fleet
- `total_items_looted` — Total loot pickups
- `fleet_dps` — Calculated fleet damage per second

These metrics are available via `MetricsCollector::fleet_metrics()`.

## Integration Points

### Orchestrator Loop
The orchestrator loop calls `metrics_collector.tick()` once per tick, which automatically drains and processes fleet events.

### Combat System
Hook into combat completion to emit `emit_combat_round()`.

### Movement System
Hook into zone changes or coordinate updates to emit `emit_zone_change()`.

### Loot System
Hook into item pickup handlers to emit `emit_loot_drop()`.

## Testing

Use `EventHookContext::new(None)` to create a hook context without a channel for testing. All emit calls will be no-ops.

```rust
#[test]
fn test_combat_without_channel() {
    let ctx = EventHookContext::new(None);
    ctx.emit_combat_round(1, 100, 50, 1000); // No panic
}
```
