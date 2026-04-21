# Deep Technical: DLL Injection, Memory Control, and VEH

This document provides comprehensive technical coverage of how TextQuest injects into EverQuest, controls characters through memory manipulation, and uses Vectored Exception Handlers (VEH). It serves as canonical documentation for understanding the entire injection and control pipeline.

---

## Table of Contents

1. [Injection Pipeline](#injection-pipeline)
2. [Memory Layout After Injection](#memory-layout-after-injection)
3. [Character Control Mechanism](#character-control-mechanism)
4. [VEH Implementation](#veh-implementation)
5. [Memory Reading and Writing](#memory-reading-and-writing)
6. [Server-Side Detection](#server-side-detection)
7. [Code References](#code-references)

---

## Injection Pipeline

### Overview

```
textquest.exe (Orchestrator)
        │
        │ 1. Build textquest.dll
        │ 2. Stage to temp with random name
        │ 3. Generate 32-byte session token
        ▼
CreateRemoteThread + LoadLibraryW
        │
        ▼
textquest.dll (Inside eqgame.exe)
        │
        │ 1. Read session token
        │ 2. Derive session ID
        │ 3. Initialize IPC
        │ 4. Run stealth init
        ▼
Active - provides control surface
```

### Step-by-Step Injection

**Location:** `textquest/src/inject/loader.rs`

```rust
// Step 1: Open target process with required permissions
let process = OpenProcess(
    PROCESS_CREATE_THREAD  // for CreateRemoteThread
    | PROCESS_VM_OPERATION   // for VirtualAllocEx/WriteProcessMemory
    | PROCESS_VM_WRITE    // for WriteProcessMemory
    | PROCESS_VM_READ     // for reading memory
    | PROCESS_QUERY_INFORMATION, // for GetExitCodeThread
    false, pid
)?;
```

**Step 2: Allocate memory in target process**
```rust
let remote_buf = VirtualAllocEx(
    process,
    Some(std::ptr::null()),  // let OS choose address
    dll_path_bytes,         // size for DLL path string
    MEM_COMMIT | MEM_RESERVE,
    PAGE_READWRITE         // writable initially
)?;
```

**Step 3: Write DLL path to target memory**
```rust
WriteProcessMemory(
    process,
    remote_buf,
    dll_path_wide.as_ptr(),  // UTF-16 path string
    dll_path_bytes,
    None
)?;
```

**Step 4: Create remote thread calling LoadLibraryW**
```rust
CreateRemoteThread(
    process,
    None,  // default security attributes
    0,     // default stack size
    Some(load_library_fn),  // LoadLibraryW address
    Some(remote_buf),       // argument = DLL path in target
    0,                      // CREATE_SUSPENDED not set
    None
)?;
```

**Step 5: Wait for completion and verify success**
```rust
WaitForSingleObject(thread, INJECTION_TIMEOUT_MS);
// thread_exit_code is the HMODULE returned by LoadLibraryW
// Non-zero = success
if thread_exit_code == 0 {
    bail!("LoadLibraryW returned NULL");
}
```

### Key Concept: Why Use LoadLibraryW?

The classic `CreateRemoteThread + LoadLibraryW` technique:

1. **No code execution in our process** — The target process executes LoadLibrary
2. **Automatic DLL initialization** — DllMain runs automatically
3. **OS-managed thread** — Windows handles thread cleanup
4. **Dependency resolution** — Windows loads required DLLs

**Detection Risk:** `LoadLibraryW` appears in the injection's call stack. Advanced scanners could detect this pattern.

---

## Memory Layout After Injection

### Process Memory Map

```
eqgame.exe Process Memory
├───────────────────────────────────────────────────────────────────┐
│ Code Sections (.text, .rdata)                                    │
│   └── Original EQ game code (read-only when possible)              │
├─────────────────────────────────────────────────────────────────┤
│ Data Sections (.data, .bss)                                     │
│   └── Game globals, player state, spawn list                    │
├─────────────────────────────────────────────────────────────────┤
│ Stack (main thread + worker threads)                             │
│   └── EQ call stack + our hook stacks                          │
├─────────────────────────────────────────────────────────────────┤
│ Heap                                                           │
│   └── EQ allocations + our stealth allocations               │
├─────────────────────────────────────────────────────────────────┤
│ textquest.dll (INJECTED)                                       │
│   ├── .text section (page encrypted after init)               │
│   ├── .data section                                           │
│   ├── Hooked functions (trampolines)                          │
│   └── IPC buffers                                            │
├─────────────────────────────────────────────────────────────────┤
│ Shared Memory (our region)                                      │
│   └── GameState snapshots (published by DLL)                  │
├─────────────────────────────────────────────────────────────────┤
│ Named Pipe (IPC)                                              │
│   └── Command/response channel                               │
└─────────────────────────────────────────────────────────────────┘
```

### Injected DLL Memory Layout

```
textquest.dll Loaded Image
├── PE Headers (erased after init via PE erase)
│   └── DOS stub, NT headers, section headers → zeros
├── .text section (initially clear, then encrypted)
│   ├── DllMain entry
│   ├── Hook implementations (detour trampolines)
│   ├── VEH handlers ( hwbp, etw_blind, page_encrypt)
│   └── IPC server
├── .data section
│   ├── Session token
│   ├── IPC buffers
│   └── State tracking
├── .rdata (read-only data)
│   ├── String constants
│   └── Function pointers
└── Hooks (detoured functions)
    ├── InterpretCmd hook (command injection)
    ├── WSASend/WSARecv hooks (packet capture)
    ├── CastSpell callback
    └── Spawn tracking
```

---

## Character Control Mechanism

### How We Control Characters

**Overview:** We don't directly "control" characters. We inject commands into the game's command parser and interact with the UI through memory.

```
Orchestrator ──IPC (pipe+shmem)──► DLL ──► Hooked Functions ──► EQ Engine
                                              │
                                         InterpretCmd
                                              │
                                         EQ Parse
                                              │
                                         Command Execute
```

### Control Methods

#### 1. Command Injection via InterpretCmd

**Location:** `textquest-dll/src/hooks/chat.rs`

EQ's `InterpretCmd` function parses and executes slash commands. We hook this function:

```rust
// When EQ calls dsp_chat (display chat), our VEH handler:
// 1. Captures the chat message text
// 2. Optionally modifies it
// 3. Returns control to EQ
```

Then we can inject commands:
```rust
// From orchestrator: send "/sit" command via IPC
// In DLL: InterpretCmd receives the command
// EQ executes: character sits
```

#### 2. Direct Memory Manipulation

**Location:** `textquest-dll/src/eq/`

We read game memory to understand state:
```rust
// Read spawn list pointer from known offset
let spawn_list: *const Spawn = read_memory(
    EQ_BASE +offsets::SPAWN_LIST,
    size_of::<Spawn>()
)?;

// Iterate spawns
for i in 0..spawn_list.count {
    let spawn = read_memory(spawn_list.entries[i])?;
    // Process spawn data
}
```

#### 3. Spell/Cast Injection

**Location:** `textquest-dll/src/cast.rs`

```rust
// Inject cast command:
// 1. Set target ID in memory
// 2. Trigger cast through memory addresses
// 3. Monitor cast state via hooks
```

### IPC Protocol

**Location:** `textquest-common/src/ipc.rs`

```
Command Message (Orchestrator → DLL)
├── command_id: u32
├── args: Vec<u8>
└── response_channel: u64

Response Message (DLL → Orchestrator)  
├── request_id: u32
├── status: StatusCode
└── payload: Vec<u8>
```

---

## VEH Implementation

### What is VEH?

**Vectored Exception Handler (VEH)** is a Windows process-wide exception handling mechanism that runs before thread-local SEH.

**Why VEH?**
- Process-wide (not per-thread)
- Runs before SEH
- Can catch hardware breakpoints (single-step)
- Can intercept any exception

### Our Three VEH Handlers

#### 1. Hardware Breakpoint (HWBP) Dispatcher

**Location:** `textquest-dll/src/hooks/hwbp.rs`

**Purpose:** Route hardware breakpoint exceptions to our callbacks

```rust
// VEH handler for HWBP dispatch
unsafe fn veh_handler(exception_info: *mut EXCEPTION_POINTERS) -> EXCEPTION_DISPOSITION {
    let record = exception_info.as_ref()
        .context_record;
    
    // Check if SINGLE_STEP (exception we caused)
    if record.ExceptionCode == EXCEPTION_SINGLE_STEP {
        // Dispatch to registered HWBP callbacks
        return dispatch_hwbp(exception_info);
    }
    
    // Not ours - continue to next handler
    EXCEPTION_CONTINUE_SEARCH
}
```

**How HWBP hooks work:**
```
1. Set DR0-DR3 hardware breakpoints on target addresses
2. Enable Trace Flag (TF) in EFLAGS
3. When EQ calls hooked function → CPU triggers SINGLE_STEP
4. VEH catches it → our callback runs → return to EQ
```

#### 2. ETW Blinding

**Location:** `textquest-dll/src/stealth/etw_blind.rs`

**Purpose:** Suppress Event Tracing for Windows (ETW) from seeing our hooks

```rust
// VEH that intercepts NtTraceEvent calls
// Returns STATUS_SUCCESS (RAX = 0) to suppress logging
unsafe fn etw_veh_handler(info: *mut EXCEPTION_POINTERS) -> EXCEPTION_DISPOSITION {
    let record = info.as_ref();
    
    if record.ExceptionCode == STATUS_SUCCESS {
        // This is our NtTraceEvent return
        // Return success to suppress ETW logging
        return EXCEPTION_CONTINUE_EXECUTION;
    }
    
    EXCEPTION_CONTINUE_SEARCH
}
```

#### 3. Page Encryption Handler

**Location:** `textquest-dll/src/stealth/page_encrypt.rs`

**Purpose:** Decrypt code pages on-demand to prevent memory scanning

```rust
// VEH that handles page faults for encrypted pages
// 1. Detect access violation on encrypted page
// 2. Decrypt the page
// 3. Resume execution (page now readable)
// 4. Re-encrypt after callback completes
```

### Installing VEH

**Location:** `textquest-dll/src/hooks/hwbp.rs`

```rust
// Installation
let handle = AddVectoredExceptionHandler(
    1,  // First in chain (higher priority)
    Some(veh_handler)  // Our handler function
)?;

VEH_HANDLE.store(handle as usize, Ordering::Release);

// Cleanup
let handle = VEH_HANDLE.swap(0, Ordering::AcqRel);
RemoveVectoredExceptionHandler(handle as *mut _);
```

---

## Memory Reading and Writing

### Reading Game Memory

**Location:** `textquest-dll/src/eq/structs.rs`

```rust
// Basic read with validation
pub fn read_spawn(pid: u32) -> Result<Spawn, ReadError> {
    // 1. Validate pointer not null
    if ptr.is_null() { return Err(ReadError::NullPointer); }
    
    // 2. Validate in readable memory
    if !is_readable(ptr) { return Err(ReadError::Inaccessible); }
    
    // 3. Check sentinel/magic values
    if ptr.magic != SPAWN_MAGIC { return Err(ReadError::BadMagic); }
    
    // 4. Read the structure
    Ok(ptr read())
}
```

### Structure Example: PlayerClient

**Location:** `textquest-common/src/offsets.rs`

```rust
// EQ's PlayerClient structure (from offsets analysis)
#[repr(C)]
pub struct PlayerClient {
    pub vtable: *const (),           // vtable pointer
    pub name: [u8; 64],           // character name
    pub level: u8,                  // character level
    pub class: Class,                // class enum
    pub race: Race,                 // race enum
    pub hp: i32,                  // current HP
    pub max_hp: i32,             // max HP
    pub mana: i32,               // current mana
    pub max_mana: i32,            // max mana
    pub stamina: i32,             // endurance
    pub x: f32,                // position X
    pub y: f32,                // position Y  
    pub z: f32,                // position Z
    pub heading: f32,            // heading
    pub target: u32,            // target spawn ID
    pub group: u32,              // group/raid flags
    // ... more fields
}
```

### Writing to Memory

**Location:** `textquest-dll/src/eq/memory.rs`

```rust
// Write with safety checks
pub fn write_target(spawn_id: u32) -> Result<(), WriteError> {
    // Locate target pointer via spawn list
    let target_ptr = find_spawn(spawn_id)?;
    
    // Validate writable
    if !is_writable(target_ptr) { 
        return Err(WriteError::Protected); 
    }
    
    // Write the new target ID
    WriteProcessMemory(
        process,
        target_ptr.offset(TARGET_OFFSET),
        &spawn_id,
        size_of::<u32>(),
        None
    )?;
    
    Ok(())
}
```

---

## Server-Side Detection

### What Can the Server Detect?

| Detection Vector | What They See | Our Countermeasure |
|-----------------|---------------|------------------|
| **Packet Timing** | Unusual patterns | Natural timing, rate limiting |
| **Memory Scanning** | RWX pages, hooks | Page encryption, PEB unlink |
| **Thread Count** | Extra threads | Use thread pool, not CreateThread |
| **Import Table** | LoadLibrary calls | Reflective loading (future) |
| **Exception Patterns** | VEH installed | Legitimate VEH usage (normal) |
| **Hardware Breakpoints** | DR0-DR7 reads | Could detect (use carefully) |
| **Packet Content** | Modified packets | Authenticated encryption (future) |
| **Timing Analysis** | Consistent timing | Add jitter, humanization |

### Server Detection Limitations

1. **No direct memory access** — Server runs on different machine
2. **Packet analysis only** — Can analyze what we send
3. **Timing patterns** — Statistical analysis of packet intervals
4. **Client integrity checks** — Hash verification of client memory

### Our Stealth Features

| Feature | Location | Purpose |
|---------|----------|---------|
| **PEB Unlinking** | `stealth/peb_unlink.rs` | Hide from module enumeration |
| **Page Encryption** | `stealth/page_encrypt.rs` | Prevent code scanning |
| **PE Header Erase** | `stealth/pe_erase.rs` | Remove PE headers |
| **ETW Blinding** | `stealth/etw_blind.rs` | Suppress event logging |
| **Stack Spoofing** | `stealth/stack_spoof.rs` | Fake return addresses |
| **Syscall Layer** | `stealth/syscall/*.rs` | Indirect syscalls |

---

## Code References

### Injection

| File | Lines | Description |
|------|-------|-------------|
| `textquest/src/inject/loader.rs` | 158-349 | Main injection code |
| `textquest/src/inject/mod.rs` | 1-10 | Module exports |
| `textquest/src/inject/dll_prep.rs` | all | DLL staging |
| `textquest/src/inject/reflective.rs` | all | Reflective loader |

### VEH and Hooks

| File | Lines | Description |
|------|-------|-------------|
| `textquest-dll/src/hooks/hwbp.rs` | 1-100 | HWBP dispatcher |
| `textquest-dll/src/hooks/hwbp.rs` | 275-300 | VEH install |
| `textquest-dll/src/stealth/etw_blind.rs` | all | ETW VEH |
| `textquest-dll/src/stealth/page_encrypt.rs` | 180-230 | Page encrypt VEH |
| `textquest-dll/src/stealth/peb_unlink.rs` | all | PEB hiding |

### IPC

| File | Lines | Description |
|------|-------|-------------|
| `textquest-dll/src/ipc/server.rs` | all | IPC server |
| `textquest/src/ipc/client.rs` | all | IPC client |
| `textquest-common/src/ipc.rs` | all | Protocol definitions |

### Memory Reading

| File | Lines | Description |
|------|-------|-------------|
| `textquest-dll/src/eq/structs.rs` | all | EQ structures |
| `textquest-dll/src/eq/memory.rs` | all | Memory operations |
| `textquest-common/src/offsets.rs` | all | Offset definitions |

---

## Handoff Notes

### For a New Developer

1. **Start with injection** — Read `loader.rs` to understand how DLL gets into process
2. **VEH is the key** — `hwbp.rs` shows how hooks dispatch via VEH
3. **Memory safety first** — Always validate before reading/writing
4. **Test incrementally** — Use TUI packet filter to validate capture works

### Key Files to Know

| System | File |
|--------|------|
| Injection | `textquest/src/inject/loader.rs` |
| VEH Hooks | `textquest-dll/src/hooks/hwbp.rs` |
| Memory | `textquest-dll/src/eq/structs.rs` |
| IPC | `textquest-dll/src/ipc/server.rs` |
| Stealth | `textquest-dll/src/stealth/*.rs` |

### Testing Commands

```bash
# Test packet capture
textquest tui → press 5 for Packets screen

# Test injection
textquest inject --pid <eqgame_pid>

# Debug logging
TEXTQUEST_DEBUG=1 textquest tui
```

---

## Related Documentation

- [DLL Injection and IPC Pipeline](DLL-Injection-and-IPC-Pipeline.md)
- [Security and Anti-Detection Notes](Security-and-Anti-Detection-Notes.md)
- [Offsets, EQ Internals, and MacroQuest References](Offsets-EQ-Internals-and-MacroQuest-References.md)
- [Architecture Overview](Architecture-Overview.md)
