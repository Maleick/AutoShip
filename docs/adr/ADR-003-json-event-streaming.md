# ADR-003: JSON Event Streaming

**Status:** Accepted  
**Date:** 2026-04-18  
**Author:** TextQuest Team

## Context

TextQuest's autonomous sessions produce thousands of events:

- Combat actions (spell cast, ability used, DPS dealt)
- Zone transitions (left zone X, entered zone Y)
- Inventory changes (loot picked up, item sold)
- Character status (death, resurrection, buff applied)
- Error events (DC, timeout, permission denied)

Operators need to:

1. **Audit** — Replay a session to understand what went wrong
2. **Monitor** — Watch live events via Discord or web dashboard
3. **Query** — Run analytics: "How many DPS events from Mage1 in the last hour?"
4. **Archive** — Store sessions for compliance and learning

Traditional approach: Log to rotating text files with printf-style formatting. Problems:

- Unstructured text is hard to parse and query
- Mixing log levels (INFO, ERROR, DEBUG) makes filtering difficult
- Binary format (protobuf, msgpack) requires schema versioning and backward-compatibility
- Central database requires upfront schema design and is not append-only

## Decision

Implement **JSONL (JSON Lines) append-only event logging**:

Each event is written as a single JSON object on its own line. The file grows monotonically and is never rewritten.

```jsonl
{"timestamp":"2026-04-18T14:30:00Z","event_type":"spell_cast","character":"Camrene","spell_name":"Spell: Charm","target":"orc_pawn"}
{"timestamp":"2026-04-18T14:30:01Z","event_type":"ability_used","character":"Zisdarenu","ability":"Lay on Hands","target":"Camrene","healing":450}
{"timestamp":"2026-04-18T14:30:02Z","event_type":"loot_picked_up","character":"Camrene","item":"Crafted Silk Sleeves","item_id":12345}
{"timestamp":"2026-04-18T14:30:03Z","event_type":"character_died","character":"Toon03","zone":"The Sebilis Citadel","killer":"guardian_statue"}
```

Each event carries:

- **timestamp** — ISO 8601 UTC (sortable, ISO-queryable)
- **event_type** — Discriminator (spell_cast, loot, death, etc.)
- **character** — Which character performed the action
- **...other fields** — Specific to event_type (spell_name, target, healing, etc.)

Events are written by the orchestrator to `data/events-YYYYMMDD-HHmm.jsonl` (rotated every N lines or hours).

## Rationale

1. **Append-only** — Guarantees durability and immutability. New events are appended; old events never change. Safe for concurrent readers (e.g., live dashboard queries).

2. **Human-readable** — Each line is valid JSON. Can be inspected with `jq`, `grep`, or `tail -f`. No binary decoding required.

3. **Extensible** — Adding new event fields doesn't require schema migration. Clients that don't recognize a field ignore it.

4. **Queryable** — Event log can be indexed into SQLite (`LOAD JSON INTO TABLE`) or streamed through aggregation pipelines. Analytics tools (pandas, elk, clickhouse) read JSONL natively.

5. **Replay-safe** — Sorted by timestamp, events can be replayed in order to reconstruct session state.

6. **Archive-friendly** — JSONL files compress well (gzip) and are self-describing. No separate schema file needed.

## Implementation Notes

- Events are written **atomically** (one line per write, flush after each line to ensure durability).
- Timestamp is assigned by the orchestrator when the event occurs, not when it's written. This ensures correct causal ordering even with buffering.
- If an event is malformed or missing a required field, the orchestrator logs a warning and skips the write (rather than crashing).
- Event files are rotated when they exceed ~100MB or after 24 hours.
- Old event files are archived to `data/archive/` and optionally compressed.

Example event schema (non-exhaustive):

```rust
#[derive(Serialize)]
struct Event {
    timestamp: String,           // ISO 8601
    event_type: String,          // "spell_cast", "loot", "death", etc.
    character: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>, // flexible nested data
}
```

## Alternatives Considered

1. **Binary format (protobuf, bincode)** — More efficient storage/parsing, but requires schema versioning and isn't human-readable.
2. **Centralized time-series DB (InfluxDB, Prometheus)** — Excellent for metrics, but overkill for event audit trail; requires external service.
3. **Rotating text logs (printf-style)** — Simple but unstructured; hard to parse and query.
4. **SQLite directly** — Schema is rigid; harder to extend; write amplification from WAL.

## Related ADRs

- ADR-006: Metrics with Percentiles (aggregates histogram data from events)
- ADR-004: Per-Account Test Runners (scenarios emit events to JSONL)

## References

- `textquest/src/event_log.rs` — Event writing implementation
- `textquest/src/event.rs` — Event type definitions (cast, loot, death, etc.)
- `data/events-*.jsonl` — Live event files
