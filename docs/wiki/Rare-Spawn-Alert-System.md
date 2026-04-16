# Rare Spawn Alert System

Provides MQ2SpawnMaster parity for detecting and alerting on named NPC spawns.

## Overview

The Rare Spawn Alert System watches for specific named NPCs in the zone and fires alerts via sound, TUI notifications, web notifications, and optional broadcast to all clients.

## Configuration

Watch patterns are defined in `config/textquest.toml`:

```toml
[spawn_alerts]
enabled = true
sound_enabled = true
sound_file = "rare_spawn.wav"
tui_notification = true
web_notification = true
broadcast_to_all = false

[[spawn_alerts.watch_patterns]]
pattern = "Tunare"        # Glob-style pattern
zone = "froglok_caverns"   # Optional: limit to specific zone
spawn_sound = "tunare.wav"
```

## TUI Integration

When a watched NPC spawns:
1. Sound plays (configurable sound file)
2. Toast notification appears in the TUI
3. Event logged to spawn events panel
4. Time since last pop is tracked

### Despawn Notifications

When a watched NPC is killed or despawns:
- TUI notification confirms despawn
- Spawn history is updated

## Web UI

Access via the Spawn Alerts panel in the web dashboard (`http://localhost:3000`).

### Features
- View watch list patterns
- Add/edit/remove watch patterns
- View spawn history with timestamps
- Configure notification preferences
- Time since last pop display

## API Endpoints

### Watch Patterns

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/spawn-alerts/patterns` | List all watch patterns |
| POST | `/api/spawn-alerts/patterns` | Add new watch pattern |
| PUT | `/api/spawn-alerts/patterns/:id` | Update pattern |
| DELETE | `/api/spawn-alerts/patterns/:id` | Remove pattern |

### Spawn Events

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/spawn-alerts/events` | List spawn events (paginated) |
| GET | `/api/spawn-alerts/stats` | Get spawn statistics |

### Request/Response Examples

```json
// POST /api/spawn-alerts/patterns
{
  "pattern": "Tunare",
  "zone": "froglok_caverns",
  "spawn_sound": "tunare.wav"
}
```

```json
// GET /api/spawn-alerts/events?limit=50
{
  "total": 142,
  "offset": 0,
  "limit": 50,
  "entries": [
    {
      "id": 1,
      "spawn_name": "Tunare",
      "zone": "froglok_caverns",
      "event_type": "spawn",
      "timestamp": "2026-04-16T10:30:00Z"
    }
  ]
}
```

## IPC Events

Spawn alert events are published via IPC for cross-client coordination:

```rust
enum IpcEvent {
    SpawnAlert(SpawnAlertEntry),
    // ...
}
```

## Related

- [MQ2SpawnMaster](https://github.com/RedGuides/MQ2SpawnMaster)
- [Named Tracking (#796)](https://github.com/Maleick/TextQuest/issues/796)
- [MQ2 Coverage Gap Analysis](../MQ2_COVERAGE_GAP_ANALYSIS.md#c3)
