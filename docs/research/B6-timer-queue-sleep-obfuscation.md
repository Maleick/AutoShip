# B6 — Timer-Queue Sleep Obfuscation (Ekko / Gargoyle Variants)

**Issue:** #2180  
**Status:** Research + Stub (implementation gated on live test rig)  
**Last updated:** 2026-04-21

---

## Problem Statement

`textquest-dll/src/stealth/mod.rs` manages a **per-frame** wake/sleep cycle:
decrypt `.text` + `PAGE_EXECUTE_READ` at frame start, re-encrypt + `PAGE_READWRITE`
at frame end. This works well while EQ's game loop is ticking (≈60 Hz), but fails
in two long-idle scenarios:

| Scenario                          | Duration           | Risk                                                                            |
| --------------------------------- | ------------------ | ------------------------------------------------------------------------------- |
| Login screen / character-select   | 30 s – several min | DLL stays decrypted; EQ's ~3-min memshift scan can land during plaintext window |
| Alt-tabbed or zone-loading client | 10 s – several min | Same exposure; no frame ticks fire                                              |

The **EQ memshift scan** (opcode `0x4f27`, handler `FUN_1400b5720`) lets the server
request a hash of any `.text` address range on demand and compare it against its own
baseline. A single scan landing during an idle decrypted window exposes all patches.

---

## Background: Sleep Obfuscation Tradecraft

### Gargoyle (Josh Lospinoso, 2017)

**Core idea:** A shellcode or DLL payload lives in memory that is marked `PAGE_READWRITE`
(non-executable) except for the brief window when it actually needs to execute. A
Windows Waitable Timer (`CreateWaitableTimer` / `SetWaitableTimer`) is armed with a
periodic interval; its APC callback fires on an alertable thread, re-marks the region
`PAGE_EXECUTE_READ`, runs the payload, then re-marks it `PAGE_READWRITE` before the
next scan window.

**Key properties:**

- Between executions the region shows no executable pages — evades `VirtualQuery` walks
  that look for RX regions not backed by known modules.
- The timer itself is a normal kernel object; no suspicious threads.
- Original implementation uses a ROP chain in the stack frame to avoid the callback
  pointing directly into the payload.

**Detection surface:**

- `VirtualProtect` call monitoring (the transition is observable via ETW or kernel
  callbacks).
- ROP gadget scanning in thread stacks.
- Waitable timer APC threads must be in an alertable wait (`SleepEx`, `WaitForSingleObjectEx`).

**References:**

- Josh Lospinoso, "Gargoyle" (2017): https://jlospinoso.github.io/security/assembly/c/2017/03/04/gargoyle.html
- Source: https://github.com/JLospinoso/gargoyle
- Relevant to TextQuest: `docs/wiki/Research-Anti-Detection.md` row "Persistence → Gargoyle timer-based execution"

---

### Ekko (C5pider / 5pider-man, ~2022)

**Core idea:** A sleep obfuscation technique designed for Cobalt Strike BOFs / reflective
loaders. Uses **`CreateTimerQueueTimer`** to schedule a chain of three timer callbacks
that execute in sequence via OS thread pool worker threads:

1. **Callback 1** — captures current thread context (snapshot of RSP, RIP, etc.) via
   `RtlCaptureContext`; arms callback 2.
2. **Callback 2** — sets the beacon's memory to `PAGE_READWRITE`, XOR-encrypts the
   region, then sets thread context to a spoofed frame (via `NtContinue`) pointing into
   a `WaitForSingleObject` call to sleep the thread; arms callback 3.
3. **Callback 3** — fires after the sleep duration; XOR-decrypts, restores
   `PAGE_EXECUTE_READ`, restores the original thread context via `NtContinue`.

**Key properties:**

- No standalone `CreateThread` — callbacks run on the process-default thread pool (OS
  worker threads), consistent with PoolParty (SafeBreach, 2023).
- Stack spoofing: during the encrypted window the sleeping thread's call stack shows
  `WaitForSingleObject` inside `ntdll`, not the DLL payload.
- Timer queue handles are valid NT objects; timer creation is not suspicious in a
  thread-pool-heavy process.

**Detection surface:**

- `CreateTimerQueueTimer` is visible in IAT if not obfuscated; use `GetProcAddress`
  resolved at runtime instead.
- Timer callbacks fired on pool workers can be enumerated via thread pool work-item
  inspection (theoretical; not observed in EQ AC).
- `NtContinue` with a synthetic `CONTEXT` is a behavioral indicator some EDRs flag.

**References:**

- C5pider, "Ekko" (2022): https://github.com/Cracked5pider/Ekko
- matro7sh BypassAV §3.1 (Execution Delays & Sleep Obfuscation → Foliage, Ekko, DeathSleep)
- `docs/etw-ti-setthreadcontext.md` — ETW `SetThreadContext` surface analysis

---

### Foliage / DeathSleep (adjacent techniques, same family)

| Technique  | Author         | Key difference                                                                                                 |
| ---------- | -------------- | -------------------------------------------------------------------------------------------------------------- |
| Foliage    | klezVirus      | Uses `RtlRegisterWait` (thread pool wait objects) rather than timer queue; callback fires after handle signals |
| DeathSleep | janogleam      | Adds encrypted stack frames; sleep-time stack looks like legitimate kernel waits                               |
| Hypnus     | TextQuest #345 | Our planned stack-spoof wrapper — `stack_spoof.rs` stub already in tree                                        |

All three share the same invariant: payload is non-executable and/or encrypted during
idle windows, with execution re-enabled only for the minimum required window.

---

## Target Idle Durations (TextQuest Context)

Based on observed EQ session patterns:

| Idle Type                          | Typical Duration | Target Encrypted Fraction      |
| ---------------------------------- | ---------------- | ------------------------------ |
| Login screen (account select)      | 30 s – 3 min     | > 95 %                         |
| Character select                   | 10 – 60 s        | > 95 %                         |
| Zone load ("Loading, please wait") | 5 – 30 s         | > 90 %                         |
| Alt-tabbed (long AFK)              | 5 – 30 min       | > 98 %                         |
| EQ ~3-min memshift scan window     | ≈ 6 s per scan   | Must be encrypted at scan time |

The **1.5-second timer interval** proposed in the issue design sketch gives:

- Encrypted window per cycle: ~1.49 s (wake for < 10 ms to execute hooks)
- Encrypted fraction: > 99.3 %
- Worst-case scan exposure: a scan landing in the < 10 ms wake window → 0.7 %
  probability per scan, negligible across 36 clients

For long AFK idles, a **variable-interval** approach (initial 1.5 s, backoff to 5 s
after 60 s idle) further reduces VirtualProtect call frequency, lowering ETW
telemetry volume.

---

## Interaction with Per-Frame Path

The existing per-frame path (`stealth::wake()` / `stealth::sleep()`) and the timer-queue
path must be mutually exclusive. Race conditions between them would produce:

- Double-encrypt (XOR applied twice → plaintext, undetectable but wastes a cycle)
- Decrypt during timer callback while frame path assumes encrypted state

**Proposed ownership model:**

```
enum SleepOwner { None, Frame, TimerQueue }
static OWNER: Mutex<SleepOwner> = Mutex::new(SleepOwner::None);
```

- `stealth::wake()` — acquires OWNER, asserts owner is `None` or `Frame`, sets `Frame`.
- `stealth::sleep()` — sets `None`.
- Timer callback — tries to acquire OWNER; if owner is `Frame`, skips (frame path has
  control); if `None`, sets `TimerQueue`, runs encrypt cycle, releases.

**SAFE_MODE guard:** Timer callback must check `hooks::integrity::SAFE_MODE` before
running. If set, the callback skips the VirtualProtect transition (hook callbacks may
be mid-flight; race with encrypt is unsafe).

---

## Windows API Surface

| API                             | Purpose                                                                    | Module   |
| ------------------------------- | -------------------------------------------------------------------------- | -------- |
| `CreateTimerQueue`              | Create a timer queue (optional; NULL = default process queue)              | kernel32 |
| `CreateTimerQueueTimer`         | Register a periodic callback on the queue                                  | kernel32 |
| `DeleteTimerQueueTimer`         | Remove a timer (INVALID_HANDLE_VALUE = wait for completion)                | kernel32 |
| `DeleteTimerQueue`              | Destroy queue                                                              | kernel32 |
| `RtlRegisterWait` (alternative) | Register a wait on a handle; callback fires when handle signals or timeout | ntdll    |
| `NtContinue`                    | Restore CONTEXT record (used by Ekko for stack spoof during sleep)         | ntdll    |
| `CreateWaitableTimerEx`         | High-resolution waitable timer (Gargoyle original)                         | kernel32 |
| `SetWaitableTimer`              | Arm the waitable timer                                                     | kernel32 |

For TextQuest we prefer `CreateTimerQueueTimer` (Ekko-style) over
`CreateWaitableTimerEx` (Gargoyle-original) because:

1. Callbacks fire on the **process-default thread pool** — consistent with the
   existing `thread_pool.rs` (PoolParty) approach; no new thread-pool is created.
2. No APC / alertable-wait requirement — Gargoyle needs a thread blocking in
   `SleepEx(..., TRUE)` which is a distinct behavioral signal.
3. Integrates naturally with `DeleteTimerQueueTimer(INVALID_HANDLE_VALUE)` for
   clean shutdown.

---

## Stub Module Location

`textquest-dll/src/stealth/timer_queue_sleep.rs` — see stub in tree (created with
this issue). The stub defines the public API surface and documents the implementation
contract; Windows-specific body is `todo!()` pending live-test rig availability.

**Public API:**

```rust
/// Initialize the timer-queue sleep path.
/// Must be called after `stealth::init()` (per-frame path).
/// Interval controls how often the encrypt/decrypt cycle fires during idle.
pub fn init(interval_ms: u32) -> Result<(), TimerQueueError>

/// Shut down the timer-queue sleep path.  Blocks until the in-flight
/// callback (if any) completes.  Decrypts .text if currently encrypted.
pub fn shutdown()

/// Returns true if the timer-queue path is currently active.
pub fn is_active() -> bool
```

---

## Integration Plan (Future PR)

1. **Add `Win32_System_Threading` timer-queue features** to `textquest-dll/Cargo.toml`
   windows crate feature list (already present: `Win32_System_Threading`).

2. **Implement `timer_queue_sleep.rs`** Windows inner module:
   - `CreateTimerQueueTimer` with `WT_EXECUTEDEFAULT` (pool thread, not persistent).
   - Callback acquires OWNER mutex; skips if `Frame` owner; skips if `SAFE_MODE`.
   - Callback: `page_guard::set_writable()` → `text_encrypt::encrypt()` → sleep
     for `interval_ms - overhead` via `WaitForSingleObject` → `text_encrypt::decrypt()`
     → `page_guard::set_executable()` → release OWNER.

3. **Expose toggle in config** — `[stealth] timer_queue_sleep = true` /
   `timer_queue_interval_ms = 1500`.

4. **Live acceptance test** (character at login screen 5 min):
   - Sampling thread reads `.text` bytes every 500 ms; checks byte 0 against known
     plaintext value.
   - Log encrypted fraction; target > 95 %.

5. **Interaction test** (unit):
   - Simulate rapid `wake()` / `sleep()` calls interleaved with timer callback;
     verify OWNER mutex prevents double-encrypt / race.

---

## Risk Assessment

| Risk                                                  | Likelihood | Mitigation                                                                      |
| ----------------------------------------------------- | ---------- | ------------------------------------------------------------------------------- |
| `VirtualProtect` call frequency flagged by ETW        | Medium     | Backoff interval during long idle; `etw_blind.rs` already suppresses ETW writes |
| Timer callback thread observable via pool enumeration | Low        | Callbacks are anonymous work items; no persistent thread created                |
| Race between timer callback and frame path            | Medium     | OWNER mutex with try-lock (skip, not block)                                     |
| `NtContinue` EDR flag (if stack spoof used)           | Low        | Skip `NtContinue` path initially; add only if stack scan required               |
| Double-encrypt producing plaintext                    | Medium     | `CODE_ENCRYPTED` atomic + OWNER state; unit test required                       |

---

## References

1. Josh Lospinoso, "Gargoyle" (2017) — https://jlospinoso.github.io/security/assembly/c/2017/03/04/gargoyle.html
2. C5pider, "Ekko Sleep Obfuscation" (2022) — https://github.com/Cracked5pider/Ekko
3. matro7sh, "BypassAV" — §3.1 Execution Delays & Sleep Obfuscation (Foliage, Ekko, DeathSleep)
4. SafeBreach PoolParty — 8 thread pool injection variants (referenced in `thread_pool.rs`)
5. `docs/wiki/Research-Anti-Detection.md` — §Recommended M7 Evasion Architecture row "Persistence → Gargoyle"
6. `docs/etw-ti-setthreadcontext.md` — ETW `SetThreadContext` surface analysis
7. `textquest-dll/src/stealth/mod.rs` — existing per-frame sleep path
8. `textquest-dll/src/stealth/thread_pool.rs` — PoolParty primitives
9. `textquest-dll/src/hooks/integrity.rs` — `SAFE_MODE` flag definition
