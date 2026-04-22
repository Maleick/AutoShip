# B8: Early Bird APC Injection

## Overview

Early Bird APC injection is a process injection technique that queues an Asynchronous Procedure Call (APC) to the main thread of a newly spawned process **before** that process's entry point executes. The thread is created in a suspended state; the APC is enqueued; then the thread is resumed. When the OS scheduler finally runs the thread, it drains its APC queue before transferring control to the process entry point, so our shellcode executes before any user-mode anti-cheat initialization can occur.

In the context of EverQuest (eqgame.exe), the anti-cheat (AC) subsystem initializes during process startup — loading drivers, establishing communication channels, and scanning the process image. If we can race that initialization window, hooks and patches placed via Early Bird APC execute in a pre-AC environment, dramatically reducing detection surface.

---

## Why Early Bird Beats Standard APC Injection

Standard `QueueUserAPC` injection queues an APC to an **already-running** thread. The target thread must be in an alertable wait state (`SleepEx`, `WaitForSingleObjectEx`, `ReadFileEx`, etc.) to drain it, and by that time the AC is fully running.

Early Bird sidesteps this:

| Property                      | Standard APC                            | Early Bird APC           |
| ----------------------------- | --------------------------------------- | ------------------------ |
| Target thread state           | Must be alertable                       | Suspended (pre-startup)  |
| AC initialization state       | Fully initialized                       | Not yet started          |
| Thread creation required      | No                                      | Yes (suspended spawn)    |
| Detection by AC               | High — thread scan catches foreign APCs | Low — AC not running yet |
| Works with modern anti-cheat? | Rarely                                  | Frequently               |

---

## Windows Internals: APC Drain Sequence

When a thread is created with `CREATE_SUSPENDED` (`dwCreationFlags = 0x4`) and then resumed via `ResumeThread`, the kernel schedules the thread but — critically — the thread does not jump immediately to its start address. Instead, the initial kernel-to-user transition executes `ntdll!LdrInitializeThunk`, which:

1. Initializes the loader and TLS.
2. Drains the **user-mode APC queue** via `ntdll!KiUserApcDispatcher`.
3. Transfers control to the process entry point (PE loader, then WinMain).

Our APC callback fires at step 2. The application's `WinMain`/`DllMain` has not run. No EQ code is executing. No AC driver hooks are installed.

---

## Technique Mechanics

### Step 1 — Spawn EQ Suspended

```rust
// CreateProcess with CREATE_SUSPENDED | CREATE_NEW_CONSOLE
let mut si = STARTUPINFOW::default();
let mut pi = PROCESS_INFORMATION::default();
CreateProcessW(
    /* lpApplicationName */ eq_path,
    /* lpCommandLine     */ None,
    /* lpProcessAttributes */ None,
    /* lpThreadAttributes  */ None,
    /* bInheritHandles   */ FALSE,
    /* dwCreationFlags   */ CREATE_SUSPENDED | CREATE_NEW_CONSOLE,
    /* lpEnvironment     */ None,
    /* lpCurrentDirectory */ None,
    &si,
    &mut pi,
)?;
```

`pi.hThread` is the main thread handle, suspended at creation. `pi.hProcess` is used for memory allocation in the target.

### Step 2 — Allocate Shellcode in Target

```rust
// Allocate RW memory in target process
let remote_buf = VirtualAllocEx(
    pi.hProcess,
    None,
    shellcode.len(),
    MEM_COMMIT | MEM_RESERVE,
    PAGE_READWRITE,
)?;

// Write payload
WriteProcessMemory(
    pi.hProcess,
    remote_buf,
    shellcode.as_ptr() as _,
    shellcode.len(),
    None,
)?;

// Flip to RX
let mut old_protect = PAGE_PROTECTION_FLAGS(0);
VirtualProtectEx(
    pi.hProcess,
    remote_buf,
    shellcode.len(),
    PAGE_EXECUTE_READ,
    &mut old_protect,
)?;
```

The `VirtualProtect` flip from RW to RX is done before `ResumeThread`. No writes occur during execution — the memory is already RX when the APC drains.

### Step 3 — Queue the APC

```rust
// Cast remote_buf to the PAPCFUNC signature
let apc_routine: PAPCFUNC = Some(std::mem::transmute(remote_buf));
QueueUserAPC(apc_routine, pi.hThread, 0)?;
```

`QueueUserAPC` enqueues our function pointer in the suspended thread's APC queue. Because the thread is suspended, it will not drain until `ResumeThread` is called.

### Step 4 — Resume

```rust
ResumeThread(pi.hThread)?;
```

The thread becomes runnable. The OS drains the APC queue (our shellcode runs) and then transfers to EQ's actual entry point. EQ initializes normally — it does not know its own startup code ran after ours.

---

## Payload Design for EQ

The APC callback receives a single `ULONG_PTR` parameter (the `dwData` passed to `QueueUserAPC`). For a reflective loader payload:

```
APC callback (dwData = 0)
  → locate our DLL image in remote process (mapped by Step 2)
  → call reflective loader entry point
  → reflective loader fixes relocations, resolves imports, calls DllMain
  → DllMain submits init to thread pool (stealth::thread_pool)
  → APC returns
  → EQ entry point runs
```

The payload is a position-independent reflective loader — the same technique used by `textquest-dll`'s existing reflective injection path. The difference is **when** the loader is invoked (pre-AC vs. post-AC).

---

## Race Window Analysis

EQ's anti-cheat initialization sequence (based on observed behavior and common AC patterns):

```
Process start
  │
  ├─ ntdll!LdrInitializeThunk     ← [APC DRAINS HERE] ★
  ├─ kernel32!BaseProcessStart
  ├─ EQ CRT initialization
  ├─ WinMain
  │    ├─ AC driver connection      ← first AC contact
  │    ├─ AC heartbeat thread       ← background scan starts
  │    ├─ AC integrity scan         ← image hash verification
  │    └─ game subsystem init
  └─ Game loop
```

Our window is the gap between `LdrInitializeThunk` (APC drain) and `WinMain`. This window is reliable and consistent — it does not depend on process-specific timing or alertable waits.

---

## Detection Surface

### What EQ AC Can See

| Signal                                | Detectability                                     | Mitigation                                          |
| ------------------------------------- | ------------------------------------------------- | --------------------------------------------------- |
| `VirtualAllocEx` in target            | HIGH — `NtAllocateVirtualMemory` call in injector | Injector runs before AC; AC only monitors post-init |
| `WriteProcessMemory`                  | HIGH — same caveat                                | Same: pre-AC                                        |
| `QueueUserAPC`                        | MEDIUM — logged by ETW kernel provider            | ETW blind (`stealth::etw_blind`) can suppress       |
| Foreign APC in queue at resume        | LOW — AC not running when APC drains              | Core advantage of Early Bird                        |
| Reflective loader artifacts in memory | MEDIUM — PE headers visible                       | `stealth::pe_erase` + PEB unlink                    |
| `VirtualAllocEx` RW→RX flip           | MEDIUM                                            | Pre-flip before resume (no AC running)              |

### Injector Signature

The injector process itself is the primary detection vector. `CreateProcess` + `VirtualAllocEx` + `WriteProcessMemory` + `QueueUserAPC` + `ResumeThread` is a well-known pattern. Mitigations:

1. **Indirect syscalls** (`textquest-dll/src/syscall/`) — route `NtAllocateVirtualMemory`, `NtWriteVirtualMemory`, `NtQueueApcThread` through direct NTDLL dispatch stubs to avoid user-mode hooks on Win32 API layer.
2. **Injector disguise** — launch injector as a child of a trusted process (e.g., Windows Explorer) or use a hollowed host process.
3. **ETW suppression** — `stealth::etw_blind` patches ETW session buffer to blind kernel-mode event consumers.

---

## Integration with Existing TextQuest Architecture

### Current State

`textquest-dll/src/lib.rs` (DllMain) currently uses `CreateThread` to run the init sequence:

```rust
// Current approach in DllMain
let thread = CreateThread(
    None, 0,
    Some(init_thread_fn),
    Some(context),
    THREAD_CREATION_FLAGS(0),
    None,
);
```

This is called **after** injection, meaning AC is already running when `CreateThread` fires.

### Early Bird Integration Path

Early Bird APC replaces the injector's `LoadLibrary`/`CreateRemoteThread` calls. The DLL's `DllMain` can remain as-is; what changes is the **outer injector** (in `textquest` or `textquest-soul`) that launches EQ:

```
Before: textquest-soul spawns eqgame.exe → waits → injects DLL via LoadLibrary/CreateRemoteThread
After:  textquest-soul spawns eqgame.exe suspended → writes reflective loader → queues APC → resumes
```

The DLL payload itself does not change. The init thread spawned by `DllMain` now starts in a pre-AC window because the DLL's `DllMain` was invoked pre-AC.

### Thread Pool Init Compat

`stealth::thread_pool::submit_to_thread_pool` (PoolParty) submits work to the process-default thread pool. During Early Bird execution, the process thread pool may not yet be initialized (it is set up during CRT init, which happens after our APC). Two options:

1. **`CreateThread` from the APC** — acceptable; by the time EQ's AC scans threads, ours is already in a legitimate wait state.
2. **Delay init submission** — queue a small stub that waits for the CRT to finish initializing (by polling a known CRT global or sleeping briefly), then submits to the thread pool.

Option 1 is simpler and adequate for the pre-AC window.

---

## Comparison to PoolParty (Existing Approach)

| Property                 | PoolParty (current)   | Early Bird APC (B8)               |
| ------------------------ | --------------------- | --------------------------------- |
| Injector complexity      | Medium                | Higher (suspended spawn)          |
| AC window                | Post-init             | Pre-init                          |
| Thread pool availability | Guaranteed            | Not guaranteed (CRT not run)      |
| ETW detection vectors    | Thread pool work item | `NtQueueApcThread`                |
| Reliability              | High                  | High (well-tested technique)      |
| Loader lock safety       | No issue              | No issue (APC fires post-LdrInit) |

---

## Open Questions / Future Work

1. **Injector placement** — which process in the TextQuest stack spawns EQ? `textquest-soul` (orchestrator) or a dedicated launcher stub? A dedicated stub is preferable to avoid the orchestrator's process having suspicious API call patterns.

2. **CRT init dependency** — if our reflective loader calls into Rust std (which it does via `DllMain` → `initialize()`), and Rust std depends on the CRT being initialized, we may fault during Early Bird execution. The Rust no-std allocator path (`stealth::alloc`) avoids this. Track as a compatibility risk.

3. **NTDLL hook by AC** — some AC implementations hook `NtQueueApcThread` in NTDLL. Syscall dispatch (`textquest-dll/src/syscall/`) mitigates this by calling the kernel directly.

4. **Multiple APC slots** — `QueueUserAPC` can be called multiple times. A two-stage approach (stub APC → waits for CRT → loads full DLL) would be more robust than a single monolithic APC payload.

5. **WoW64 considerations** — EQ is a 32-bit process on 64-bit Windows. APC functions and remote allocation must use 32-bit addresses. The injector must be 32-bit or use WoW64 thunking correctly.

---

## References

- [Early Bird Code Injection - CyberArk (2018)](https://www.cyberark.com/resources/threat-research-blog/masking-malicious-memory-artifacts-part-ii-insights-from-moneta)
- Windows Internals, 7th Edition — Chapter 8: System Mechanisms (APC internals)
- `ntdll!NtQueueApcThread` / `ntdll!KiUserApcDispatcher` — NT source leaks and Geoff Chappell's documentation
- SafeBreach PoolParty (2023) — context for thread pool injection variants
- `textquest-dll/src/stealth/thread_pool.rs` — existing PoolParty implementation
- `textquest-dll/src/syscall/` — existing indirect syscall infrastructure
- `textquest-dll/src/stealth/etw_blind.rs` — ETW suppression for coverage
