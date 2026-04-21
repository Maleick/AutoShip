# Packet Counter Watchdog

**Module**: `textquest-dll/src/hooks/packet_hook.rs`
**Scope**: M5.5 — Message counter 0xbb29 preservation (issue #2176)

## Purpose

EQ maintains global send/receive counters (`OUTBOUND_MSG_COUNTER = 0x140F60FC8`, `INBOUND_MSG_COUNTER = 0x140F60FC4`). Every 500 ms the client refills (+0x37 outbound / +0x55 inbound), negates both, and sends via opcode `0xbb29`. Server compares against its own send/receive counts. Any drift from our hook's packet injection or drops = detection.

## Design

**Audit (passive)**: All existing hook paths (`wsa_send_detour`, `wsa_recv_detour`, `handle_send`, `handle_recv`) classified as observational — EQ's opcode handler decrements the counter *before* calling `NET_SEND` → `WSASend`, so the hook fires post-decrement. No current paths inject or drop packets.

**Debug-build drift watchdog** (`#[cfg(debug_assertions)]`, Windows-only):
- Static counters `WD_OUTBOUND_OBSERVED` / `WD_INBOUND_OBSERVED` incremented per packet through the hook
- `maybe_watchdog_tick()` fires at most once per 500 ms (CAS-guarded); reads live counters via `rebase()` + `read_volatile`, compares delta to observed
- Threshold `WD_DRIFT_WARN_THRESHOLD = 60`; logs `debug` if within, `warn` if exceeded

## Operator Workflow

Zero overhead in release builds. Debug runs emit `warn!` on drift > 60 deltas per 500 ms window. Inbound refill (+0x55 = 85) intentionally exceeds threshold — helps distinguish expected refill ticks from genuine drift.

## Related

- Epic: #2173 (M5.5)
- Siblings: #2175 (server memcheck), #2177 (file integrity)
