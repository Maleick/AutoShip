# C2 Early Bird Race Feasibility

**Issue:** #2185
**Date:** 2026-04-21
**Confidence:** Medium (SME-sourced AC timing, community-sourced APC technique, no live-validated win window yet)
**Exposure categories touched:** Injection method, hook installation timing, module presence

---

## Summary

Early Bird APC injection is a technique where a DLL is injected into a process before its main thread
resumes, causing the payload to execute before most user-mode code — including anti-cheat (AC)
initialization — has run. If EQ's AC initializes lazily in the main game loop rather than during
process startup, an Early Bird injection wins the race and establishes HWBP hooks _before_ the AC
establishes any baseline.

This document analyzes the EQ AC initialization sequence, the conditions required for a race win,
and the practical feasibility within TextQuest's current architecture.

---

## 1. EQ Anti-Cheat Initialization Timeline

### What is confirmed (Ghidra-verified, 2026-04-03)

| Event                                                       | Location / Evidence | Timing                       |
| ----------------------------------------------------------- | ------------------- | ---------------------------- |
| Process startup (DllMain calls, CRT init, TLS callbacks)    | ntdll loader        | T=0                          |
| `eqgame.exe` WinMain entry                                  | EQ startup          | T≈50-200ms                   |
| Import library init (DirectX, WinSock, etc.)                | EQ startup          | T≈100-400ms                  |
| EQ network connection established                           | After server select | T=varies                     |
| `MainGameLoop_AntiCheatValidation` hook-check loop          | `0x140270d00`       | **T=post-login, continuous** |
| Server memcheck opcode `0x4f27` → `FUN_1400b5760`           | Server-initiated    | T=post-zone-connect          |
| File integrity check `FUN_140564bc0` / WorldAuthenticate    | `FUN_1402c9c80`     | T=per zone-connect           |
| Message counter heartbeat `FUN_1401a4650` (opcode `0xbb29`) | Main loop           | T=every 500ms after connect  |

**Key finding:** EverQuest's **client-side** AC validation (`MainGameLoop_AntiCheatValidation`)
runs inside the main game loop, not at process startup or DLL load time. It is not present in
TLS callbacks or DllMain. This means there is a window — from process creation through the point
the main game loop first executes — where the AC hook-check is not yet running.

### What is unconfirmed / open

- Whether any out-of-band Daybreak EAC/DLLMain-level AC code runs _before_ `WinMain`
  (e.g. injected via TLS callback in a middleware DLL). **Not observed in import table or
  disassembly to date** — `GetThreadContext`/`SetThreadContext` absent from IAT.
- Exact millisecond timestamp when `MainGameLoop_AntiCheatValidation` first checks hooks
  (dependent on login screen UI load time, which varies ~2-15 seconds).
- Whether the server initiates a memcheck before the player reaches character select.

---

## 2. Early Bird APC Technique

### Mechanism

Early Bird APC (also called "Early Bird Code Injection") works as follows:

1. **Create the target process suspended** (`CREATE_SUSPENDED` flag in `CreateProcessW`).
   The process is created but the main thread is not yet scheduled. Only the loader runs up
   to the process entry point handoff.
2. **Allocate + write payload** into the suspended process's virtual address space
   (`VirtualAllocEx` + `WriteProcessMemory`).
3. **Queue an APC** targeting the main thread (`NtQueueApcThread` or `QueueUserAPC`).
   APCs queued to a thread fire when that thread enters an alertable wait state.
4. **Resume the main thread** (`ResumeThread` / `NtResumeThread`).
5. The Windows loader's initialization path for a newly resumed thread includes alertable
   waits (specifically inside `LdrInitializeThunk` / `NtTestAlert`). The APC fires at this
   point — **before `WinMain` is called, before any application code runs**.

This guarantees the payload code runs before any game-level initialization, including any
AC code that EQ itself initializes during `WinMain`.

### Why it beats `CreateRemoteThread + LoadLibraryW`

| Aspect              | `CreateRemoteThread + LoadLibraryW` (current) | Early Bird APC                               |
| ------------------- | --------------------------------------------- | -------------------------------------------- |
| Injection timing    | After process is fully running                | Before WinMain executes                      |
| AC baseline race    | AC may already be running                     | AC definitely not running yet                |
| Detectability (ETW) | `ImageLoad` ETW event for LoadLibraryW        | APC queue event visible via ETW              |
| Thread creation     | Creates a new thread (suspicious)             | Executes on main thread (normal)             |
| Loader lock         | LoadLibraryW acquires loader lock             | Fires before loader lock is released to user |

---

## 3. EQ-Specific Race Window Analysis

### Process launch sequence (current TextQuest flow)

```
TextQuest spawner.rs
  └─ CreateProcessW (PROCESS_CREATION_FLAGS(0) — NOT suspended)
        │
        ▼
  eqgame.exe starts running
  EQ CRT + DLL loads (~100-400ms)
  EQ WinMain begins
  EQ loads login screen (~2-15s)
  ...
  [TextQuest inject window opens here — post-process-start]
        │
        ▼
  textquest/src/inject/loader.rs
    └─ CreateRemoteThread + LoadLibraryW
          │
          ▼
    DllMain → CreateThread → initialize()
    → install_hooks() → HWBP on MainLoop, chat, etc.
```

### Current injection timing problem

`spawner.rs` launches EQ with `PROCESS_CREATION_FLAGS(0)` — the process is **not** created
suspended. This means EQ runs freely from the moment of launch. TextQuest must wait for the
process to start, open a handle to it, allocate memory, write the DLL path, and call
`CreateRemoteThread`. On a typical machine this takes 200ms–2s after `eqgame.exe` first appears
in the process list.

By that point:

- EQ CRT initialization has completed
- All import DLLs have run their own `DllMain` calls
- The login screen is likely loading or loaded
- `WinMain` is well into execution

The `MainGameLoop_AntiCheatValidation` hook-check loop at `0x140270d00` is **NOT running yet**
at this point (the main game loop requires a fully loaded server connection), so the race is
technically won. However:

1. The **baseline for the hook-check is established when the check first runs**, not at process
   startup. This means the AC observes function prologues as they are at first-check-time, not
   at startup.
2. Current TextQuest hooks (HWBP, no byte modification) do not alter function prologues —
   so the current hook-check is already beaten by design, regardless of injection timing.
3. The risk vector is the **server-initiated memcheck** (`0x4f27`), which fires post-connection
   and can hash arbitrary `.text` ranges. This is where injection timing matters most.

### Early Bird window if we own the launcher

If TextQuest launches EQ **with `CREATE_SUSPENDED`** and queues an APC before resuming:

```
TextQuest spawner (modified):
  └─ CreateProcessW with CREATE_SUSPENDED
        │
        ▼ Main thread halted at LdrInitializeThunk
  VirtualAllocEx + WriteProcessMemory (DLL or shellcode)
  NtQueueApcThread (main thread, points to DLL loader or reflective stub)
  ResumeThread
        │
        ▼ APC fires inside LdrInitializeThunk alertable wait
  Our code runs — before WinMain, before any EQ code
  HWBP hooks installed
  PEB unlink + PE header erase done
        │
        ▼ LdrInitializeThunk completes → WinMain called
  EQ starts normally — hooks already in place
  MainGameLoop_AntiCheatValidation eventually runs:
    - Checks function prologues → clean (HWBP doesn't patch bytes)
    - Sees no E9/FF25 patterns → passes
```

**Race result: WIN.** Our hooks are in place before any EQ code runs. The AC's first scan
occurs with our HWBP hooks already installed — and since HWBP hooks leave no byte-level
footprint, the scan passes.

---

## 4. Detection Risks of Early Bird APC

Early Bird is not free from detection. The risks shift rather than disappear:

| Risk                           | Description                                                                                                               | Severity | Mitigation                                                                                                                                                                      |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **ETW `ApcFired` events**      | Kernel ETW traces log APC delivery to threads. If a Daybreak agent collects kernel ETW, APC-sourced execution is visible. | Medium   | Daybreak EQ client does **not** import `EtwEventWrite` or `NtTraceEvent` (Ghidra-confirmed). ETW blinding via HWBP on `NtTraceEvent` already present in `stealth/etw_blind.rs`. |
| **Timing anomaly**             | `CREATE_SUSPENDED` + `ResumeThread` sequence has a characteristic delay between `ProcessStart` and first thread quantum.  | Low      | Window is milliseconds; unlikely to be fingerprinted without dedicated correlation.                                                                                             |
| **APC in loader path**         | Windows debuggers and AC tools can detect APCs queued to threads while still in `LdrInitializeThunk`.                     | Medium   | Only exploitable if a kernel-mode AC driver is watching. EQ has no kernel driver (no `NtQuerySystemInformation` driver enumeration in IAT).                                     |
| **Payload writeability**       | `VirtualAllocEx` creates a `MEM_PRIVATE` RWX or RW+RX region. VAD walk from kernel sees it.                               | Medium   | Use reflective loader that marks pages appropriately post-load. `stealth/section_remap.rs` exists for COW conversion.                                                           |
| **LoadLibraryW call from APC** | If APC payload calls `LoadLibraryW`, the `ImageLoad` ETW event still fires (loader lock acquired inside APC context).     | High     | Use reflective injection (`textquest/src/inject/reflective.rs`) inside the APC — no `LoadLibraryW`, no ETW event, no PEB entry until we add it manually.                        |

---

## 5. Comparison with Current Architecture

### Current hook approach (HWBP, already AC-safe)

The existing HWBP approach in `textquest-dll/src/hooks/hwbp.rs` already defeats the primary
client-side scan (`MainGameLoop_AntiCheatValidation`):

- Ghidra confirms `GetThreadContext`/`SetThreadContext` are **NOT in EQ's import table**.
- The hook-check at `0x140270d00` looks for `E9` (JMP) or `FF25`/`FF24` prefixes only.
- HWBP hooks write nothing to code — the check sees clean prologue bytes and passes.

This means the **Early Bird benefit for the current hook set is marginal** with respect to the
client-side hook scan. The HWBP approach already wins that check regardless of timing.

### Where Early Bird provides real value

| AC system                                 | Current timing (post-start)             | Early Bird timing                     | Benefit                                       |
| ----------------------------------------- | --------------------------------------- | ------------------------------------- | --------------------------------------------- |
| Client hook scan (`0x140270d00`)          | Already bypassed (HWBP = no byte mod)   | Already bypassed                      | No change                                     |
| Server memcheck (`0x4f27`)                | Must pass regardless of timing          | Must pass regardless of timing        | No change                                     |
| ETW `ImageLoad` for injected DLL          | Fires when `LoadLibraryW` runs          | Suppressed with reflective APC loader | **Real gain**                                 |
| PEB module presence                       | Visible until `peb_unlink` runs         | Never added to PEB if reflective      | **Real gain**                                 |
| Process scan (`CreateToolhelp32Snapshot`) | DLL visible in module list until unlink | DLL never in list                     | **Real gain**                                 |
| Startup process enum (cross-client)       | Orchestrator process visible at launch  | No difference                         | No change                                     |
| BytePatch hooks (if any were used)        | Visible to memcheck                     | Present at baseline — invisible       | **Theoretical gain** (not applicable to HWBP) |

**Conclusion:** Early Bird's primary value for TextQuest is not timing the hook-check race —
it is enabling **reflective injection without a `LoadLibraryW` call**, which eliminates the
ETW `ImageLoad` event and PEB module list entry entirely.

---

## 6. Implementation Path

### Required changes to win the race

**Step 1: Modify `spawner.rs` to use `CREATE_SUSPENDED`**

```rust
// In textquest/src/launcher/spawner.rs
use windows::Win32::System::Threading::CREATE_SUSPENDED;

CreateProcessW(
    None,
    windows::core::PWSTR(cmd_wide.as_mut_ptr()),
    None,
    None,
    false,
    CREATE_SUSPENDED,   // <-- change from PROCESS_CREATION_FLAGS(0)
    None,
    windows::core::PCWSTR(eq_dir.as_ptr()),
    &si,
    &mut pi,
)?;
```

**Step 2: Queue APC to main thread using reflective loader**

The existing `textquest/src/inject/reflective.rs` is a reflective DLL loader. It needs to be
callable as an APC target. The APC callback signature is:
`unsafe extern "system" fn(parameter: usize)` — the `parameter` is the address of the
shellcode/reflective stub in the remote process.

```rust
// Rough sketch — not production code
let apc_target = remote_shellcode_addr as usize;
NtQueueApcThread(
    pi.hThread,
    Some(reflective_loader_entry), // fn(usize)
    apc_target,
    0,
    0,
)?;
ResumeThread(pi.hThread)?;
```

**Step 3: Handle loader-lock constraint**

The APC fires during `LdrInitializeThunk`. This is a constrained context:

- `LoadLibraryW` must NOT be called (causes deadlock / loader lock reentry)
- Direct memory operations and VEH installation are safe
- `CreateThread` is safe once the loader completes (not inside the APC itself)

The reflective loader resolves this: it manually maps the DLL, resolves imports, and calls
`DllMain` without going through the loader. This is exactly how `textquest/src/inject/reflective.rs`
is designed to operate.

**Step 4: HWBP installation within APC context**

HWBP hooks (`NtSetContextThread`) require targeting a specific thread. In Early Bird context,
the main thread is the only thread — install HWBP on it directly. The VEH handler must be
registered before the HWBP is armed (standard requirement, already handled in `hooks/hwbp.rs`).

---

## 7. Open Questions and Validation Requirements

| Question                                                                       | Confidence | How to validate                                                   |
| ------------------------------------------------------------------------------ | ---------- | ----------------------------------------------------------------- |
| Does `LdrInitializeThunk` enter an alertable wait reliably on Win10/11?        | Medium     | Test on Frostreaver with simple APC payload                       |
| Does EQ set up any TLS callbacks that establish AC state before `WinMain`?     | Low        | Ghidra: check PE TLS directory in eqgame.exe header               |
| Can a reflective DLL be loaded from within an APC context without deadlock?    | Medium     | Test with minimal reflective stub + `OutputDebugString`           |
| Does `MainGameLoop_AntiCheatValidation` fire before the first server connect?  | Medium     | Instrument with logging; watch for `0xfbb` packet pre-login       |
| Does Daybreak collect kernel ETW (process APC trace) on the EQ client host?    | Unknown    | No evidence in current IAT; would require kernel driver           |
| Does the thread pool availability issue (noted in `lib.rs`) affect Early Bird? | High       | Not applicable — Early Bird uses main thread APC, not thread pool |

---

## 8. Recommendation

**Feasibility: High for achieving pre-WinMain execution.**
**Value: Medium — real gain is ETW/PEB suppression, not hook-check timing.**

The race window exists and is winnable. EQ's client-side AC (`MainGameLoop_AntiCheatValidation`)
does not run until well into the login screen phase, which is typically several seconds after process
creation (roughly aligning with the earlier ~2–15 second login-screen timing). Early Bird comfortably wins this window.

However, TextQuest's HWBP hook architecture already defeats the hook-check regardless of timing.
The marginal gain from Early Bird is:

1. Eliminating the `LoadLibraryW` ETW `ImageLoad` event (high value)
2. Never appearing in the PEB module list (high value — PEB unlink currently runs post-init)
3. Enabling a narrower process-scan visibility window (medium value)

**Recommended next step:** Validate reflective APC loading on Frostreaver before investing in
spawner changes. The reflective loader exists; the APC queue path needs a minimal PoC.

**Blocked by:** Live testing required. Mark as `Needs Live Proof` per M5 gate rules.

---

## 9. Evidence State

| Claim                                                                        | Source                               | State                            |
| ---------------------------------------------------------------------------- | ------------------------------------ | -------------------------------- |
| `MainGameLoop_AntiCheatValidation` is main-loop-only (not startup)           | Ghidra @ `0x140270d00`               | Live-validated                   |
| `GetThreadContext`/`SetThreadContext` not in EQ IAT                          | Ghidra IAT scan                      | Live-validated                   |
| EQ AC checks for `E9`/`FF25`/`FF24` prologue bytes only                      | Ghidra decompile                     | Live-validated                   |
| Early Bird fires before `WinMain` via alertable wait in `LdrInitializeThunk` | Windows internals / community        | Research-backed                  |
| Daybreak has no kernel-mode AC driver                                        | IAT analysis (no driver APIs)        | Research-backed (not exhaustive) |
| APC survives reflective load without loader lock deadlock                    | Not yet tested on Frostreaver        | Provisional / Needs Live Proof   |
| EQ has no TLS callback AC code                                               | Not yet checked via Ghidra PE header | Open question                    |

---

## Related Docs

- `docs/wiki/Research-Anti-Detection.md` — EQ AC systems table and M7 evasion architecture
- `docs/wiki/Research-EQ-AntiCheat-Notes.md` — Hook scan address details (`0x140270d00`)
- `docs/wiki/Research-Syscall-Evasion.md` — RecycledGate indirect syscall layer
- `docs/wiki/Stealth-Stack-Reference.md` — All stealth modules, including ETW blinding
- `docs/wiki/Anti-Cheat-Deep-Dive-Injection-Memory-VEH.md` — Current injection pipeline
- `textquest/src/launcher/spawner.rs` — Current process launch (change target)
- `textquest/src/inject/reflective.rs` — Reflective loader (APC payload candidate)
- `textquest-dll/src/hooks/hwbp.rs` — HWBP engine (already AC-safe)
- `textquest-dll/src/stealth/etw_blind.rs` — ETW blinding (complements Early Bird)
