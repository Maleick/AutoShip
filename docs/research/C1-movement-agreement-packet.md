# C1 — Movement-Agreement Packet: Opcode + Field Layout

**Issue:** #2184
**Parent:** #2174
**Date:** 2026-04-21
**Status:** Research — partial evidence. Per-frame movement opcode `0x1643` confirmed; movement-history summary opcode unresolved; field layout derived from EQEmu + binary
**Evidence grade:** SME-reported (RTTI class confirmed) + EQEmu-sourced (struct layout) + Ghidra-partial (RTTI xref exists, serialization site not yet captured)

---

## Problem Statement

The movement-agreement system fires every ~15 ms and sends position summaries to the server
every ~1 second. The server holds a `CMovementHistoryClientException` handler (RTTI class
confirmed by Matt / Ghidra — see `Research-Anti-Detection.md` §Open Questions Q4). The server
cross-checks position, velocity, and path against its own predicted state. Any drift triggers
the exception, which presumably causes a kick or flag.

Current humanization (`textquest-dll/src/nav/humanize.rs`, `textquest-dll/src/combat/humanize.rs`)
applies jitter to heading (±1–4 EQ degrees), speed (±7% multiplier), and timing without knowing
the actual field-level tolerance the server enforces. This document consolidates what is known,
what is inferred, and what gaps remain for live validation.

---

## Opcode

### Known / Confirmed

| Opcode   | Direction | Evidence                                                          | Notes                                                                                                                                                       |
| -------- | --------- | ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `0x1643` | C→S       | `Research-EQ-Network-Architecture.md`; MQ2reachit hooks this path | Used by `UdpConnection::SendMessage` — the alternate send path distinct from `NetworkSend` at `0x140550030`. Position packets go via this lower-level path. |

### Partially Resolved / Awaiting Matt

The **movement-agreement** (path-summary, 1/sec) packet is a distinct higher-level construct
layered on top of or alongside the per-frame position update (`0x1643`). The distinction:

- **Per-frame position update** (`0x1643`): raw position + velocity; fires every ~15 ms when
  moving. Used by `UdpConnection::SendMessage`. This opcode is confirmed in the network
  architecture research.
- **Movement-history summary**: aggregated path report that the server uses for warp detection.
  The RTTI class `CMovementHistoryClientException` confirms this is a distinct validation
  concept, but the exact opcode for the history/summary packet has **not yet been extracted
  from binary**. Matt's input on this opcode is pending (#2184 scope).

**Working assumption:** the "movement agreement packet" the issue refers to is the per-frame
update at `0x1643`, which the server accumulates server-side to build the movement history
it compares against. The `CMovementHistoryClientException` fires when the server's reconstructed
path diverges from the client-reported path. A separate history-submission opcode may or may not
exist as a distinct C→S packet.

---

## Field Layout

### `PlayerPositionUpdateClient_Struct` (EQEmu canonical, client 20260310-compatible)

This struct is the payload of the per-frame position update (`0x1643` / `OP_ClientUpdate`).
Layout derived from `eq_packet_structs.h` (EQEmu) and cross-referenced with `Research-EQ-Protocol.md`.

```
Offset  Size  Type         Field                Notes
------  ----  -----------  -------------------  -----------------------------------------------
0       2     uint16       sequence             Monotonically incrementing per-send counter.
                                                Server detects gaps / replay by tracking this.
2       2     uint16       spawn_id             Self spawn ID — must match server-side session.
4       2     uint16       vehicle_id           0 = no vehicle; non-zero = mounted/in-vehicle.
6       4     float32      delta_x              Velocity component in EQ X axis (E/W).
                                                Units: EQ units/frame (approximately ~0.3/frame
                                                at run speed for a standard character).
10      4     float32      delta_z              Velocity component in EQ Z axis (vertical).
14      4     float32      delta_y              Velocity component in EQ Y axis (N/S).
18      12b   bitfield     heading              12-bit packed heading. Range 0–4095; maps to
                                                0–360°. TextQuest uses 0–512 EQ-degree range;
                                                conversion: eq_deg * (4096/512) = 8x scale.
        10b   bitfield     animation            Current animation state ID.
        10b   bitfield     (padding/reserved)
Total: ~22 bytes (exact padding depends on bitfield alignment; 24 bytes with typical alignment)
```

**Bitfield packing note:** EQ uses packed bitfields for heading+animation in a single 32-bit
word. The server rejects heading values that imply instantaneous 180° turns (exceeds angular
velocity cap). See tolerance envelope below.

### Position Coordinates (Absolute vs. Delta)

The per-frame update sends **velocity deltas** (delta_x, delta_y, delta_z), not absolute
coordinates. The server maintains its own running position estimate by accumulating deltas
from the client. Absolute position is reconciled via the full `PlayerProfile_Struct` on zone
entry and via periodic full-position packets from the server side (`OP_MobUpdate` /
`SpawnPositionUpdate`).

The server's position predictor uses:

- Last known position (from PlayerProfile or prior agreement)
- Accumulated deltas from consecutive position updates
- Expected timing (15 ms cadence expected; late or missing updates create drift)

### `OP_MobUpdate` / `SpawnPositionUpdateServer_Struct` (S→C, for reference)

Not the movement-agreement direction, but relevant for understanding the server's coordinate
representation it compares against:

```
Field           Bits  Notes
x_pos           19    Signed 19-bit fixed-point. 1 unit ≈ 1 EQ unit.
y_pos           19    Signed 19-bit fixed-point.
z_pos           19    Signed 19-bit fixed-point.
heading         12    0–4095.
animation       10    Animation state.
delta_x/y/z     varies  Server-predicted velocity.
```

---

## Reference Packet Capture

No live packet capture for the movement-agreement packet is available in this codebase.
Known reference sources:

| Source                                                                        | Confidence | Notes                                                                                        |
| ----------------------------------------------------------------------------- | ---------- | -------------------------------------------------------------------------------------------- |
| EQEmu `eq_packet_structs.h` — `PlayerPositionUpdateClient_Struct`             | High       | Canonical struct; changes per EQ expansion but stable for Velious-era TLP                    |
| MQ2reachit hook on `UdpConnection::SendMessage`                               | Medium     | Confirms opcode `0x1643` is the C→S movement path; MQ2 reads raw bytes from hook             |
| `CMovementHistoryClientException` RTTI (Ghidra, binary `eqgame.exe` 20260310) | High       | Confirms server-side validation class exists; xref to serialization site pending             |
| Matt (SME) — movement fires every ~15 ms; path summaries 1/sec                | Medium     | Single-source; consistent with EQEmu net layer flush interval of 50 ms and 20 Hz update rate |
| `Research-EQ-Protocol.md` §4.1 (`PlayerPositionUpdateClient_Struct`)          | High       | Matches EQEmu struct; opcode listed as `0x14cb` (patch-specific — rotates per patch)         |

**Opcode rotation note:** Live Daybreak / TLP servers rotate all application-layer opcodes with
every patch. The `0x1643` opcode is from the network architecture research (binary circa Dec
2024). The `0x14cb` value in `Research-EQ-Protocol.md` §4.1 is from a different patch mapping.
The actual opcode for client build 20260310 requires pattern-scan or live capture confirmation.

---

## Tolerance Envelope

Derived from EQEmu source (`zone/client_process.cpp`), SME notes, and EQ protocol research.
All bounds are **research-backed, not live-validated** — treat as conservative starting targets
until live capture confirms them.

### Angular velocity (heading)

| Bound                              | Value                    | Derivation                                           |
| ---------------------------------- | ------------------------ | ---------------------------------------------------- |
| Max turn rate (run)                | ~180 EQ-degrees/sec      | EQEmu `MAX_TURN_RATE` — approximate; varies by speed |
| Per-frame cap (at 15 ms)           | ~2.7 EQ-degrees/frame    | 180 deg/sec \* 0.015s                                |
| `humanize.rs` heading_wobble range | 1.0–4.0 EQ-degrees/frame | `from_client_id` seed computation                    |

**Assessment:** `humanize.rs` wobble range of 1.0–4.0 EQ-degrees/frame is within the
~2.7 EQ-degrees/frame per-frame cap when the heading is wobbled in a single direction.
However, the 4.0 EQ-degree upper bound may generate a delta of up to 4.0 deg/frame, which
slightly exceeds the estimated server cap. This is a drift risk at maximum `heading_wobble`
values.

**Recommendation:** Cap `heading_wobble` at 2.5 EQ-degrees per call to `wobble_heading` to
stay comfortably under the estimated ~2.7 EQ-degree/frame limit. Apply the jitter as smooth
accumulated drift rather than random per-frame jumps to avoid instantaneous angular velocity
spikes.

### Velocity / speed

| Bound                            | Value                      | Derivation                                     |
| -------------------------------- | -------------------------- | ---------------------------------------------- |
| Normal run speed                 | ~1.5 units/frame (at 15ms) | EQ base run speed ~100 units/sec / 66 Hz ≈ 1.5 |
| Max allowed velocity magnitude   | ~3.0–4.0 units/frame       | EQEmu warp detection: ~2x normal run speed     |
| `humanize.rs` speed_factor range | 0.93–1.07                  | `from_client_id` seed computation              |

**Assessment:** `humanize.rs` speed_factor of 0.93–1.07 (±7% of run speed) is well within the
~2x warp detection threshold. No drift risk identified here.

### Position delta continuity

The server expects delta_x/y/z to be consistent with the time elapsed since the last update.
Injecting a large position delta (even a plausible velocity) without the correct timing
violates the implicit timestamp assumption.

| Bound                    | Value             | Derivation                                      |
| ------------------------ | ----------------- | ----------------------------------------------- |
| Expected update cadence  | ~15 ms (67 Hz)    | SME + EQEmu hold-buffer flush rate              |
| Path summary cadence     | ~1 sec            | SME; aggregated over ~67 frames                 |
| Jitter tolerance (EQEmu) | ±50 ms per update | EQEmu net layer allows this before resequencing |

**Current TextQuest behavior:** Navigation writes heading and speed fields directly via
in-process memory writes (`textquest-dll/src/hooks/movement.rs`). The EQ client's own network
layer then reads these fields and sends the position update packet at its normal cadence.
This means the timing and delta values are generated by EQ's own code, not injected — which
is the safest possible approach for movement agreement compliance.

### Summary: humanize.rs bounds vs. tolerance envelope

| Parameter                      | Current range        | Estimated server tolerance        | Status          |
| ------------------------------ | -------------------- | --------------------------------- | --------------- |
| `speed_factor`                 | 0.93–1.07            | up to ~2.0x (warp threshold)      | SAFE            |
| `heading_wobble`               | 1.0–4.0 EQ-deg/frame | ~2.7 EQ-deg/frame est.            | MARGINAL at max |
| `detour_chance`                | 0.00–0.08            | N/A (path-level, not frame-level) | SAFE            |
| `assist_jitter_ticks` (combat) | 0–5 ticks            | N/A (combat timing, not movement) | SAFE            |
| `cast_start_delay_ticks`       | 0–3 ticks            | N/A                               | SAFE            |

---

## Ghidra Xref Path (Pending)

The issue requests Ghidra xref from `CMovementHistoryClientException` back to the
serialization site. Steps to execute this (requires live Ghidra / GhidraMCP on Frostreaver):

1. Search for RTTI string `"CMovementHistoryClientException"` in the `.rdata` section.
2. XRef from the RTTI descriptor to the type info vtable entry.
3. XRef from the vtable entry to the class constructor / throw site.
4. From the throw site, walk up the call tree to find the serialization function (the one
   that reads position fields and calls `NetworkSend` / `UdpConnection::SendMessage`).
5. Capture: function address, local variables (struct fields read), opcode passed to
   `UdpConnection::SendMessage`, any threshold comparisons before the throw.

This is the **unresolved binary evidence** step. Until completed:

- The serialization function address is unknown.
- The exact opcode for the movement-history / movement-agreement packet (distinct from `0x1643`
  per-frame updates, if such a distinct packet exists) is unknown.
- The exact field-level comparisons the exception fires on are unknown.

**Blocked by:** Frostreaver Ghidra / GhidraMCP availability; requires Windows machine with
the 20260310 binary loaded in Ghidra.

---

## Follow-On Requirements

These items must be resolved before humanization bounds can be treated as live-validated:

- [ ] **Ghidra xref** — complete the `CMovementHistoryClientException` → serialization site
      trace; capture opcode and struct. File result in `docs/wiki/Research-Anti-Detection.md`
      §Ghidra-Verified Findings.
- [ ] **Live capture** — capture a live packet trace during normal navigation on Teek/Agnarr;
      confirm `0x1643` payload matches `PlayerPositionUpdateClient_Struct` layout above.
- [ ] **Angular velocity cap** — confirm the server-side turn-rate threshold from Ghidra or
      live capture; replace the ~2.7 EQ-deg/frame estimate with the exact value.
- [ ] **`heading_wobble` cap fix** — file follow-up issue if live data confirms the 4.0-deg
      upper bound is above tolerance (see §Tolerance Envelope recommendation above).
- [ ] **Opcode reconciliation** — reconcile `0x1643` (Dec 2024 binary), `0x14cb`
      (Research-EQ-Protocol mapping), and the actual 20260310-client opcode via pattern scan.

---

## Related Files

| File                                                      | Relevance                                                     |
| --------------------------------------------------------- | ------------------------------------------------------------- |
| `textquest-dll/src/nav/humanize.rs`                       | Movement humanization bounds — see tolerance assessment above |
| `textquest-dll/src/combat/humanize.rs`                    | Combat timing jitter — not movement-agreement relevant        |
| `textquest-dll/src/hooks/movement.rs`                     | In-process movement controller (safest path)                  |
| `docs/wiki/Research-Anti-Detection.md` §Open Questions Q4 | Movement agreement opcode status                              |
| `docs/wiki/Research-EQ-Protocol.md` §4.1                  | `PlayerPositionUpdateClient_Struct` reference                 |
| `docs/wiki/Research-EQ-Network-Architecture.md`           | `0x1643` opcode + `UdpConnection::SendMessage` path           |
| `docs/wiki/Research-Packet-Engine.md`                     | Layer 3 (`UdpConnection::SendMessage`) inventory              |

---

## Sources

1. EQEmu Server source — `common/eq_packet_structs.h` (`PlayerPositionUpdateClient_Struct`)
2. EQEmu Server source — `zone/client_process.cpp` (warp detection thresholds)
3. `Research-EQ-Network-Architecture.md` — opcode `0x1643`, `UdpConnection::SendMessage`
4. `Research-EQ-Protocol.md` §4.1 — struct layout, §4.3 — update frequency
5. `Research-Anti-Detection.md` §SME-Sourced TLP Q4 — 15 ms cadence, RTTI confirmation,
   CMovementHistoryClientException status
6. Matt (SME / Blownt) — 15 ms movement packet cadence, 1/sec path summaries, RTTI class
   existence (Medium confidence — single source)
