# Say Detection and Alerting

Provides MQ2Say-style `/say` monitoring for fast quest-text, warning-message, and player-contact detection.

## Overview

Say Detection watches only the `/say` channel and evaluates a rule list every orchestrator pulse. Each rule can either:

- fire an alert through the configured alert channels
- run a local slash command on the client that saw the line
- broadcast a slash command to every connected client

This complements the broader event-trigger system by specializing in low-latency `/say` text handling.

## Configuration

Rules are stored in `config/textquest.toml`:

```toml
[say_detection]
enabled = true
sound_enabled = true
sound_file = "say_alert.wav"
toast_enabled = true
discord_webhook_url = ""
broadcast_all_clients = false

[[say_detection.rules]]
name = "Quest hail"
pattern = "I have been waiting for you"
pattern_type = "substring"
action_type = "alert"
enabled = true

[[say_detection.rules]]
name = "Broadcast warning"
pattern = "\\bwarning\\b"
pattern_type = "regex"
action_type = "broadcast"
action_value = "/bc Incoming warning text"
enabled = true

[[say_detection.rules]]
name = "Reply to herald"
pattern = "hail"
pattern_type = "exact"
action_type = "command"
action_value = "/say Ready"
enabled = true
```

## Alert Routing

Rules with `action_type = "alert"` use the top-level say-detection routing settings:

- `sound_enabled`
- `sound_file`
- `toast_enabled`
- `discord_webhook_url`
- `broadcast_all_clients`

The orchestrator always logs alert matches and additionally routes Discord alerts when a webhook is configured. When `broadcast_all_clients` is enabled, alert-style matches are mirrored to every connected client with `/echo`.

## Web UI

The web dashboard exposes a dedicated **Say Detection** panel with:

- engine enable/disable toggle
- alert-routing controls
- inline rule creation
- per-rule editing for pattern type and action type
- match summary (total matches and last matched rule)

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/say-detection/status` | Current config plus aggregate match status |
| GET | `/api/say-detection/config` | Read persisted say-detection config |
| PUT | `/api/say-detection/config` | Persist say-detection config |
| GET | `/api/say-detection/rules` | Read the current rule list |
| POST | `/api/say-detection/rules` | Append a single rule |
| DELETE | `/api/say-detection/rules/{rule_name}` | Delete a rule by name |
| POST | `/api/say-detection/sync` | Sync a match event into web status state |

## Validation

- `/say` matches should be observed within one orchestrator pulse.
- Alert rules should log immediately and route Discord/broadcast delivery when configured.
- Command rules should execute on the detecting client only.
- Broadcast rules should execute on all connected clients.

## Related

- [MQ2 Coverage Gap Analysis](../MQ2_COVERAGE_GAP_ANALYSIS.md#f5)
- [Rare Spawn Alert System](Rare-Spawn-Alert-System)
- [Web Dashboard Operator Console](Web-Dashboard-Operator-Console)
