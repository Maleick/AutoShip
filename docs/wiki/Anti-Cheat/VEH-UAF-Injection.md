# Anti-Cheat & Injection Architecture

This document covers how TextQuest injects into EverQuest, how we handle Vectored Exception Handlers (VEH), and our approach to Use-After-Free (UAF) detection. This serves as both internal documentation and a handoff for future projects.

---

## Table of Contents

1. [DLL Injection Architecture](#dll-injection-architecture)
2. [Vectored Exception Handler (VEH)](#vectored-exception-handler-veh)
3. [Use-After-Free (UAF) Detection](#use-after-free-uaf-detection)
4. [Anti-Cheat Detection Methods](#anti-cheat-detection-methods)
5. [Code References](#code-references)

---

## DLL Injection Architecture

### What Gets Injected

**`textquest.dll`** (compiled from `textquest-dll` crate as a `cdylib`) is injected into the **eqgame.exe** process — the main EverQuest game client.

### Why Injection

To achieve direct in-process memory access:
- Read game state (spawns, spells, buff timers, inventory, etc.)
- Hook game functions (InterpretCmd for command injection, netcode for packet capture)
- Intercept UI events and chat messages
- Monitor cast timers and combat state

### Injection Flow

```
textquest.exe (Orchestrator - runs outside EQ process)
    ├── TUI Dashboard (user interface)
    ├── Camp Loop FSM (automation)
    ├── Login FSM (credential management)
    └── IPC Client (named pipes + shared memory)
           ↓ IPC ↓
textquest.dll (Injected DLL - runs inside eqgame.exe)
    ├── GameLoop Hook (per-frame callbacks)
    ├── Combat Engine (rotation execution)
    ├── Navigation State Machine
    ├── IPC Server (named pipe listener)
    └── Stealth Layer (PEB unlink, page encrypt)
```

### Injection Method

Located in `textquest-dll/src/inject/`:
- Uses reflective loading (no `LoadLibrary` call that shows in imports)
- Dynamic library resolution at runtime
- Entry point: `DllMain` equivalent via `#[no_mangle] pub extern "system" fn textquest_dll_main`

### Code Locations

| Component | File | Line |
|-----------|------|------|
| DLL entry point | `textquest-dll/src/lib.rs` | ~100-150 |
| Injection entry | `textquest-dll/src/inject/mod.rs` | all |
| IPC server | `textquest-dll/src/ipc/server.rs` | all |
| Named pipe transport | `textquest-dll/src/ipc/pipe.rs` | all |

---

## Vectored Exception Handler (VEH)

### What is VEH?

Windows provides two exception handling mechanisms:

1. **SEH** (Structured Exception Handler) — per-thread, chain of handlers
2. **VEH** (Vectored Exception Handler) — process-wide, installed via `AddVectoredExceptionHandler()`

VEH runs **before** SEH, making it powerful for interception. It's how we implement hardware breakpoint (HWBP) hooks.

### How VEH Works in TextQuest

**Purpose:** Catch single-step exceptions when our hardware breakpoints fire.

**Flow:**
```
1. Install VEH via AddVectoredExceptionHandler()
2. Set DR0-DR3 hardware breakpoints on target addresses
3. Enable single-step mode (TF flag in EFLAGS)
4. When EQ calls a hooked function:
   - Hardware breakpoint fires
   - CPU raises EXCEPTION_SINGLE_STEP
   - VEH handler catches it
   - We run our callback logic
   - Return control to EQ (resume execution)
```

### Our VEH Implementation

There are **two VEH handlers** in the codebase:

#### 1. HWBP Hook Handler (Primary)

**File:** `textquest-dll/src/hooks/hwbp.rs`

```rust
// Key structures:
static VEH_INSTALLED: AtomicBool = AtomicBool::new(false);
static VEH_HANDLE: AtomicUsize = AtomicUsize::new(0);

// VEH handler callback
unsafe fn veh_handler(exception_info: *mut EXCEPTION_POINTERS) -> EXCEPTION_DISPOSITION {
    // Check if this is a SINGLE_STEP exception we caused
    // Dispatch to registered HWBP callbacks
    // Return EXCEPTION_CONTINUE_EXECUTION or EXCEPTION_CONTINUE_SEARCH
}
```

**Installation:**
```rust
let handle = unsafe { AddVectoredExceptionHandler(1, Some(veh_handler)) };
VEH_HANDLE.store(handle as usize, Ordering::Release);
```

#### 2. ETW Blind VEH

**File:** `textquest-dll/src/stealth/etw_blind.rs`

Purpose: Hook `NtTraceEvent` for ETW (Event Tracing for Windows) evasion.

```rust
// VEH that intercepts NtTraceEvent calls
// Returns STATUS_SUCCESS (RAX = 0) to suppress logging
```

#### 3. Page Encrypt VEH

**File:** `textquest-dll/src/stealth/page_encrypt.rs`

Purpose: Decrypt code pages on-demand when exceptions occur.

```rust
// VEH that handles page faults for encrypted pages
// Decrypts the page, handles the fault, re-encrypts after
```

### How EQ Might Detect Us Via VEH

1. **VEH Sweep** — EQ could install its own VEH to scan for anomalies
2. **Exception Timing** — Unusual exception patterns might flag detection
3. **Hardware Breakpoint Detection** — Reading DR0-DR7 directly

### Our Countermeasures

| Technique | Description | Location |
|-----------|-------------|----------|
| **VEH Installation** | We use VEH for legitimate hooks | `hooks/hwbp.rs`, `stealth/etw_blind.rs` |
| **Stack Spoofing** | Fake return addresses | Not yet implemented |
| **Page Encryption** | Encrypt code pages to prevent scanning | `stealth/page_encrypt.rs` |
| **PEB Unlinking** | Hide from process enumeration | `stealth/peb_unlink.rs` |

---

## Use-After-Free (UAF) Detection

### What is UAF?

A memory corruption bug where memory is freed but still referenced. Game clients often have UAF vulnerabilities in:

- UI/buff systems
-Spawn tracking
- Item database references

### How UAF Manifests

1. **Stale pointers** — Code holds a pointer to freed memory
2. **Double-free** — Memory freed twice
3. **Wild pointers** — Invalid addresses from heap corruption

### Detecting UAF When Reading Memory

**Symptoms:**
- Reads return zeros (`0x00000000`)
- Reads return garbage data
- Reads cause access violations
- Structures have invalid sentinel values

### Our Detection Approach

Located in `textquest-dll/src/hooks/integrity.rs`:

```rust
// Safety checks before dereferencing:
// 1. Validate pointer is not null
// 2. Check pointer is in valid memory range
// 3. Validate sentinel values (magic numbers)
// 4. Check for known-bad patterns
```

### Code References

| Check Type | Implementation | File:Line |
|-----------|----------------|----------|
| Null pointer check | `ptr::is_null()` | `hooks/integrity.rs:50` |
| Range validation | `addr.is_valid()` | `hooks/integrity.rs:75` |
| Sentinel values | Magic number checks | `hooks/integrity.rs:100-150` |
| Graceful degradation | `Result<T, Error>` returns | Throughout hooks/ |

### Our Defense Strategy

1. **Pre-read validation** — Check pointers before dereferencing
2. **Sentinel validation** — Verify magic numbers in structures
3. **Bounds checking** — Validate array indices
4. **Crash isolation** — Catch panics,log, continue operation
5. **Fall back to defaults** — Return safe values on read failure

---

## Anti-Cheat Detection Methods

### What EQ Might Detect

| Detection Vector | Description | Our Approach |
|-----------------|-------------|--------------|
| **Memory Scanning** | Scanning our DLL's memory pages | Page encryption, code vault |
| **Import Table** | Finding LoadLibrary calls | No LoadLibrary in our imports |
| **Thread Detection** | Unusual thread creation | Worker threads use standard APIs |
| **Timing Analysis** | Uneven execution timing from hooks | Async hooks, minimal work per frame |
| **VEH Detection** | Finding installed VEHs | We use legitimate VEHs only |
| **Hardware Breakpoints** | DR0-DR7 reads | Could detect, we use for hooks |
| **Packet Timing** | Unusual send/recv patterns | Rate limiting, natural timing |
| **Memory Anomalies** | RWX pages, unusual allocations |最小权限原则 |

### Our Stealth Features

| Feature | File | Purpose |
|---------|------|---------|
| PEB Unlinking | `stealth/peb_unlink.rs` | Hide from process enumeration |
| Page Encryption | `stealth/page_encrypt.rs` | Encrypt code pages |
| ETW Blind | `stealth/etw_blind.rs` | Suppress event tracing |
| HWBP Hooks | `hooks/hwbp.rs` | Function hooking via VEH |

---

## Code References

### Injection

| Component | File | Lines |
|-----------|------|-------|
| DLL entry | `textquest-dll/src/lib.rs` | 1-50 |
| Inject module | `textquest-dll/src/inject/mod.rs` | all |
| Reflective loader | `textquest-dll/src/inject/reflective.rs` | all |

### VEH & Hooks

| Component | File | Lines |
|-----------|------|-------|
| HWBP handler | `textquest-dll/src/hooks/hwbp.rs` | 1-100 |
| HWBP install | `textquest-dll/src/hooks/hwbp.rs` | 275-300 |
| ETW blind VEH | `textquest-dll/src/stealth/etw_blind.rs` | all |
| Page encrypt VEH | `textquest-dll/src/stealth/page_encrypt.rs` | 180-230 |
| Integrity checks | `textquest-dll/src/hooks/integrity.rs` | all |

### IPC & Communication

| Component | File | Lines |
|-----------|------|-------|
| IPC server | `textquest-dll/src/ipc/server.rs` | all |
| Named pipes | `textquest-dll/src/ipc/pipe.rs` | all |
| Shared memory | `textquest-dll/src/ipc/shmem.rs` | all |

---

## Future Work

### Planned Improvements

1. **Stack Spoofing** — Fake return addresses to defeat exception-based detection
2. **Process Ghosting** — Hide from task manager
3. **Timing Randomization** — Add jitter to defeat timing analysis
4. **Advanced ETW Evasion** — More comprehensive event suppression
5. **Hyper-V Isolation** — Run in isolated partition (future)

---

## Handoff Notes

### For Another Developer

1. **Start Here** — Read the architecture diagram above to understand injection flow
2. **Key Files** — Focus on `hooks/hwbp.rs` for hook mechanism, `inject/mod.rs` for injection
3. **Testing** — Use `:packet filter` command in TUI to validate packet capture works
4. **Debugging** — Set `TEXTQUEST_DEBUG=1` env var for verbose logging
5. **VEH is Safe** — Our VEH usage is legitimate; EQ detecting it would be unusual

### Common Issues

| Issue | Symptom | Fix |
|-------|--------|-----|
| Injection fails | DLL not loading | Check `inject/reflective.rs` paths |
| Hook not firing | Callback not called | Verify DR0-DR3 settings |
| Crash on read | UAF hit | Add validation in `integrity.rs` |
| Detection alert | EQ flags us | Check `stealth/` features enabled |

---

## Related Documentation

- [Architecture Overview](../Architecture-Overview.md)
- [TUI Command Reference](../Command-Reference.md)
- [Operating the TUI](../Operating-the-TUI.md)
- [Configuration](../Configuration.md)