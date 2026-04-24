# Stealth Stack Reference

**Comprehensive developer reference for all anti-detection and stealth modules in `textquest-dll`.**

This document is **excluded from the public site** and serves as internal architecture documentation for security engineers and maintainers.

---

## Module Inventory

| #   | Module                      | Location                | Layer       | Purpose                                                             |
| --- | --------------------------- | ----------------------- | ----------- | ------------------------------------------------------------------- |
| 1   | PEB Unlink                  | `stealth/peb_unlink`    | Visibility  | Remove DLL from 3 PEB module lists                                  |
| 2   | Page Encryption             | `stealth/page_encrypt`  | Memory      | Nighthawk-style VEH-based page encryption (~2% plaintext)           |
| 3   | Text Encrypt                | `stealth/text_encrypt`  | Code        | Layer 2: SSE2 SIMD XOR of .text section                             |
| 4   | Page Guard                  | `stealth/page_guard`    | Code        | Layer 1: VirtualProtect RW/RX toggle                                |
| 5   | PE Header Erase             | `stealth/pe_erase`      | Forensics   | Zero DOS + NT headers of loaded DLL                                 |
| 6   | ETW Blinding                | `stealth/etw_blind`     | Telemetry   | Patchless via DR0 HWBP on `NtTraceEvent`                            |
| 7   | Stealth Allocator           | `stealth/alloc`         | Memory      | Avoids VirtualAlloc: RtlAllocateHeap + NtCreateSection              |
| 8   | Trampoline Hardening        | `stealth/trampoline`    | Code        | XOR concealment + RX protection for detours                         |
| 9   | Stack Spoof                 | `stealth/stack_spoof`   | Call Stack  | RBP chain walk, replace rets with ntdll/kernel32 gadgets            |
| 10  | Section Remap               | `stealth/section_remap` | Memory      | Trigger COW: MEM_IMAGE → MEM_PRIVATE conversion                     |
| 11  | Thread Pool Injection       | `stealth/thread_pool`   | Execution   | PoolParty-style: `CreateThreadpoolWork` + `SubmitThreadpoolWork`    |
| 12  | Sleep Orchestrator          | `stealth/mod`           | Code        | Gargoyle-style 3-layer per-frame sleep (pg + text + spoof)          |
| 13  | RecycledGate + TartarusGate | `syscall/`              | Syscalls    | Indirect syscalls via `syscall;ret` gadget in real ntdll            |
| 14  | HWBP Engine                 | `hooks/hwbp`            | Hooking     | DR0-DR3 execution BPs, VEH dispatch, zero code modification         |
| 15  | Fingerprint Spoofing        | `hooks/fingerprint`     | EQ Protocol | Hook `SystemFingerprint` → unique per-client IDs from session token |
| 16  | Hook Integrity Check        | `hooks/integrity`       | Validation  | HWBP slot self-check pre-IPC activation, SAFE_MODE gate             |

---

## Layer 1: Visibility Evasion

### PEB Unlink (`textquest-dll/src/stealth/peb_unlink.rs`)

**Purpose:** Remove the injected DLL from the Process Environment Block's three module lists.

**Implementation:**

- Walks `PEB_LDR_DATA` inbound from `PEB.Ldr` (via `gs:[0x60]`)
- Unlinks from `InLoadOrderModuleList`, `InMemoryOrder`, and `InInitializationOrder`
- Validates all pointers before dereferencing (user-mode canonical address range)
- Idempotency guard: double-unlink returns error (self-referential list entry)
- Iteration limit: 4096 max steps to detect corruption

**Coverage:**

- Defeats loader-backed enumeration (e.g., tools that walk `PEB_LDR_DATA` first before export inspection).
- Does **not** prevent Process Hacker or direct memory scans of the module.

**Known Limits:**

- Timing: if a memory snapshot is taken _before_ PEB unlink runs, the module remains visible in that snapshot.
- Forensics: removed entries leave evidence in heap/memory dump if analyzed post-hoc.

---

## Layer 2: Memory Isolation

### Page Encryption (`textquest-dll/src/stealth/page_encrypt.rs`)

**Purpose:** Encrypt all code pages except the currently executing one, leaving only ~2% of code readable at any moment.

**Implementation:**

- **Windows VEH handler:** intercepts `EXCEPTION_ACCESS_VIOLATION` (0xC0000005)
  - Decrypts faulted page, re-encrypts previously-active page
  - Tracks active page via atomic index
- **Encryption:** XOR with unique 16-byte key per page (OS entropy via `getrandom`)
- **Memory state:** All non-active pages marked `PAGE_NOACCESS`
- **Concurrency:** LIVE_GATE and MANAGER_PTR double-check synchronization to prevent TOCTOU during cleanup

**Coverage:**

- Reduces memory-scanner exposure: scanner can only see 1 page (4KB) of plaintext code out of potentially megabytes
- Defeats static code analysis from memory dumps
- Does **not** protect against:
  - Page fault monitoring (EDR can log the fault address sequence)
  - Runtime behavioral analysis (DLL still executes)

**Known Limits:**

- Requires single-threaded access to encrypted pages (concurrent page faults cause race)
- Cleanup must synchronize with VEH removal (RemoveVectoredExceptionHandler blocks)

---

### Per-Frame Sleep Obfuscation Orchestrator (`textquest-dll/src/stealth/mod.rs`)

**Purpose:** Gargoyle-style 3-layer encryption of DLL code between game loop frames.

**Implementation:** Three-layer stack executed at frame end (sleep) and frame start (wake):

**Layer 1: Page Guard (`page_guard`)**

- `VirtualProtect` toggle: `.text` → PAGE_READWRITE (before encryption)
- After encryption: `.text` → PAGE_NOACCESS
- Functions in `.tq` section stay executable (separate section)

**Layer 2: Text Encrypt (`text_encrypt`)**

- SSE2 SIMD XOR: 16-byte key, 128-bit lanes
- Key regenerated fresh each encrypt cycle (OS entropy fallback to PRNG)
- Scalar fallback for non-SIMD architectures

**Layer 3: Stack Spoof (`stack_spoof` stubs)**

- Stub functions reserved for per-frame stack spoofing (wired after #345 HWBP hooks)
- Will overwrite return addresses on encrypted frame boundaries

**Execution Order:**

```
Frame N ends:
  1. set_writable()        → PAGE_READWRITE
  2. encrypt()             → XOR .text with fresh key
  3. prepare_spoofed_frame() → (stub, reserved)
  4. set_noaccess()        → PAGE_NOACCESS
  CODE_ENCRYPTED = true

Frame N+1 begins:
  1. restore_real_frame()  → (stub, reserved)
  2. decrypt()             → XOR .text (self-inverse)
  3. set_executable()      → PAGE_EXECUTE_READ
  CODE_ENCRYPTED = false
```

**Coverage:**

- DLL code is encrypted ~97% of the time (only 1 frame awake per cycle)
- ~2-5ms per frame = encrypted state duration
- Defeats:
  - Snapshots between frames
  - Memory scans during idle periods
  - Static code analysis from crash dumps

**Known Limits:**

- **Incompatible with IPC background threads:** sleep obfuscation init fails if IPC is running (prevents race on shared .text)
- Frame timing: encryption cycle must complete before next game loop
- Stack spoofing layer not yet active (reserved for HWBP implementation)

---

## Layer 3: Code Integrity Obfuscation

### Text Encrypt (`textquest-dll/src/stealth/text_encrypt.rs`)

**Purpose:** SSE2 SIMD XOR encryption of the DLL's `.text` section.

**Key Details:**

- Scans PE headers to locate `.text` section (base + size)
- Uses OS entropy (`getrandom`) to generate 16-byte key each cycle
- Fallback: time-based PRNG if `getrandom` fails (with warning log)
- **Single-threaded access:** game loop thread only, no synchronization needed

**SIMD Implementation:**

- x86_64: SSE2 `_mm_xor_si128` on 16-byte lanes
- ARM/other: scalar fallback (portable)
- Tail: scalar XOR for remainder bytes

**Coverage:**

- Protects against byte-pattern scanning while encrypted
- Encrypts all code uniformly (no selective key regions)

---

### Page Guard (`textquest-dll/src/stealth/page_guard.rs`)

**Purpose:** VirtualProtect state machine for `.text` during sleep/wake cycle.

**Operations:**

- `set_writable()`: `.text` → PAGE_READWRITE (before encryption)
- `set_executable()`: `.text` → PAGE_EXECUTE_READ (after decryption)

**Error Handling:**

- Logs on VirtualProtect failure (does not panic)
- Called from single-threaded game loop context

---

### Stack Spoof (`textquest-dll/src/stealth/stack_spoof.rs`)

**Purpose:** Mask return addresses on the call stack with legitimate ntdll/kernel32 gadgets.

**Implementation:**

- **Gadget cache:** scanned once per module (ntdll, kernel32) for `ret` (0xC3) instructions
  - Heuristic: preceded by 4 non-zero bytes (avoid padding)
  - Typically hundreds of gadgets per module
  - Invalid module names are rejected up front; the scanner no longer falls back to an empty C string that would make `GetModuleHandleA` resolve the calling module instead of the requested target
- **Spoof depth:** 4 frames (RBP chain walk from current frame)
- **RAII restoration:** Drop impl automatically restores original rets (panic-safe)
- **API:** `with_spoofed_stack<F>` closure-based wrapper

**Coverage:**

- Stack walks by anti-cheat see ntdll/kernel32 addresses instead of injected DLL addresses
- Defeats stack unwinding detection
- Does **not** protect:
  - RIP inspection during execution (register set still points into our DLL)
  - Advanced return-address validation (some EDR checks RIP at each frame)

**Known Limits:**

- Gadget heuristic is not fool-proof (may pick padding or misaligned rets)
- Works only on caller frame boundaries (mid-function rets not masked)
- The frame walker rewrites the current RBP chain heuristically; it does not verify that each original return address belongs to a specific trusted module before swapping in a gadget
- Stub for layer 3 of sleep cycle: `prepare_spoofed_frame()` / `restore_real_frame()` not yet active

---

## Layer 4: Memory Management Evasion

### Stealth Allocator (`textquest-dll/src/stealth/alloc.rs`)

**Purpose:** Avoid `VirtualAlloc` detection signatures by using NT heap/section APIs.

**Windows Implementation:**

**Heap Allocation:**

```
RtlAllocateHeap(heap_from_PEB, 0, size)
```

- Reads process default heap from PEB (offset 0x30 on x64)
- Uses `gs:[0x60]` (PEB pointer) + offset to resolve heap
- Fallback to system heap if resolution fails

**Section Allocation:**

```
NtCreateSection(pagefile-backed)
↓
NtMapViewOfSection(into current process)
```

- Creates anonymous section (no file backing)
- Maps with SECTION_ALL_ACCESS, VIEW_UNMAP flag
- Closes section handle (mapping persists)

**Non-Windows Stub:**

- Falls back to `std::alloc` with size header for freeing

**Coverage:**

- Common detection signature: VirtualAlloc in ETW/WMI logs
- These APIs are normal for system processes → less suspicious

**Known Limits:**

- Heap allocations still appear in heap snapshots
- Section mapping still shows in VirtualAlloc calls (just through a different path)
- No protection against EDR hooking of RtlAllocateHeap

---

### Section Remap (`textquest-dll/src/stealth/section_remap.rs`)

**Purpose:** Trigger copy-on-write (COW) to convert loaded DLL pages from MEM_IMAGE → MEM_PRIVATE.

**Implementation:**

1. Walk all pages in DLL (via VirtualQuery)
2. For each MEM_IMAGE page:
   - Allocate temp buffer
   - VirtualProtect to RW
   - Copy to temp
   - VirtualProtect to R/RX
   - Copy back from temp to original
   - VirtualFree temp buffer

**Effect:** Forces Windows to replace loader-mapped pages with private copies, breaking memory-map signatures.

**Coverage:**

- Defeats detection based on MEM_IMAGE type detection
- Private pages blend with normal heap activity

**Known Limits:**

- Timing: must run early in DLL init before module caches its own image
- Expensive: O(code_size) multiple copies
- Does not hide the DLL itself, only its memory type

---

### Trampoline Hardening (`textquest-dll/src/stealth/trampoline.rs`)

**Purpose:** Protect detour/trampoline regions (return-oriented gadgets) from inspection.

**Implementation:**

- Registry (HashMap): tracks `(addr, (size, is_concealed))` per trampoline
- **Concealment:** XOR with key 0xA5 when not executing
- **Protection:** VirtualProtect to PAGE_EXECUTE_READ (RX after setup)
- **Reveal/conceal:** toggle concealment state in registry

**API:**

```rust
hardener.register(addr, size);
hardener.conceal(addr, size);
hardener.reveal(addr, size);
hardener.protect(addr, size);  // Windows only
```

**Coverage:**

- XOR concealment hides trampoline bytes from memory scans
- RX protection prevents modification after setup
- Registry queries support live checking of concealment state

---

### Thread Pool Injection (`textquest-dll/src/stealth/thread_pool.rs`)

**Purpose:** Execute callbacks on OS-managed worker threads instead of creating suspicious new threads.

**Pattern:** SafeBreach PoolParty (8 variants, using variant 1: simple work item)

**Implementation:**

```rust
CreateThreadpoolWork(callback, context, nullptr)
↓
SubmitThreadpoolWork(work)
↓
CloseThreadpoolWork(work)  // work still runs, just unreferenced
```

**Coverage:**

- No `CreateThread` or `CreateRemoteThread` events in ETW
- Worker thread identity hidden (OS-managed pool)
- Callback runs indistinguishably from normal app activity

**Known Limits:**

- Callback must be thread-safe (no loader-lock dependencies)
- Execution latency depends on OS pool congestion
- Must not hold high-level locks during callback

---

## Layer 5: Syscall Evasion

### RecycledGate + TartarusGate (`textquest-dll/src/syscall/`)

**Purpose:** Invoke NT APIs through a `syscall;ret` gadget in the real ntdll, making syscall stacks appear legitimate.

**Architecture:**

**1. Hash Layer (`hash.rs`)**

- DJB2 hashing of API names (e.g., "NtProtectVirtualMemory")
- No plaintext strings in binary (defeats import scanning)

**2. Table Layer (`table.rs`)**

- **TartarusGate:** Map fresh ntdll from KnownDlls
  - Walk PE exports, extract SSNs (system service numbers)
  - Pattern match Zw/Nt stubs: `mov r10, rcx` → `mov eax, SSN` → ...
  - Locate `syscall;ret` gadget in real ntdll (pattern: 0x0F 0x05 0xC3)
  - Unmap fresh copy immediately (leaves no suspicious section behind)
- **SyscallTable:** stores (hash, SSN, gadget_addr) triples

**3. Gate Layer (`gate.rs`)**

- Assembly stubs: load SSN into EAX, copy first arg to R10 (syscall ABI)
- JMP to gadget address (inside real ntdll)
- `ret` from gadget returns to our caller, stack appears clean

**Supported Syscalls:**

- `NtProtectVirtualMemory` (memory protection)
- `NtAllocateVirtualMemory` (memory allocation)
- `NtSetContextThread` (register modification)
- `NtGetContextThread` (register inspection)

**Coverage:**

- Stack walks see return addresses inside ntdll, not our DLL
- Defeats:
  - Call stack origin detection (stack RIPs legitimate)
  - Syscall hooking in ntdll import table
  - Syscall count anomalies
- Does **not** protect against:
  - Parameter inspection
  - Syscall sequence analysis

**Known Limits:**

- Only 4 target syscalls implemented
- Gadget scanning is one-time at init (if no gadget found, falls back to normal `syscall`)
- Windows-only (macOS stubs return STATUS_SUCCESS)

---

## Layer 6: EQ Protocol Integration

### HWBP Engine (`textquest-dll/src/hooks/hwbp.rs`)

**Purpose:** Zero-modification hooking engine using x86_64 debug registers.

**Architecture:**

- **Debug registers:** DR0-DR3 (4 execution breakpoint slots)
- **Vectored Exception Handler:** intercepts EXCEPTION_SINGLE_STEP
- **Callback dispatch:** maps slot → callback function pointer
- **No code modifications:** original function bytes untouched (invisible to integrity checks)

**Usage:**

```rust
register_hook(addr, slot, callback)?  // arm breakpoint
unregister_hook(slot)?                 // disarm
is_active(slot)?                       // query state
```

**Limitations:**

- MAX_SLOTS = 4 (only 4 simultaneous hooks)
- Execution breakpoints fire on every instruction execution at target address
- Callback must be fast (runs in exception context)

**Typical Hooks:**

- SystemFingerprint interception (fingerprint spoofing)
- Integrity validation checks
- EQ protocol inspection points

---

### Fingerprint Spoofing (`textquest-dll/src/hooks/fingerprint.rs`)

**Purpose:** Intercept `SystemFingerprint` (0x140594840) to return unique per-client hardware IDs.

**Problem:** 36 multibox clients send identical hardware IDs → instant detection.

**Solution:** Derive unique, deterministic IDs from session token:

**Fields:**

1. **VideoCardId:** PCI vendor:device format (e.g., "PCI\VEN_10DE&DEV_2684")
2. **NetworkCardId:** MAC address format (e.g., "00-1A-2B-3C-4D-5E")
3. **HardriveId:** Volume serial (e.g., "A1B2-C3D4")
4. **ComputerName:** Windows hostname format (e.g., "DESKTOP-A1B2C3D")

**Implementation:**

- **Domain-separated hashing:** FNV-1a 128-bit with per-field domain prefixes
- **Deterministic:** same token → same fingerprints across client restarts
- **Plausible:** outputs look like real hardware IDs (format-compliant)

**Initialization:**

```rust
fingerprint::init(session_token)  // called during DLL init
```

**Coverage:**

- Each client gets unique fingerprint (multibox now passes this check)
- Persistent across reconnects for same account slot
- Does **not** protect against:
  - Reverse lookups (anti-cheat may recognize spoofed format)
  - Temporal correlation (same client always has same fingerprint)

---

### Hook Integrity Self-Check (`textquest-dll/src/hooks/integrity.rs`)

**Purpose:** Validate HWBP slot consistency before accepting IPC commands.

**Check Logic:**

A slot is **consistent** when:

- **If active:** `address != 0` AND `callback != null`
- **If inactive:** `address == 0` AND `callback == null`

A slot is **corrupted** when:

- Active with zero address (VEH would JMP to 0x00000000)
- Active with null callback (would call function pointer 0)
- Inactive with residual address or callback (cleanup failed)

**Failure Response:**

- Sets global `SAFE_MODE` flag
- IPC layer rejects all commands while `SAFE_MODE` is true
- Prevents cascading failures from corrupted hooks

**Timing:**

- Runs after all hooks are installed, before IPC listener activates
- Post-injection initialization sequence

**Coverage:**

- Early detection of hook setup failures
- Prevents silent execution of corrupted hooks
- Operator visibility: IPC failure indicates hook problem

---

## Reflective Injection (`textquest/src/inject/reflective.rs`)

**Purpose:** Load textquest-dll.dll into running eqgame.exe without calling `LoadLibraryW`.

**Implementation:**

- **Manual PE mapping:** reads PE headers, allocates sections, applies relocations
- **No loader involvement:** bypasses `LoadLibraryW` call tracing
- **TLS handling:** initializes thread-local storage for DLL callbacks
- **Entry point:** calls DLL's `DllMain(DLL_PROCESS_ATTACH, ...)`

**Coverage:**

- Defeats detection based on `LoadLibraryW` API call tracing
- DLL still appears in module list (PEB unlink removes it post-injection)
- No suspicious string in loaded modules before PEB unlink

**Known Limits:**

- PE parsing must be robust (malformed headers can crash)
- Relocation fixups must match Windows loader semantics
- Import table resolution can expose API calls (mitigated by syscall layer)

---

## Initialization Sequence

At DLL load, the following sequence executes (from `lib.rs` entrypoint):

1. **Logging setup** → tracing subscriber initialization
2. **Stealth activation** (conditional on config):
   - `peb_unlink::unlink_module(dll_base)` → remove from PEB lists
   - `pe_erase::erase_pe_headers(dll_base)` → zero headers
   - `syscall::init()` → build syscall table
   - `stack_spoof::init()` → cache gadgets from ntdll/kernel32
   - `etw_blind::init()` → set DR0 on NtTraceEvent
   - `page_encrypt::init()` → set up VEH for page encryption
   - `section_remap::remap_sections()` → trigger COW for MEM_PRIVATE
3. **Hook installation** (via `hwbp` engine):
   - `fingerprint::init(session_token)` → initialize spoofed values
   - Register HWBP callbacks for SystemFingerprint
4. **Integrity check:**
   - `integrity::check_all_slots()` → validate HWBP state
   - Set SAFE_MODE if any slot is corrupted
5. **IPC server start:**
   - Listen for named pipe commands
   - Reject all commands if SAFE_MODE is true
6. **Game loop hook:**
   - Install detours on game main loop
   - Per-frame: wake → decrypt code → execute → encrypt code → sleep

---

## Known Gaps and Limitations

### Server-Initiated Detection

**Gap:** Page encryption + text encryption defend against client-side memory scanning, but **not** against server-requested integrity responses.

**Example:** EQ sends opcode 0x4f27 (memcheck) requesting hash of specific memory region. DLL decrypts region, computes hash, sends response — but decryption happens during the active frame when server is watching.

**Status:** M7 validation work needed (see Roadmap-and-Known-Gaps.md).

### Sleep Orchestration with IPC

**Gap:** `stealth::sleep()` is incompatible with active IPC background threads (prevents encryption race).

**Current workaround:** IPC threads must be paused before sleep activation.

**Status:** M8 orchestrator work.

### Stack Spoof Layer 3

**Gap:** Per-frame stack spoofing (`prepare_spoofed_frame` / `restore_real_frame`) stubs not yet wired.

**Status:** Reserved after #345 HWBP implementation.

### Fingerprint Temporal Correlation

**Gap:** Same multibox client always has same fingerprint (deterministic per session token).

**Risk:** Anti-cheat can correlate accounts by fingerprint persistence across restarts.

**Status:** Research track M7.

### Syscall Coverage

**Gap:** Only 4 syscalls implemented. Other NT APIs fall back to normal calls.

**Coverage:** NtProtectVirtualMemory, NtAllocateVirtualMemory, NtSetContextThread, NtGetContextThread.

**Status:** Prioritize based on M5 validation (which syscalls are actually monitored).

---

## Cross-References

- **Public docs:** `docs/wiki/Security-and-Anti-Detection-Notes.md` (operator-facing, partial mermaid)
- **Research:** `docs/wiki/Research-Anti-Detection.md` (EQ binary analysis, Ghidra findings)
- **Offsets:** `textquest-common/src/offsets.rs` lines 353-384 (anti-cheat/integrity syscalls)
- **Roadmap:** `docs/wiki/Roadmap-and-Known-Gaps.md` (M5/M7/M8 milestones)
- **Issues:** GitHub Issues #723, #728, #730, #722, #934-#936 (related gaps)

---

## Development Notes

### When Implementing New Hooks

1. **Check MAX_SLOTS limit:** HWBP engine has only 4 slots
2. **Verify callback safety:** runs in VEH context (no heap alloc, no syscalls unless via gate)
3. **Run integrity check:** call `integrity::check_all_slots()` after hook installation
4. **Test with disabled IPC:** some stealth features conflict with background threads

### When Modifying Sleep Obfuscation

1. **Order matters:** wake/sleep must call in strict order (page_guard → text_encrypt → stack_spoof)
2. **Timing budget:** all three layers must complete in <5ms per frame
3. **Test on Windows:** stubs on macOS; only Windows impl is functional
4. **Sleep-cycle lock stays in `.tq`:** `wake()`/`sleep()` serialize on a spin-lock guarded by `SleepCycleGuard`. The guard's `acquire`/`drop` are both annotated `#[link_section = ".tq"]` so the lock path remains executable while `.text` is encrypted. The RAII guard also keeps the critical section panic-safe — if any helper (`restore_real_frame`, `encrypt`, `decrypt`) unwinds, `SLEEP_CYCLE_LOCKED` is cleared on drop instead of stranding at `true`.
5. **Spin-lock ordering:** compare-exchange uses `Acquire`/`Relaxed` (success/failure). The `Release` store on drop pairs with the next acquirer's `Acquire`. A `std::hint::spin_loop()` sits in the retry path to avoid burning a core under contention between the timer-queue thread and the frame thread.

### When Adding Syscalls to RecycledGate

1. **Add hash constant** in `syscall/hash.rs` (DJB2 of API name)
2. **Add to TARGET_HASHES** in `syscall/mod.rs` initialization
3. **Create gate stub** in `syscall/gate.rs` (assembly pattern)
4. **Test gadget scanning** — if no gadget found, syscall fails (test error handling)

---

## Fix #2172

Stack-spoof gadget scanner now correctly searches the target module (not the caller module) via fixed CString handling.
