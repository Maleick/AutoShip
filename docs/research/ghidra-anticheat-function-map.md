# Ghidra Anti-Cheat Function Map

**Issue**: #722 (parent); sub-issues #934, #935, #936
**Binary**: `eqgame.exe` (Live TLP build, base `0x140000000`)
**Analysis tool**: GhidraMCP on Frostreaver (2026-04-03)
**Evidence state**: `Live-validated` where noted; `Research-backed` elsewhere
**Date**: 2026-04-25

---

## 1. Anti-Cheat Candidate Functions via String References (#934)

String references in `.rdata` are the primary discovery surface for anti-cheat
code. The following functions were identified by tracing XREFs from known
anti-cheat strings.

### 1.1 Primary Entry Point — Hook Scanner

| Label | Address | Discovery Method |
|---|---|---|
| `MainGameLoop_AntiCheatValidation` | `0x140270D00` | String ref scan + game loop XREF |

This function runs periodically inside the main game loop at a randomized
interval: `(iVar29 % 0x51 + 0x53)` milliseconds. It scans the prologues of
imported and internal functions for JMP patches.

**Detected byte patterns:**

| Pattern | Bytes | Description |
|---|---|---|
| Direct JMP | `E9 xx xx xx xx` | 5-byte relative jump (most common detour) |
| Indirect JMP (abs) | `FF 25 xx xx xx xx` | 6-byte absolute indirect jump |
| Indirect JMP (SIB) | `FF 24 ...` | SIB-based indirect jump variant |

**Reporting opcode:** `0xfbb` — carries spawn ID, detection type `0x2e`, and a
function-specific report code. Report is state-change gated: only fires when the
hook state changes, not on every check.

### 1.2 Functions Monitored by the Hook Scanner

#### Kernel32.dll imports scanned

| Function | Report Code | Purpose |
|---|---|---|
| `GetComputerNameW` | `0xb24` | Machine identity (fingerprint input) |
| `OpenProcess` | `0x7253` | Injection detection surface |
| `K32EnumProcesses` | `0x20d8` | Process enumeration |
| `K32EnumProcessModules` | `0x45d5` | Module enumeration per process |
| `GetModuleFileNameA` | `0x227c` | Module path recovery |
| `GetCurrentProcess` | `0x390` | Current process handle |
| `QueryFullProcessImageNameA` | `0xce0` | Full process image path |

#### Iphlpapi.dll imports scanned

| Function | Report Code | Purpose |
|---|---|---|
| `GetAdaptersAddresses` | `0x2b65` | NIC enumeration (fingerprint input) |

#### Internal EQ functions scanned (address → report code)

| Address | Report Code | Inferred Purpose |
|---|---|---|
| `0x140549A00` | `0x7e60` | Unknown — pending Ghidra decompile |
| `0x1405843C0` | `0x1e0d` | Unknown — pending Ghidra decompile |
| `0x140584610` | `0x16d7` | Unknown — pending Ghidra decompile |
| `0x140582690` | `0x1939` | Unknown — pending Ghidra decompile |
| `0x140584450` | `0x2f3d` | Unknown — pending Ghidra decompile |
| `0x140290F60` | `0x7b44` | Likely memcheck / process scanner |
| `0x1402910D0` | `0x1d3` | Likely memcheck branch |
| `0x140292C50` | `0x214` | Likely memcheck branch |
| `0x140269290` | `0x2233` | Game loop integrity caller |
| `0x1406B3460` | `0x921` | Network / packet function |
| `0x1406B3330` | `0x40a6` | Network / packet function |
| `0x1406B35C0` | `0x4e98` | Network / packet function |
| `0x1406B28C0` | `0x5aee` | Network / packet function |
| `0x1406B3040` | `0x5858` | Network / packet function |
| `0x1406B2D70` | `0x6a0b` | Network / packet function |
| `0x1406B2E70` | `0x2cf5` | Network / packet function |

> **Open gap**: Internal function addresses at `0x14054xxxx` and `0x14058xxxx`
> have not been decompiled. Ghidra sweep needed to confirm whether these are
> additional fingerprint serializers or packet pipeline functions. Tracked in
> C4 §5 open gaps.

### 1.3 Other Anti-Cheat Entry Points (String-Referenced)

| Label | Address | String anchor | Notes |
|---|---|---|---|
| `MEMCHECK4_PROCESS_ENUM` | `0x140299120` | Process enumeration strings (MQ2, WinEQ) | Runs at startup + periodic; enumerates all processes |
| `SYSTEM_FINGERPRINT` | `0x140594840` | `/afp -get/-send/-getworld/-testvm` | Serializes VideoCardId, NetworkCardId, HardriveId, ComputerName, NetworkAdapters, HardDrives |
| `FILE_INTEGRITY_DISPATCHER` | `0x140564BC0` | File hash verification strings | Dispatches three file-hash checks on WorldAuthenticate; uses LFG PRNG for region sampling |
| `SERVER_MEMCHECK_HANDLER` | `0x1400B5720` | Opcode `0x4f27` handler | Returns region hashes to server on demand |
| `ZONE_ENTRY_INTEGRITY` | `0x1402827C0` | Opcode `0xe4b3` string | Zone-connect integrity reporter |

---

## 2. CheaterLdFlag — Read/Write Path and Trigger Conditions (#935)

### 2.1 Addresses

| Constant | Address | Type |
|---|---|---|
| `CHEATER_LD_FLAG_STRING` | `0x140AFEBC8` | Format string: `"CheaterLdFlag=%d\n"` |
| `CHEATER_LD_FLAG_VAR` | `0x140AFED90` | Global flag variable (integer) |

Both constants are registered in `textquest-common/src/offsets.rs` and are
covered by the non-overlap test `cheater_ld_flag_offsets_do_not_overlap`.

### 2.2 Interpretation

`CheaterLdFlag` is a global integer in `eqgame.exe` that persists ban or
anti-cheat state across sessions. Based on the format string and variable
location:

- **Zero**: clean state — no active cheat signal
- **Non-zero**: suspicious or flagged state — behavior implications unknown
  without full call-graph trace

The presence of a dedicated printf-style format string (`"CheaterLdFlag=%d\n"`)
indicates this value is logged or reported, likely to the server via a packet
opcode (candidate: `0xfbb` or a dedicated state-change opcode).

### 2.3 Write Path (Partial — Ghidra Sweep Pending)

Confirmed write sites are not yet fully enumerated. Known information:

- The flag is written by at least one function in the anti-cheat validation
  path (suspected callsite within or adjacent to `MainGameLoop_AntiCheatValidation`)
- The `CheaterLdFlag` report appears to be state-cached: value changes trigger
  a report, repeated same-value writes do not re-fire
- State can persist across zone changes (global variable, not stack-local)

> **Open gap**: Full write-XREF sweep needed in Ghidra. Search all MOV/CALL
> XREFs to `0x140AFED90`. Tracked as open item for #935.

### 2.4 Trigger Conditions (Research-Backed)

Based on reverse-engineering of the scanner loop and SME community research:

| Trigger | Confidence | Notes |
|---|---|---|
| JMP-patched function detected by hook scanner | High | Confirmed via decompile of opcode `0xfbb` path |
| `MEMCHECK4_PROCESS_ENUM` finds MQ2/WinEQ process | High | Confirmed via process scan implementation |
| Server memcheck hash mismatch (opcode `0x4f27`) | High | Confirmed via `SERVER_MEMCHECK_HANDLER` decompile |
| File integrity mismatch (WorldAuthenticate) | High | Confirmed via `FILE_INTEGRITY_DISPATCHER` |
| A/B packet counter drift | Medium | Counter mismatch triggers opcode `0xd799` disconnect |
| Hardware fingerprint correlation (duplicate IDs) | Medium | 36 clients with same fingerprint = instant flag |
| Non-zero `CheaterLdFlag` at session start | Unknown | Persistence behavior not fully confirmed |

### 2.5 TextQuest Evasion Status

| Trigger | Evasion Implemented | Mechanism |
|---|---|---|
| Hook scanner (JMP detection) | Yes | HWBP (DR0-DR3) — zero code bytes modified |
| Process enumeration | Partial | PEB module unlink; PE header erasure |
| Server memcheck | Yes | `memcheck.rs` pre-hashes `.text` blocks; HWBP on Dr3 |
| File integrity | Yes | `FILE_INTEGRITY_DISPATCHER` hook implemented |
| Hardware fingerprint | Partial | Per-client spoofing required (#343) |
| Packet counter | Yes | Counter watchdog implemented |

---

## 3. EQ Scan Regions (#936)

### 3.1 Module List Scan

EQ enumerates running processes and loaded modules via:

- `K32EnumProcesses` — lists all running PIDs
- `K32EnumProcessModules` — lists modules for each process
- `GetModuleFileNameA` — resolves module path for name matching

**What it checks**: process names cross-referenced against a known-cheats list
(MQ2, WinEQ, and related tools). Runs at startup and periodically.

**Address**: `MEMCHECK4_PROCESS_ENUM` at `0x140299120`

**Evasion**: TextQuest DLL is injected with PEB module unlinking and PE header
erasure, making it invisible to this enumeration. Confirmed technique:
reflective/manual mapping avoids appearing in `K32EnumProcessModules` output.

### 3.2 IAT Scan (Hook Detection)

EQ reads back the **first bytes** of each monitored import's function prologue
(not the IAT pointer itself). This catches JMP-style detour hooks installed at
the function entry point.

**What it does NOT check**: IAT pointer integrity — confirmed by Ghidra analysis
that EQ does not compare IAT entries to known export addresses. IAT patching
(replacing the pointer in the IAT) is therefore a lower-exposure hook method
than `.text` JMP detours.

**Confirmed**: `VirtualProtect` is NOT imported by EQ — it cannot directly
observe the `VirtualProtect` call used to make the IAT writable. (ETW
kernel-mode tracing could still observe it externally.)

**HWBP status**: `GetThreadContext` / `SetThreadContext` are NOT imported by EQ.
HWBP hooks installed via debug registers are invisible to the client's hook
scanner.

### 3.3 Heap / Data Region Scan

EQ's server memcheck scans specific memory regions on demand:

- **Trigger**: Server sends opcode `0x4f27` with an address range
- **Handler**: `SERVER_MEMCHECK_HANDLER` at `0x1400B5720`
- **Mechanism**: Client hashes the requested region and returns the result
- **PRNG**: LFG (Lagged Fibonacci Generator) with p=55, q=24, 256 samples
  used by `FILE_INTEGRITY_DISPATCHER` to select which 256-byte region to hash

**Confirmed scan targets** (from decompile):
- `.text` code sections (primary target — JMP patches change bytes)
- File integrity regions (WorldAuthenticate path)

**Confirmed NOT scanned** (from decompile + IAT/VTable analysis):
- `.rdata` (IAT, VTable pages) — outside server memcheck hash range
- Heap allocations — no heap walk confirmed in current scan surface

**Evasion**: `memcheck.rs` pre-hashes `.text` blocks before any modification.
HWBP on Dr3 intercepts the server's hash-check call and returns the pre-computed
clean hash.

### 3.4 HWBP / Debug Register Scan

**Anti-debug imports confirmed present**:
- `IsDebuggerPresent` — trivially bypassed via PEB `BeingDebugged` flag

**Anti-debug imports confirmed ABSENT** (Ghidra-verified):
- `GetThreadContext` — not imported; HWBP values unreadable by EQ
- `SetThreadContext` — not imported
- `NtQueryInformationProcess` — not imported
- `CheckRemoteDebuggerPresent` — not imported

**Conclusion**: EQ cannot enumerate or detect HWBP-based hooks installed in
DR0-DR3. This is the foundation of TextQuest's hook strategy.

### 3.5 Scan Summary Table

| Scan Type | EQ Mechanism | Address | TextQuest Evasion | Status |
|---|---|---|---|---|
| Process / module enumeration | `K32EnumProcesses` + `K32EnumProcessModules` | `0x140299120` | PEB unlink + manual map | Partial (#343) |
| IAT pointer integrity | Not performed | N/A | N/A (not a threat) | Confirmed safe |
| `.text` prologue hook scan | Read first bytes at monitored function | `0x140270D00` | HWBP (zero bytes modified) | Implemented |
| Server `.text` memcheck | Hash region on opcode `0x4f27` | `0x1400B5720` | Pre-hash + intercept on Dr3 | Implemented |
| File integrity (WorldAuth) | 3-subopcode hash dispatch | `0x140564BC0` | Hook implemented | Implemented |
| Debug register enumeration | Not performed (`GetThreadContext` absent) | N/A | N/A (not a threat) | Confirmed safe |
| Heap walk | Not confirmed in current analysis | Unknown | Unknown | Open gap |
| ETW telemetry | No ETW APIs imported | N/A | N/A | Confirmed safe |

---

## 4. Open Gaps

The following items require additional Ghidra sweeps to close. Each maps to a
sub-issue.

| Gap | Sub-issue | Priority |
|---|---|---|
| Full write-XREF sweep on `CHEATER_LD_FLAG_VAR` (`0x140AFED90`) | #935 | High |
| Decompile internal functions `0x14054xxxx`/`0x14058xxxx` to confirm purpose | #934 | Medium |
| Confirm inbound opcode dispatcher address (C4 §5) | C4 open | High |
| Confirm `0xd799` checksum-mismatch disconnect handler address | C4 open | Medium |
| Confirm heap walk presence/absence (full address-space scan path) | #936 | Medium |
| Verify `CheaterLdFlag` persistence across zone/session | #935 | Low |

---

## 5. References

- `docs/wiki/Research-Anti-Detection.md` — canonical anti-detection research; §Ghidra-Verified Findings
- `docs/wiki/Research-EQ-AntiCheat-Notes.md` — hook scanner decompile, monitored functions table
- `docs/wiki/Security-and-Anti-Detection-Notes.md` — current evasion architecture
- `docs/research/cheater-ld-flag.md` — CheaterLdFlag address discovery
- `docs/research/hook-detection-surface.md` — hook catalog and migration priority
- `docs/research/C4-integrity-callsite-xref.md` — server-initiated integrity call graph
- `docs/research/B9-iat-vtable-patching.md` — IAT/VTable patching analysis
- `textquest-common/src/offsets.rs` lines 294-420 — all anti-cheat constants
- `textquest-dll/src/hooks/memcheck.rs` — server memcheck responder
- `textquest-dll/src/hooks/hwbp.rs` — HWBP engine
