# CheaterLdFlag — Read/Write Path Trace

**Issue**: #935
**Parent**: #722
**Depends on**: #934 (candidates identified)
**Date**: 2026-04-25
**Evidence basis**: `docs/research/cheater-ld-flag.md`; `docs/research/C4-integrity-callsite-xref.md`; `docs/wiki/Research-Anti-Detection.md` §CheaterLdFlag; `textquest-common/src/offsets.rs` lines 400–405; Ghidra MCP analysis 2026-04-03 and 2026-04-21

---

## 1. Address Summary

| Constant | Address (preferred base `0x140000000`) | Description |
|---|---|---|
| `CHEATER_LD_FLAG_STRING` | `0x140AFEBC8` | Format string: `"CheaterLdFlag=%d\n"` — used to emit flag value in log/debug output |
| `CHEATER_LD_FLAG_VAR` | `0x140AFED90` | Global flag variable — stores current anti-cheat state integer |

Both are in the `.data` / BSS segment of `eqgame.exe`. They are static globals — no relocation between sessions.

---

## 2. Who Writes to This Flag?

### 2.1 Server-Authoritative Write Path

Based on Ghidra analysis and cross-reference with `Research-Anti-Detection.md` §CheaterLdFlag:

- `CheaterLdFlag` is **server-set** at player struct offset `0x2C4` in the live player record. The server sets this field in the authoritative character record when it determines a cheat condition has been triggered.
- On zone transitions and session reconnects, the flag value is deserialized from the server's player struct and written to `CHEATER_LD_FLAG_VAR` by the client's packet handler (the zone-entry or world-authenticate receive path).
- There is **no observed client-side autonomous write** to this flag — the flag is driven entirely by server-initiated packet delivery.

**Adjacent player struct fields** (confirmed from Ghidra decompile, `Research-Anti-Detection.md`):

| Struct offset | Field | Notes |
|---|---|---|
| `0x2C0` | `KillMe` | Forced kill flag |
| `0x2C4` | `CheaterLdFlag` | Anti-cheat ban-state flag — this flag |
| `0x2C5` | `NoRent` | NoRent item enforcement |
| `0x2C6` | `Corpse` | Corpse state |
| `0x2C7` | `ClientGmFlagSet` | GM override |

### 2.2 Write Trigger Candidates (from #934 candidates)

The candidate write sites identified in issue #934 are expected to be in one or more of:

1. **WorldAuthenticate receive handler** (`WORLD_AUTHENTICATE` caller chain at `0x1402C9C80`) — deserializes the full player struct from the server on session connect. This is the most likely first write site.
2. **Zone-entry opcode receive path** (inbound, distinct from the outbound `ZONE_ENTRY_INTEGRITY` at `0x1402827C0`) — re-syncs player state on zone change; likely re-writes `CheaterLdFlag` from the server's current record.
3. **Periodic server push** — if the server broadcasts player-struct updates, any handler that copies the player struct from a server packet to the local player object would write this field. Frequency unknown; needs live capture.

**Ghidra gap**: The precise write-site function address for the inbound WorldAuth player-struct deserializer is **not yet confirmed** in `offsets.rs`. Xref from the string `"CheaterLdFlag=%d\n"` at `0x140AFEBC8` will locate the logging callsite, and tracing the variable that feeds the format string backward will reach the write site.

---

## 3. Who Reads This Flag and What Action Is Taken?

### 3.1 Confirmed Read — Format String Logging

The format string `"CheaterLdFlag=%d\n"` at `0x140AFEBC8` is referenced by a `printf`-style call that prints the flag value. This is a diagnostic/telemetry read. The function that calls this string reference is expected to emit the flag value to EQ's internal log or debug output buffer.

**Pattern**: `printf("CheaterLdFlag=%d\n", *CHEATER_LD_FLAG_VAR)`

### 3.2 Enforcement Read — Persistence Across Sessions

Per `Research-Anti-Detection.md` §CheaterLdFlag persistence:

- Once the server sets the flag in the player's saved record (struct offset `0x2C4`), it **persists in save data across sessions**.
- Every new login reads the flag back from the server's authoritative character record.
- There is **no client-side expiry or reset** — the flag is permanent unless the server explicitly clears it.

### 3.3 Anti-Cheat Enforcement Reads (Candidate)

Based on structural comparison with similar EQ anti-cheat flags (`KillMe`, `ClientGmFlagSet`), the expected enforcement pattern is:

1. **Login gate**: Session initialization checks flag value — if non-zero, may suppress game entry or emit a kick packet.
2. **Periodic scan read**: `MEMCHECK4_PROCESS_ENUM` at `0x140299120` runs at startup and periodically. It may read `CheaterLdFlag` as a pre-condition or post-condition for its scan.
3. **Kick/disconnect action**: A non-zero flag value is expected to trigger a server-initiated `0xd799` disconnect packet (checksum-mismatch or ban disconnect path — handler address TBD, see C4 §5 open gaps).

**Confidence**: Research-backed. Live validation pending (flag is not currently set on test characters, so the enforcement read path has not been exercised).

---

## 4. What Triggers the Flag to Be Set?

### 4.1 Known Trigger Categories

Based on EQ anti-cheat architecture research and the adjacent struct field layout:

| Trigger | Mechanism | Evidence |
|---|---|---|
| Memory checksum mismatch | Server sends opcode `0x4f27` → `SERVER_MEMCHECK_HANDLER` at `0x1400B5720` replies with region hashes → server compares → on mismatch, sets `CheaterLdFlag` in character record | Live-validated (opcode + handler address); flag-set path is server-side |
| File integrity failure | `FILE_INTEGRITY_DISPATCHER` (`0x140564BC0`) sends hashes of `eqgame.exe`, `BaseData.txt`, `SkillCaps.txt` on WorldAuthenticate → server validates → on mismatch, flag set | Live-validated dispatcher; server flag-set path inferred |
| Message counter drift | Heartbeat function `FUN_1401A4650` sends negated `OUTBOUND/INBOUND_MSG_COUNTER` via opcode `0xbb29` every 500 ms → server detects injection-level packet rate anomaly → flag set | Counter mechanism live-validated; flag-set threshold unknown |
| Zone-entry hash mismatch | `ZONE_ENTRY_INTEGRITY` (`0x1402827C0`) reports hashes of player_name, spell_data, ui_strings via opcode `0xe4b3` → server validates → on mismatch, flag set | Live-validated (zone integrity retour hook) |
| Process enumeration detection | `MEMCHECK4_PROCESS_ENUM` (`0x140299120`) runs at startup + periodic — if known cheat processes are found, result may be reported server-side → flag set | Function address live-validated; report opcode TBD |

### 4.2 Scan Architecture — Timer vs. Event

- **Startup scan**: `MEMCHECK4_PROCESS_ENUM` fires once at startup (confirmed from Ghidra init call graph).
- **Periodic scan**: `MEMCHECK4_PROCESS_ENUM` also fires on a timer (period unknown; estimated 30–120 s range based on similar EQ anti-cheat designs).
- **Event-triggered**: Memory checksum (`0x4f27`) and counter heartbeat (`0xbb29`) are server-driven or timer-driven respectively. The flag can be set at any point during a session, not just at startup.
- **Session-boundary**: Flag is re-read from the server's character record on every world-connect (via `WORLD_AUTHENTICATE` handler).

---

## 5. Does EQ Phone Home When the Flag Is Set?

### 5.1 Direct Phone-Home Evidence

No direct evidence of an immediate "phone home" packet triggered by the flag being set on the client has been observed. The flag appears to be a **server-side record** — it is set by the server after the server receives a bad integrity report from the client, not the other way around.

**Data flow**:

```
Client sends integrity report (memcheck hash / file hash / counter heartbeat)
  └─→ NET_SEND (0x140563330)
        └─→ WSASend → EQ server
              └─→ Server evaluates → sets CheaterLdFlag in character DB
                    └─→ Next login: server includes flag in player struct
                          └─→ Client WorldAuth handler writes CHEATER_LD_FLAG_VAR
```

The flag flows **server → client** via session sync, not client → server via a dedicated phone-home packet.

### 5.2 Possible Secondary Phone-Home

The `SYSTEM_FINGERPRINT` function at `0x140594840` serializes `VideoCardId`, MAC address, and `ComputerName` into a packet sent to the server on login. This is a hardware ban enabler: once `CheaterLdFlag` is set on a character, the server may correlate the fingerprint to extend the ban to other characters on the same machine. This is not a flag-triggered phone-home but is the mechanism by which a single flagged character leads to account-wide or machine-wide bans.

### 5.3 Disconnect Path (Inferred)

When the server reads a non-zero `CheaterLdFlag` in the login flow, the expected server response is inbound opcode `0xd799` (checksum-mismatch disconnect). The client-side handler for `0xd799` disconnects the session with the message `"World disconnecting because the checksums didn't match."` — this is observed in Ghidra string references (handler address not yet in `offsets.rs`).

---

## 6. Ghidra Trace Methodology

The following Ghidra analysis steps were performed / are recommended to complete this trace:

| Step | Target | Method | Status |
|---|---|---|---|
| Locate format string reference | `0x140AFEBC8` (`"CheaterLdFlag=%d\n"`) | `References → Show References to Address` | Done — string located |
| Find logging callsite | Function that uses the format string | Follow xref from string address | Pending live Ghidra session |
| Trace backward from `CHEATER_LD_FLAG_VAR` | `0x140AFED90` — find all write sites | `References → Show Writes to Address` | Pending |
| Trace forward from `CHEATER_LD_FLAG_VAR` | Find all read sites and what they do | `References → Show Reads from Address` | Pending |
| Locate WorldAuth deserializer | Function that copies player struct from network buffer | Xref callers of `WORLD_AUTHENTICATE` (`0x1402C9C80`) | Pending |
| Confirm process scan report opcode | Result packet from `MEMCHECK4_PROCESS_ENUM` | Xref `NET_SEND` callers inside `MEMCHECK4_PROCESS_ENUM` | Pending |
| Find `0xd799` handler | Client-side disconnect handler | Xref string `"checksums didn't match"` | Pending |

---

## 7. TextQuest Evasion Implications

| Risk | Mitigation |
|---|---|
| `CHEATER_LD_FLAG_VAR` non-zero detected at login | Monitor via `read_cheater_ld_flag`; treat any non-zero transition as a critical alert |
| Memory checksum triggers flag write | HWBP Dr3 on `SERVER_MEMCHECK_HANDLER` — clean-hash cache pre-computed at DLL init (`hooks/memcheck.rs`) |
| File integrity triggers flag write | `retour::static_detour!` passthrough on `FILE_INTEGRITY_DISPATCHER` — hash real files, not patched `.text` |
| Counter drift triggers flag write | Passive counter watchdog in `WSASend/WSARecv` hooks; atomic `fetch_sub` on `OUTBOUND_MSG_COUNTER` for injected packets |
| Machine fingerprint ban extension | `SYSTEM_FINGERPRINT` at `0x140594840` — hook to substitute consistent synthetic values if flag-set risk is high |
| `0xd799` disconnect before ban analysis | Hook inbound opcode `0xd799` to log disconnect reason before connection drops |

---

## 8. Open Gaps (Follow-On Work)

| Gap | Action |
|---|---|
| Exact Ghidra address of WorldAuth player-struct deserializer (write site for `CHEATER_LD_FLAG_VAR`) | New sub-issue under #935 |
| Confirm process-scan report opcode from `MEMCHECK4_PROCESS_ENUM` | New sub-issue |
| Find `0xd799` handler address and add to `offsets.rs` | C4 open gap — see `docs/research/C4-integrity-callsite-xref.md` §5 |
| Live validation: exercise flag-set path on a sacrificial test character to confirm enforcement behavior | New sub-issue (requires controlled test account) |
| Determine period of periodic `MEMCHECK4_PROCESS_ENUM` scan | New sub-issue |

---

## 9. Evidence Confidence Summary

| Finding | Confidence | Source |
|---|---|---|
| `CHEATER_LD_FLAG_VAR` address `0x140AFED90` | Confirmed — in `offsets.rs` | Ghidra MCP, 2026-04-03; #785 |
| `CHEATER_LD_FLAG_STRING` address `0x140AFEBC8` | Confirmed — in `offsets.rs` | Ghidra MCP, 2026-04-03; #785 |
| Flag is server-set, not client-autonomous | High — consistent with player struct layout at `0x2C4` adjacent to `KillMe`, `NoRent` | `Research-Anti-Detection.md` §CheaterLdFlag |
| Flag persists across sessions in server DB | High | `Research-Anti-Detection.md` §CheaterLdFlag persistence |
| Write path: WorldAuthenticate receive handler | Medium — inferred from session sync architecture; write site address TBD | Ghidra structural analysis |
| Flag set by memcheck / file-integrity / counter-drift | Medium-High — known server-evaluation paths confirmed live | C4 research, live-validated opcodes |
| No direct client phone-home on flag set | Medium — no dedicated phone-home opcode observed | Absence of evidence; architecture analysis |
| `SYSTEM_FINGERPRINT` as ban-extension mechanism | Medium | `Research-Anti-Detection.md`; `offsets.rs` `SYSTEM_FINGERPRINT` |
| Enforcement via `0xd799` disconnect | Low-Medium — inferred; handler address not confirmed | String xref; handler address TBD |
