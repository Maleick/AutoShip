# Zone Transition Packet Audit

## Overview

This document audits EverQuest network packets involved in zone transitions and documents opcode gaps requiring further research. Zone transitions are critical orchestration events in TextQuest: they mark state resets, combat interruptions, and navigation state changes.

## Zone Transition Flow

EQ zone transitions follow this sequence:

1. **OP_ZoneChange** (S→C, code: variable) — Server initiates zone transition
2. **OP_ClientUpdate** (C→S, code: variable) — Client acknowledges and sends position/state
3. **OP_ZoneEntry** (S→C, code: 0x42 / 66) — Server sends zone entry data
4. **OP_SendAAStats** (S→C, code: variable) — AA state for new zone
5. **OP_SpawnAppearance** (S→C, code: variable) — Initial spawn appearance updates
6. Game loop resumes with spawn list, ground items, doors, objects

## Zone Handler Functions

### Offset Database References

Zone handling is orchestrated through three primary EQ functions (from `textquest-common/src/offsets.rs`):

| Function         | Address          | Status     | Purpose                          |
| ---------------- | ---------------- | ---------- | -------------------------------- |
| `EQ_BEGIN_ZONE`  | 0x0001_4028_D0E0 | ✓ Known    | Begin zone transition (pre-load) |
| `EQ_END_ZONE`    | 0x0              | ⚠️ Unknown | Finalize zone unload             |
| `EQ_FINISH_ZONE` | 0x0              | ⚠️ Unknown | Zone load completion             |
| `EQ_ZONE_CHANGE` | 0x0              | ⚠️ Unknown | Zone change event callback       |

These are wrapped in `textquest-common/src/offset_db.rs`:

- Registered as function lookups: `eqBeginZone`, `eqEndZone`, `eqFinishZone`, `eqZoneChange`
- Exposed to the Ghidra database for dynamic offset resolution
- Callsites may pass zone ID, spawn list base, or player state

### Game State Machine

Zone transitions trigger state machine resets in `textquest-common/src/soul.rs`:

```rust
SoulEvent::ZoneEnter { zone: String }  // Marks new zone entry
```

This event is critical for:

- Combat loop resets (all buffs/debuffs cleared in new zone)
- Spawn list rebuild (old spawn list invalidated)
- Navigation state reset (current path/camp cleared)

## Packet Opcode Audit

### Opcodes: Known

| Opcode | Direction | Hex  | Name            | Handler | Notes                                                                     |
| ------ | --------- | ---- | --------------- | ------- | ------------------------------------------------------------------------- |
| 66     | S→C       | 0x42 | OP_ZoneEntry    | Unknown | Zone entry data payload; server→client only. Triggers zone load sequence. |
| 128    | C→S       | 0x80 | OP_ClientUpdate | Unknown | Player position/state acknowledged in new zone; client→server only.       |

**Source**: `textquest-common/src/ghidra_db.rs` test fixtures (lines 758–759).

### Opcodes: Named but Unknown

The following opcode names appear in EverQuest documentation but their numeric codes are not yet confirmed in the TextQuest codebase:

| Opcode             | Direction | Purpose                                    | Status          |
| ------------------ | --------- | ------------------------------------------ | --------------- |
| OP_ZoneChange      | S→C       | Server-initiated zone change (begin state) | ⚠️ Code unknown |
| OP_SendAAStats     | S→C       | AA ability state for new zone              | ⚠️ Code unknown |
| OP_SpawnAppearance | S→C       | NPC/PC appearance data (initial/update)    | ⚠️ Code unknown |
| OP_NewZone         | S→C       | Alternative naming for zone entry          | ⚠️ Code unknown |
| OP_ZoneSpawns      | S→C       | Spawn list transmission                    | ⚠️ Code unknown |

### Opcodes: Common but Unconfirmed in Zone Context

These packets are known to occur during zone transitions but are not explicitly catalogued:

| Opcode        | Direction | Context                                         | Research Required                            |
| ------------- | --------- | ----------------------------------------------- | -------------------------------------------- |
| SendAAStats   | S→C       | AA state updates during zone load               | Research exact opcode numeric value          |
| MemorizeSpell | S→C       | Spell gem state reset                           | Verify inclusion in zone transition sequence |
| ExpUpdate     | S→C       | Experience updates after zone entry             | Verify timing relative to ZoneEntry          |
| PlayerProfile | S→C       | Full character state (contingent on zone rules) | Timing in TLP vs. Live EQ                    |
| PickupRequest | C→S       | Loot pickup (zone-dependent restrictions)       | Verify zone-specific rules                   |

## Zone Packet Capture Infrastructure

TextQuest has packet-capture code paths and operator plumbing, but the current
checkout does not install `packet_hook` during normal DLL startup. Treat the
items below as repo-grounded implementation detail, not as live-proof that the
packet monitor is active on every build.

### Capture Mechanisms

1. **WSASend/WSARecv hook code** (`textquest-dll/src/hooks/packet_hook.rs`):
   - `packet_hook::install()` resolves ws2_32.dll exports and enables retour detours
   - Captures packets before scrambler (outbound) and after descrambler (inbound)
   - Uses retour static detours for atomic trampoline installation
   - Forwards opcode + direction + timestamp to orchestrator via IPC
   - Current blocker: `textquest-dll/src/lib.rs` does not call `packet_hook::install()` during normal DLL init

2. **Packet ring buffer** (`textquest-common/src/packet.rs`):
   - `CaptureSession`: bounded ring buffer with configurable capacity
   - `PacketFilter`: whitelist/blacklist/direction-based filtering
   - Binary serialization (custom DMPC format) + JSON lines export
   - Session diffing for behavioral analysis

### IPC Protocol

From `textquest-common/src/ipc.rs`:

```rust
/// Captured packet event from the DLL's send/recv hooks.
DllEvent::PacketEvent {
    client_id: ClientId,
    opcode: u16,
    direction: PacketDirection,  // Inbound or Outbound
    timestamp_ms: u64,
    payload_size: u32,
}

/// Batched response: `Command::PollPackets` returns accumulated events.
Response::PacketBatch {
    events: Vec<PacketEvent>,
}
```

## Zone Graph Infrastructure

From `textquest-common/src/nav.rs` and offsets:

- **ZoneGuideManagerClient** at `0x140FAA40` (2026-03-10 client) — singleton manager
- **ZoneGuideZone** (0x48 bytes each) — per-zone data structure
- **ZoneGuideConnection** (0x14 bytes each) — adjacency information

Zone transitions must respect the graph loaded via `QueryZoneGraph` command (from `textquest-common/src/ipc.rs`).

## Research Gaps & Sub-Issues

The following opcodes and zone transition behaviors require dedicated research:

### Priority 1: Zone Entry Sequence

- [ ] **G1.1** — Confirm numeric opcode for `OP_ZoneChange` (server initiates)
- [ ] **G1.2** — Confirm numeric opcode for `OP_ZoneEntry` (zone data payload) — _assumed 0x42 / 66_
- [ ] **G1.3** — Confirm numeric opcode for `OP_ClientUpdate` (client ACK) — _assumed 0x80 / 128_
- [ ] **G1.4** — Trace the call sequence: `eqBeginZone` → `eqZoneChange` → `eqFinishZone`

### Priority 2: Zone State Packets

- [ ] **G2.1** — Confirm `OP_SendAAStats` opcode value; document AA reset behavior
- [ ] **G2.2** — Confirm `OP_SpawnAppearance` opcode value and zone entry timing
- [ ] **G2.3** — Verify `OP_NewZone` existence vs. `OP_ZoneEntry` naming inconsistency
- [ ] **G2.4** — Research packet order: is `SendAAStats` before or after spawn list begins?

### Priority 3: Zone Transition Edge Cases

- [ ] **G3.1** — Zone transition with corpse in zone (OP_Corpse handling)
- [ ] **G3.2** — Zone transition with active buffs (persist or cleared?)
- [ ] **G3.3** — Zone transition with group members (OP_GroupUpdate behavior)
- [ ] **G3.4** — Long zone load sequence: max opcode count and timeout strategy

### Priority 4: TLP vs. Live Behavior

- [ ] **G4.1** — Verify opcode consistency across EQ Live vs. TLP progression servers
- [ ] **G4.2** — Confirm zone transition behavior for instanced zones (Guild Hall, Planes)
- [ ] **G4.3** — Research inter-server zone transitions (teleports, boats)

## Implementation Checklist

### Code Artifacts

- ~ Packet capture code path: `textquest-dll/src/hooks/packet_hook.rs` (WSASend/WSARecv detours exist, but normal DLL startup does not currently install them)
- ✓ Ring buffer: `textquest-common/src/packet.rs` (CaptureSession, filtering, persistence)
- ✓ IPC protocol: `textquest-common/src/ipc.rs` (PacketEvent, PacketBatch responses)
- ✓ Zone handlers: `textquest-common/src/offsets.rs` (EQ_BEGIN_ZONE, EQ_ZONE_CHANGE offsets)
- ✓ Zone graph: `textquest-common/src/nav.rs` (ZoneGuideManagerClient, ZoneGuideZone layout)

### Offset Database

- ✓ Ghidra DB schema: `textquest-common/src/ghidra_db.rs` (opcode table with direction + handler)
- ✓ Test fixtures: zones 0x42 / 0x80 registered as example opcodes

### Testing

- ✓ Packet filter tests: `textquest-common/src/packet.rs` — whitelist/blacklist/direction
- ✓ Session serialization: binary round-trip + JSON lines export
- ✓ Ring buffer eviction: LRU replacement policy validated

## Existing Research References

- `docs/research/cheater-ld-flag.md` — Anti-cheat detection; unrelated to zone transitions
- `docs/research/hook-detection-surface.md` — Hook stealthiness; packet hook uses retour (HIGH detection risk)

## Recommended Next Steps

1. **Live capture session**: Hook onto an active EQ TLP client during zone transition; capture opcode sequence
2. **Opcode mapping**: Cross-reference captured opcodes against MacroQuest's opcode header (eqlib)
3. **Offset confirmation**: Use Ghidra to trace `eqZoneChange` and `eqFinishZone` function calls; confirm offsets
4. **Sub-issue creation**: File issues G1.1–G4.3 as blocking work; each with expected duration and evidence requirements
5. **Automation**: Enable `CaptureSession` in DLL on zone-entry IPC commands; persist to `textquest.log` + Ghidra DB

## Conclusion

Zone transition packets are well-understood at the protocol level (EQ streams
opcodes in a fixed sequence), but TextQuest's implementation remains incomplete.
The repo has packet-capture code, IPC batching, and a packet-monitor UI, but
the current checkout does not install `packet_hook` during normal DLL startup,
and opcode numeric values are only partially mapped. Filling these gaps still
requires live packet capture from a TLP server and cross-referencing against
EQ's internal definitions (via Ghidra or MacroQuest).
