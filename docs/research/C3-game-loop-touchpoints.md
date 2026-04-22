# C3 — Main Game Loop: Byte-Count and Counter Touchpoints

**Issue:** #2186  
**Parent:** #2174  
**Date:** 2026-04-21  
**Evidence state:** SME-reported (Matt/Blownt) + Ghidra-verified partial (2026-04-03). Main loop body full decompile blocked by ~20 KB function body timeout. See §Evidence Gaps.

---

## Summary

The EQ main game loop (`__ProcessGameEvents`, preferred base `0x14028E0F0`) is a ~20 KB function body that the decompiler timed out on. This document consolidates all known touchpoints from SME input, Ghidra partial analysis, and inference from existing code — organized by region so that future Ghidra work can target individual sub-regions rather than the full body at once.

The "inline byte count checks" originally reported by the SME were subsequently identified via Ghidra as a **message counter heartbeat system**, not static byte comparisons. The two systems are documented separately below.

---

## 1. Key Addresses

### 1.1 Main Loop Entry Point

| Symbol                | Preferred Address                | Source                                           | Confidence        |
| --------------------- | -------------------------------- | ------------------------------------------------ | ----------------- |
| `__ProcessGameEvents` | `0x14028E0F0`                    | `textquest_common::offsets::PROCESS_GAME_EVENTS` | High (repo-coded) |
| `MAIN_LOOP_OFFSET`    | `0x0028_E0F0` (from module base) | `textquest-dll/src/eq/mod.rs:16`                 | High (repo-coded) |

TextQuest hooks this address via HWBP on DR0. No code bytes are modified at the entry point. The HWBP fires before the first instruction of `__ProcessGameEvents`, runs `on_game_tick()`, then resumes with RF set.

### 1.2 Message Counter Globals

| Symbol                 | Preferred Address  | Type         | Direction        | Source                                            |
| ---------------------- | ------------------ | ------------ | ---------------- | ------------------------------------------------- |
| `OUTBOUND_MSG_COUNTER` | `0x0001_40F6_0FC8` | `i32` global | Outbound packets | `textquest_common::offsets::OUTBOUND_MSG_COUNTER` |
| `INBOUND_MSG_COUNTER`  | `0x0001_40F6_0FC4` | `i32` global | Inbound packets  | `textquest_common::offsets::INBOUND_MSG_COUNTER`  |

**Note on address discrepancy:** The Ghidra analysis session (2026-04-03) named these `DAT_140f60ed8` (outbound) and `DAT_140f60ed4` (inbound) in the wiki text. The offsets.rs constants differ by 0xF0 bytes (`FC8`/`FC4` vs `ed8`/`ed4`). The offsets.rs values are the authoritative repo source and were added after the wiki was written. **Live validation is required** to confirm which pair is correct. See §Evidence Gaps.

### 1.3 Related Anti-Cheat Functions

| Ghidra Name                  | Preferred Address  | Role                                                                                                                                                          | Source                                                   |
| ---------------------------- | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| `FUN_1401a4320`              | `0x1401A4320`      | Counter heartbeat sender (sends `0xbb29`) — named in Open Questions §3                                                                                        | Research-Anti-Detection.md §Open Questions               |
| `FUN_1401a4650`              | `0x1401A4650`      | Named in Ghidra-Verified section as heartbeat system; same address as `CDisplay::RealRender_World` per offsets.rs — **naming collision, requires resolution** | Research-Anti-Detection.md §Ghidra-Verified + offsets.rs |
| `NET_SEND` (`FUN_140563330`) | `0x0001_4056_3330` | Main packet send (247 bytes, 29+ callers)                                                                                                                     | `textquest_common::offsets::NET_SEND`                    |
| `FILE_INTEGRITY_DISPATCHER`  | `0x0001_4056_4BC0` | EXE self-hash + data file integrity checks                                                                                                                    | `textquest_common::offsets::FILE_INTEGRITY_DISPATCHER`   |
| `SERVER_MEMCHECK_HANDLER`    | `0x0001_400B_5720` | Handles server-initiated memcheck (opcode `0x4f27`)                                                                                                           | `textquest_common::offsets::SERVER_MEMCHECK_HANDLER`     |

---

## 2. Inline Byte-Count Checks (Recharacterized)

### 2.1 What the SME Reported

Original SME claim: the main loop contains "inline byte count checks" that detect JMP patches by comparing section sizes. These were described as continuous and woven into the loop body, providing circular protection with memshift detection.

### 2.2 What Ghidra Resolved

After Ghidra analysis (2026-04-03), the "inline byte count checks" were recharacterized as the **message counter heartbeat system**. The counters track packet activity (not code section byte sizes) and report to the server. The original "byte count" framing was imprecise — the counters count network messages, not code bytes.

**This resolves the confusion but does not eliminate the constraint.** If EQ does perform actual code-section integrity checks (static byte comparisons against a baseline), those were not found in the partial Ghidra analysis. They may exist in cold subroutines called from the loop body that were not analyzed due to the timeout. This remains an **open question** requiring the chunked decompile described in §6.

### 2.3 Standard Detour Hook Risk

Standard detour hooking (JMP patch at function prologue) alters code bytes. Even if the inline "byte count" checks turn out to be network message counters rather than code checksums, the **server-initiated memcheck** (`opcode 0x4f27`) can request a hash of any address range in `.text`, including the patched prologues. Standard detours remain high-risk regardless of how the inline checks are classified.

---

## 3. Message Counter Heartbeat System

### 3.1 Mechanism

EQ maintains two global `i32` counters. Every opcode handler in the main loop decrements the appropriate counter immediately after calling `NET_SEND` (`FUN_140563330`):

- **Outbound counter** (`OUTBOUND_MSG_COUNTER`) — decremented on every outbound packet send.
- **Inbound counter** (`INBOUND_MSG_COUNTER`) — decremented on every inbound WorldAuth message processed.

Every ~500 ms, the heartbeat function:

1. Checks if either counter is below 2.
2. Refills: outbound += `0x37` (55 decimal), inbound += `0x55` (85 decimal).
3. Negates both values.
4. Sends the negated pair to the server via opcode `0xbb29`.

The server independently tracks send/receive counts and compares against the client-reported values. Any mismatch is treated as a detection event.

### 3.2 Known Counter Decrement Sites

The SME reported **40+ decrement sites** in the main loop body — one per opcode handler. A complete list cannot be produced without the full decompile (blocked by timeout). Known categories from partial analysis:

| Category                                            | How Counter Changes                                         | Notes                                            |
| --------------------------------------------------- | ----------------------------------------------------------- | ------------------------------------------------ |
| Outbound opcode handlers                            | `OUTBOUND_MSG_COUNTER--` after each `NET_SEND` call         | 40+ handler sites in loop body per SME           |
| Inbound WorldAuth messages                          | `INBOUND_MSG_COUNTER--` per message processed               | Exact handler count unknown                      |
| Heartbeat tick (`0xbb29` send)                      | Outbound counter decrements for the heartbeat packet itself | Counted as a legitimate outbound send            |
| File integrity sends (`0x8bdc`, `0xe91d`, `0x9562`) | Outbound counter decremented                                | Triggered on zone/world connect, not every frame |

### 3.3 TextQuest Watchdog Implementation

TextQuest implements a debug-mode counter drift watchdog in `textquest-dll/src/hooks/packet_hook.rs`:

- Polls at 500 ms intervals (matching EQ's heartbeat cadence).
- Reads `OUTBOUND_MSG_COUNTER` and `INBOUND_MSG_COUNTER` via rebased absolute address reads.
- Tracks locally-observed packet counts via atomics `WD_OUTBOUND_OBSERVED` / `WD_INBOUND_OBSERVED`.
- Warns if drift magnitude exceeds `WD_DRIFT_WARN_THRESHOLD = 60`.
  - Outbound refill (+55) is within threshold — no false positive.
  - Inbound refill (+85) exceeds threshold — a refill event will fire a warning. This is expected; operators distinguish refill events from genuine drift by observing the counter rising back to the refill floor.

The watchdog is **purely observational** and never writes to counter memory.

### 3.4 Hook Safety for WSASend/WSARecv

Current TextQuest path (Winsock-level hooking):

- **WSASend detour:** fires inside WSASend, _after_ EQ's opcode handler has already decremented `OUTBOUND_MSG_COUNTER`. No counter adjustment needed.
- **WSARecv detour:** calls original first, then reads buffer. No counter involvement.

Future injection paths that synthesize packets by calling WSASend directly (bypassing EQ's opcode handler) **must** perform an atomic `fetch_sub(1, SeqCst)` on the rebased `OUTBOUND_MSG_COUNTER` address. See `packet_hook.rs` §Counter audit comment for the canonical requirement.

---

## 4. Memshift Detection

### 4.1 What the SME Reported

Memshift detection was reported as occurring every ~3 minutes, inline in the main game loop body, checking whether main loop memory has been modified since baseline.

### 4.2 Current Status

Not independently verified by Ghidra analysis (decompile timed out before reaching that code region). The SME description remains `SME-reported` confidence. It may be:

- A check on the loop body's own code bytes (static integrity of the loop itself).
- A broader `.text` section CRC.
- An alternate invocation of the server-initiated memcheck mechanism.

**This is the primary remaining unknown** driving the no-touch zone classification for the main loop body.

---

## 5. No-Touch Zone Assessment (Per-Region)

| Region                                                  | Classification                         | Rationale                                                                                                                                                                                              |
| ------------------------------------------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `__ProcessGameEvents` entry point (first instruction)   | **SAFE for HWBP**                      | TextQuest already hooks here via DR0. No code bytes modified. Server-initiated memcheck would see clean bytes. Zero inline code modification.                                                          |
| `__ProcessGameEvents` body — opcode handler sub-regions | **NO-TOUCH (code modification)**       | 40+ counter decrement sites; standard JMP patches alter bytes detectable by server memcheck. HWBP on any interior address is technically possible but risks HWBP slot exhaustion (only DR0-DR3).       |
| `__ProcessGameEvents` body — memshift check sub-region  | **NO-TOUCH (unknown until decompile)** | Location and mechanism of memshift check unknown. If it checks its own code bytes, any modification in the loop body would trigger it. Cannot confirm safe sub-regions without chunked decompile (§6). |
| `NET_SEND` (`0x140563330`)                              | **NO-TOUCH for standard detour**       | Server memcheck can hash this address. 247 bytes, 29+ callers. HWBP is safe (zero code modification).                                                                                                  |
| `OUTBOUND_MSG_COUNTER` / `INBOUND_MSG_COUNTER` globals  | **READ-ONLY**                          | TextQuest reads via volatile pointer in watchdog. No writes except to simulate counter injection — which is prohibited unless an outbound packet is being synthesized.                                 |
| Counter heartbeat function (`FUN_1401a4320`)            | **UNKNOWN**                            | Address not yet confirmed as a distinct function vs `CDisplay::RealRender_World`. Requires address resolution.                                                                                         |
| Server memcheck handler (`0x140B5720`)                  | **DO NOT HOOK**                        | Hooking would make memcheck return falsified hashes, creating a detectable anomaly visible to the server.                                                                                              |

---

## 6. Chunked Decompile Plan

Full decompile of the ~20 KB `__ProcessGameEvents` function body is blocked by Ghidra timeout. The recommended approach:

1. **Split into chunks via Ghidra function splitting.** The main loop body should be analyzed in 2–4 KB segments by creating artificial function boundaries at identified sub-regions.
2. **Priority regions for decompile:**
   - Memshift check sub-region (~3-minute cadence trigger). Look for timer comparisons followed by CRC/hash computations on the code section.
   - Counter decrement dense region — enumerate all `dec [rip+offset]` or `lock xadd` instructions referencing `DAT_140f60xxx` addresses to get the full 40+ site list with offsets.
   - Loop entry / loop exit — identify whether there are sub-function calls at entry or exit that are actually in cold subroutines (would make them individually safe to hook).
3. **Expected output:** a table of counter decrement sites (RVA offsets relative to `__ProcessGameEvents` start), memshift check location, and any cold subroutine call targets that fall outside the loop body.

This work requires live Ghidra access on Frostreaver (see `reference_ghidra_mcp.md` in project memory for API endpoints). It cannot be completed from Mac without remote GhidraMCP.

---

## 7. Address Discrepancies Requiring Resolution

The following discrepancies must be resolved via live Ghidra re-analysis before this research can be promoted to `Live-validated`:

| Item                     | Value A                               | Value B                                   | Resolution needed                                                                                                                |
| ------------------------ | ------------------------------------- | ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Outbound counter address | `DAT_140f60ed8` (wiki Open Questions) | `0x140F60FC8` (offsets.rs)                | Live Ghidra read at both addresses; confirm which is the actual decrement target                                                 |
| Inbound counter address  | `DAT_140f60ed4` (wiki Open Questions) | `0x140F60FC4` (offsets.rs)                | Same                                                                                                                             |
| Heartbeat function       | `FUN_1401a4320` (Open Questions §3)   | `FUN_1401a4650` (Ghidra-Verified section) | Confirm which address is the `0xbb29` sender; note `0x1401a4650` = `REAL_RENDER_WORLD` in offsets.rs — possible naming collision |

---

## 8. Implications for Hook Work

### Safe hook sites (confirmed)

- `__ProcessGameEvents` entry via HWBP (DR0). Already in production.
- `LoginController::GiveTime` via HWBP (DR1 in eqmain mode). In production.
- `dsp_chat` via HWBP (DR1 in-game). In production.

### Unsafe until memshift is resolved

- Any hook inside the `__ProcessGameEvents` body — memshift check location unknown.
- Any modification to `NET_SEND` prologue — server memcheck detectable.
- Any hook of the heartbeat function — would break counter reporting.

### Safe with counter discipline

- Injecting packets via WSASend directly: safe only with matching `OUTBOUND_MSG_COUNTER` decrement. Currently prohibited by project policy (no injection paths exist).
- Dropping inbound packets: would cause `INBOUND_MSG_COUNTER` to drift relative to server-observed inbound count. Prohibited.

---

## 9. Feeds

This research directly feeds:

- **#2176 (A2 — counter preservation):** Counter semantics confirmed. MUST-decrement rule documented in `packet_hook.rs`. Watchdog implementation complete in debug builds.
- **Future main-loop-adjacent hook work:** Entry point confirmed safe for HWBP. Interior regions blocked until chunked decompile (§6) is completed on Frostreaver.

---

## References

- `textquest-common/src/offsets.rs` — `OUTBOUND_MSG_COUNTER`, `INBOUND_MSG_COUNTER`, `NET_SEND`, `PROCESS_GAME_EVENTS`
- `textquest-dll/src/eq/mod.rs:16` — `MAIN_LOOP_OFFSET`
- `textquest-dll/src/hooks/game_loop.rs` — hook installation, `on_game_tick()`, HWBP slot Dr0
- `textquest-dll/src/hooks/packet_hook.rs` — counter audit comment, drift watchdog, `WD_DRIFT_WARN_THRESHOLD`
- `docs/wiki/Research-Anti-Detection.md` — §SME-Sourced TLP Anti-Cheat Intel, §Ghidra-Verified Findings, §Evidence Status
- `docs/research/hook-detection-surface.md` — hook method classification table
