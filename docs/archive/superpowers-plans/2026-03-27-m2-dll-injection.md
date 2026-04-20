# M2: DLL Injection + Internal Function Hooking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Inject a Rust-built DLL (`cdylib`) into eqgame.exe to hook internal EQ functions for direct control -- movement, casting, targeting -- enabling multibox orchestration of up to 36 clients on a TLP server.

**Architecture:** Single TextQuest binary handles injection, orchestration, and TUI. A separate `cdylib` crate produces the injected DLL payload. Classic `CreateRemoteThread` + `LoadLibraryW` injection with randomized DLL names. IPC between the orchestrator and injected DLLs uses shared memory (high-frequency game state) and named pipes (commands). Self-healing monitors for EQ crashes and auto-restarts + re-injects.

**Tech Stack:** Rust workspace, `windows` crate (Win32 APIs), `retour` crate (function detouring), `shared_memory` crate (cross-process shared memory), existing M1 memory reading code.

**Supersedes:** `docs/superpowers/plans/2026-03-24-m2-input-dispatch.md` (PostMessage approach -- replaced by DLL injection for direct function access).

---

## Cargo Workspace Structure

```
Cargo.toml              (workspace root)
dmft/                   (bin) -- orchestrator + TUI + injector
  Cargo.toml
  src/
    main.rs             (moved from src/main.rs)
    config.rs           (moved from src/config.rs)
    process/            (moved from src/process/)
    eq/                 (moved from src/eq/)
    tui/                (moved from src/tui/)
    inject/
      mod.rs            -- injection orchestration
      loader.rs         -- CreateRemoteThread + LoadLibraryW injection
      dll_prep.rs       -- DLL copy with randomized name
    client/
      mod.rs            -- multi-client management
      manager.rs        -- ClientManager: track all injected EQ processes
      session.rs        -- EqSession: one EQ client (PID, injection state, IPC handles)
      healing.rs        -- self-healing: crash detection, restart, re-injection
    ipc/
      mod.rs            -- IPC orchestrator-side
      pipe.rs           -- named pipe client (send commands)
      shared.rs         -- shared memory reader (read game state)

textquest-dll/               (cdylib) -- injected DLL payload
  Cargo.toml
  src/
    lib.rs              -- DllMain entry point, initialization
    hooks/
      mod.rs            -- hook registration and management
      game_loop.rs      -- game loop tick hook
      movement.rs       -- movement function hooks
      casting.rs        -- spell casting function hooks
      targeting.rs      -- targeting function hooks
    ipc/
      mod.rs            -- IPC DLL-side
      pipe.rs           -- named pipe server (receive commands)
      shared.rs         -- shared memory writer (publish game state)
    eq/
      mod.rs            -- EQ internal function addresses + signatures

textquest-common/            (lib) -- shared types + IPC protocol
  Cargo.toml
  src/
    lib.rs
    ipc.rs              -- IPC message types, shared memory layout
    types.rs            -- GameState, Command, HookStatus, ClientId
    offsets.rs           -- shared offset constants (moved from eq/offsets.rs)
    protocol.rs         -- named pipe message framing (length-prefixed bincode)
```

---

## Task 1: Convert to Cargo Workspace

**Goal:** Restructure the existing single-crate project into a Cargo workspace. Move all existing code into the `dmft/` subcrate. Everything must compile and run identically after this step.

**Files to create:**

- `Cargo.toml` (workspace root -- replaces existing)
- `dmft/Cargo.toml` (new -- package definition for the binary crate)

**Files to move:**

- `src/**` -> `textquest/src/**`
- `config/` stays at workspace root (shared config files)

### Steps

- [ ] **Step 1.1: Create workspace root `Cargo.toml`**

Back up the existing `Cargo.toml`. Create a new workspace root:

```toml
# Cargo.toml (workspace root)
[workspace]
members = ["dmft", "textquest-dll", "textquest-common"]
resolver = "2"
```

- [ ] **Step 1.2: Create `dmft/Cargo.toml`**

```toml
[package]
name = "dmft"
version = "0.1.0"
edition = "2024"
description = "Frostreaver orchestrator + TUI + injector"

[[bin]]
name = "dmft"
path = "src/main.rs"

[dependencies]
textquest-common = { path = "../textquest-common" }
serde = { version = "1", features = ["derive"] }
toml = "0.8"
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1"
ratatui = "0.29"
crossterm = "0.28"
rand = "0.8"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.54", features = [
    "Win32_Foundation",
    "Win32_System_Diagnostics_Debug",
    "Win32_System_Threading",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_ProcessStatus",
    "Win32_System_Memory",
    "Win32_System_LibraryLoader",
] }
```

- [ ] **Step 1.3: Move source files**

```bash
mkdir -p textquest/src
mv src/* textquest/src/
rmdir src
```

- [ ] **Step 1.4: Create placeholder `textquest-common/` and `textquest-dll/` so workspace resolves**

Create minimal `textquest-common/Cargo.toml` and `textquest-common/src/lib.rs` (empty lib).
Create minimal `textquest-dll/Cargo.toml` and `textquest-dll/src/lib.rs` (empty lib).

```toml
# textquest-common/Cargo.toml
[package]
name = "textquest-common"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = { version = "1", features = ["derive"] }
bincode = "1"
```

```toml
# textquest-dll/Cargo.toml
[package]
name = "textquest-dll"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
textquest-common = { path = "../textquest-common" }
retour = "0.3"
tracing = "0.1"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.54", features = [
    "Win32_Foundation",
    "Win32_System_SystemServices",
    "Win32_System_LibraryLoader",
    "Win32_System_Memory",
    "Win32_System_Threading",
    "Win32_System_Pipes",
] }
```

- [ ] **Step 1.5: Update `textquest/src/main.rs` imports**

The `eq::offsets` module still lives in `textquest/src/eq/offsets.rs` for now (will be migrated to `textquest-common` in Task 2). Ensure `textquest/src/main.rs` has `use textquest_common;` (even if unused initially) to validate the dependency.

- [ ] **Step 1.6: Update `.gitignore`**

Ensure `/target/` covers workspace-level target directory. Update `Cargo.lock` path if needed (it stays at workspace root automatically).

- [ ] **Step 1.7: Verification**

```bash
cargo build 2>&1          # must compile all three crates
cargo run -p dmft 2>&1     # must show TUI / demo mode as before
cargo run -p dmft -- --dump 2>&1  # must work as before
```

---

## Task 2: Create `textquest-common` Crate (Shared Types + IPC Protocol)

**Goal:** Define all types shared between the orchestrator and the injected DLL: game state structures, IPC message definitions, command enums, and shared memory layout.

**Files to create:**

- `textquest-common/src/lib.rs`
- `textquest-common/src/types.rs`
- `textquest-common/src/ipc.rs`
- `textquest-common/src/offsets.rs`
- `textquest-common/src/protocol.rs`

### Steps

- [ ] **Step 2.1: Define core types in `textquest-common/src/types.rs`**

```rust
use serde::{Serialize, Deserialize};

/// Unique identifier for an EQ client (wraps PID).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClientId(pub u32);

/// Position in the EQ world.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub heading: f32,
}

/// Snapshot of game state published by the DLL via shared memory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GameState {
    pub tick: u64,
    pub player_name: [u8; 64],
    pub player_position: Position,
    pub player_hp_current: i64,
    pub player_hp_max: i64,
    pub player_mana_current: i32,
    pub player_mana_max: i32,
    pub player_level: u8,
    pub player_class: u8,
    pub target_id: u32,
    pub target_name: [u8; 64],
    pub target_hp_pct: u8,
    pub zone_id: u32,
    pub hook_status: HookStatus,
}

/// Status of all hooks in the injected DLL.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct HookStatus {
    pub game_loop: bool,
    pub movement: bool,
    pub casting: bool,
    pub targeting: bool,
}

/// Commands sent from orchestrator to DLL via named pipe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    /// Move to a position.
    MoveTo(Position),
    /// Stop all movement.
    StopMovement,
    /// Cast a spell by gem number (0-indexed) on optional target spawn ID.
    CastSpell { gem: u8, target_id: Option<u32> },
    /// Target a spawn by ID.
    TargetSpawn(u32),
    /// Clear target.
    ClearTarget,
    /// Execute an EQ slash command (e.g., "/say hello").
    SlashCommand(String),
    /// Request the DLL to unhook and unload itself.
    Eject,
    /// Ping -- DLL should respond with Pong.
    Ping,
}

/// Responses from DLL to orchestrator via named pipe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Error(String),
    Pong,
    HookStatus(HookStatus),
}
```

- [ ] **Step 2.2: Define IPC constants and shared memory layout in `textquest-common/src/ipc.rs`**

```rust
/// Named pipe name template. The `{}` is replaced with the client PID.
pub const PIPE_NAME_TEMPLATE: &str = r"\\.\pipe\dmft-cmd-{}";

/// Shared memory name template. The `{}` is replaced with the client PID.
pub const SHMEM_NAME_TEMPLATE: &str = "dmft-state-{}";

/// Size of the shared memory region in bytes.
pub const SHMEM_SIZE: usize = 4096;

/// Shared memory header layout (repr(C) for cross-process compatibility).
#[repr(C)]
pub struct SharedGameStateHeader {
    /// Monotonically increasing write counter. Reader checks this before and
    /// after reading to detect torn writes (seqlock pattern).
    pub sequence: std::sync::atomic::AtomicU64,
    /// Byte length of the serialized GameState that follows.
    pub data_len: u32,
    /// Reserved padding.
    pub _pad: [u8; 4],
    // Followed by `data_len` bytes of bincode-serialized GameState.
}

pub fn pipe_name(pid: u32) -> String {
    PIPE_NAME_TEMPLATE.replace("{}", &pid.to_string())
}

pub fn shmem_name(pid: u32) -> String {
    SHMEM_NAME_TEMPLATE.replace("{}", &pid.to_string())
}
```

- [ ] **Step 2.3: Define protocol framing in `textquest-common/src/protocol.rs`**

```rust
/// Named pipe message format:
/// [u32 little-endian length][bincode payload]
///
/// The payload is either a Command (orchestrator -> DLL)
/// or a Response (DLL -> orchestrator).

use anyhow::Result;
use std::io::{Read, Write};

pub fn write_message<W: Write, T: serde::Serialize>(writer: &mut W, msg: &T) -> Result<()> {
    let data = bincode::serialize(msg)?;
    let len = (data.len() as u32).to_le_bytes();
    writer.write_all(&len)?;
    writer.write_all(&data)?;
    writer.flush()?;
    Ok(())
}

pub fn read_message<R: Read, T: serde::de::DeserializeOwned>(reader: &mut R) -> Result<T> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut data = vec![0u8; len];
    reader.read_exact(&mut data)?;
    Ok(bincode::deserialize(&data)?)
}
```

- [ ] **Step 2.4: Move offset constants to `textquest-common/src/offsets.rs`**

Copy the contents of `textquest/src/eq/offsets.rs` into `textquest-common/src/offsets.rs`. Keep the original file in `dmft/` but change it to re-export: `pub use textquest_common::offsets::*;`. This way both the orchestrator and the DLL share identical offset definitions.

- [ ] **Step 2.5: Wire up `textquest-common/src/lib.rs`**

```rust
pub mod types;
pub mod ipc;
pub mod offsets;
pub mod protocol;
```

- [ ] **Step 2.6: Verification**

```bash
cargo build -p textquest-common 2>&1    # must compile
cargo build 2>&1                    # full workspace must compile
```

---

## Task 3: Create `textquest-dll` Crate (DLL Payload with DllMain)

**Goal:** Build the `cdylib` crate that will be injected into eqgame.exe. Implement `DllMain` entry point with attach/detach handling and a logging/initialization framework. No hooks yet -- just the skeleton.

**Files to create/modify:**

- `textquest-dll/src/lib.rs` -- `DllMain` + initialization
- `textquest-dll/src/hooks/mod.rs` -- hook manager skeleton
- `textquest-dll/src/ipc/mod.rs` -- IPC skeleton
- `textquest-dll/src/eq/mod.rs` -- EQ function address resolution skeleton

### Steps

- [ ] **Step 3.1: Implement `DllMain` in `textquest-dll/src/lib.rs`**

```rust
#![cfg_attr(windows, no_main)]

#[cfg(windows)]
use windows::Win32::Foundation::{BOOL, HMODULE, TRUE};
#[cfg(windows)]
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};

mod hooks;
mod ipc;
mod eq;

#[cfg(windows)]
#[unsafe(no_mangle)]
pub extern "system" fn DllMain(
    _hinst: HMODULE,
    reason: u32,
    _reserved: *mut core::ffi::c_void,
) -> BOOL {
    match reason {
        DLL_PROCESS_ATTACH => {
            // Spawn initialization on a new thread to avoid loader lock.
            std::thread::spawn(|| {
                if let Err(e) = initialize() {
                    // Log to a file since we have no console.
                    let _ = std::fs::write(
                        "C:\\dmft\\textquest-dll-error.log",
                        format!("Init failed: {e:?}"),
                    );
                }
            });
            TRUE
        }
        DLL_PROCESS_DETACH => {
            shutdown();
            TRUE
        }
        _ => TRUE,
    }
}

#[cfg(windows)]
fn initialize() -> anyhow::Result<()> {
    // 1. Resolve EQ base address (GetModuleHandleW(NULL))
    // 2. Set up IPC (shared memory + named pipe)
    // 3. Install hooks
    // 4. Enter command processing loop
    todo!("Task 5-9 will fill this in")
}

#[cfg(windows)]
fn shutdown() {
    // Unhook everything, close IPC handles
    hooks::unhook_all();
}

// Non-Windows stub so the crate compiles on macOS
#[cfg(not(windows))]
pub fn _stub() {}
```

- [ ] **Step 3.2: Create hook manager skeleton (`textquest-dll/src/hooks/mod.rs`)**

```rust
pub mod game_loop;
pub mod movement;
pub mod casting;
pub mod targeting;

/// Unhook all installed hooks. Called on DLL detach.
pub fn unhook_all() {
    // Will be implemented as hooks are added in Tasks 6-9
}
```

Create empty stub files for each hook module (`game_loop.rs`, `movement.rs`, `casting.rs`, `targeting.rs`) -- each should contain a comment placeholder and `#![allow(dead_code)]`.

- [ ] **Step 3.3: Create IPC skeleton (`textquest-dll/src/ipc/mod.rs`)**

Stub out `pub fn setup_ipc(pid: u32) -> anyhow::Result<()>` and `pub fn shutdown_ipc()`.

- [ ] **Step 3.4: Create EQ address resolution skeleton (`textquest-dll/src/eq/mod.rs`)**

```rust
/// Resolve the base address of eqgame.exe from within the process.
#[cfg(windows)]
pub fn get_eq_base() -> anyhow::Result<usize> {
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    let handle = unsafe { GetModuleHandleW(None)? };
    Ok(handle.0 as usize)
}

#[cfg(not(windows))]
pub fn get_eq_base() -> anyhow::Result<usize> {
    Ok(textquest_common::offsets::EQ_PREFERRED_BASE as usize)
}
```

- [ ] **Step 3.5: Verification**

```bash
cargo build -p textquest-dll 2>&1   # must compile (cdylib on Windows, rlib fallback on macOS)
```

Note: On macOS, `cdylib` will produce a `.dylib` -- that is fine. The actual `.dll` is only needed on Windows. The key check is that the code compiles.

---

## Task 4: Add Injection Logic to `textquest` Binary

**Goal:** Implement `CreateRemoteThread` + `LoadLibraryW` injection from the orchestrator into a running eqgame.exe process. Include randomized DLL name to avoid simple signature detection.

**Files to create:**

- `textquest/src/inject/mod.rs`
- `textquest/src/inject/loader.rs` -- injection logic
- `textquest/src/inject/dll_prep.rs` -- DLL file preparation (copy + rename)

### Steps

- [ ] **Step 4.1: Implement DLL preparation (`textquest/src/inject/dll_prep.rs`)**

```rust
use anyhow::{Result, Context};
use rand::Rng;
use std::path::{Path, PathBuf};

/// Copy the compiled DLL to a temp location with a randomized name.
/// Returns the full path to the copied DLL.
///
/// The source DLL is expected at `./textquest-dll.dll` (or from a configured path).
/// The destination is `C:\dmft\payloads\<random_name>.dll`.
pub fn prepare_dll(source_dll: &Path) -> Result<PathBuf> {
    // Generate random 8-char hex name
    let mut rng = rand::thread_rng();
    let name: String = (0..8).map(|_| format!("{:x}", rng.gen::<u8>())).collect();
    let dest_dir = PathBuf::from(r"C:\dmft\payloads");
    std::fs::create_dir_all(&dest_dir)
        .context("Failed to create payload directory")?;
    let dest = dest_dir.join(format!("{name}.dll"));
    std::fs::copy(source_dll, &dest)
        .context("Failed to copy DLL to payload directory")?;
    Ok(dest)
}

/// Clean up a previously injected DLL file.
pub fn cleanup_dll(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path).context("Failed to remove DLL file")?;
    }
    Ok(())
}
```

Non-Windows: Both functions should have `#[cfg(windows)]` with stubs that return `Ok` with a dummy path / no-op.

- [ ] **Step 4.2: Implement injection (`textquest/src/inject/loader.rs`)**

```rust
use anyhow::{Result, Context, bail};
use std::path::Path;

/// Inject a DLL into a target process by PID.
///
/// Steps:
/// 1. OpenProcess with PROCESS_ALL_ACCESS
/// 2. VirtualAllocEx -- allocate memory in target for the DLL path string (wide)
/// 3. WriteProcessMemory -- write the wide DLL path into allocated memory
/// 4. GetProcAddress(GetModuleHandleW("kernel32"), "LoadLibraryW")
/// 5. CreateRemoteThread with LoadLibraryW as the start routine and
///    the allocated string as the argument
/// 6. WaitForSingleObject -- wait for the remote thread to complete
/// 7. VirtualFreeEx -- free the allocated memory
///
/// Returns the HMODULE of the loaded DLL in the remote process (from thread exit code).
#[cfg(windows)]
pub fn inject_dll(pid: u32, dll_path: &Path) -> Result<u64> {
    todo!("implement with windows crate APIs")
}

#[cfg(not(windows))]
pub fn inject_dll(pid: u32, dll_path: &Path) -> Result<u64> {
    tracing::warn!(pid, path = %dll_path.display(), "inject_dll stub (non-Windows)");
    Ok(0xDEAD_BEEF)
}

/// Eject a DLL from a target process.
///
/// Steps:
/// 1. CreateRemoteThread with FreeLibrary and the HMODULE as argument
/// 2. WaitForSingleObject
#[cfg(windows)]
pub fn eject_dll(pid: u32, remote_hmodule: u64) -> Result<()> {
    todo!("implement with windows crate APIs")
}

#[cfg(not(windows))]
pub fn eject_dll(pid: u32, _remote_hmodule: u64) -> Result<()> {
    tracing::warn!(pid, "eject_dll stub (non-Windows)");
    Ok(())
}
```

The `#[cfg(windows)]` implementation must:

- Convert `dll_path` to a wide string (`OsStr::encode_wide`)
- Use `VirtualAllocEx` with `MEM_COMMIT | MEM_RESERVE` and `PAGE_READWRITE`
- Use `WriteProcessMemory` to write the wide path bytes
- Resolve `LoadLibraryW` address via `GetModuleHandleW("kernel32.dll")` + `GetProcAddress`
- Call `CreateRemoteThread` with the resolved address and the allocated buffer
- Wait with `WaitForSingleObject` (timeout: 10 seconds)
- Read the thread exit code as the remote HMODULE
- Free the virtual allocation

Add these features to `dmft/Cargo.toml` Windows dependencies:

```toml
"Win32_System_Memory",
"Win32_System_LibraryLoader",
```

- [ ] **Step 4.3: Create `textquest/src/inject/mod.rs`**

```rust
pub mod loader;
pub mod dll_prep;

use anyhow::Result;
use std::path::Path;

/// High-level inject: prepare DLL + inject into target PID.
/// Returns (remote_hmodule, dll_path) for later ejection/cleanup.
pub fn inject_into(pid: u32, source_dll: &Path) -> Result<(u64, std::path::PathBuf)> {
    let prepared = dll_prep::prepare_dll(source_dll)?;
    let hmod = loader::inject_dll(pid, &prepared)?;
    tracing::info!(pid, hmod, path = %prepared.display(), "DLL injected");
    Ok((hmod, prepared))
}
```

- [ ] **Step 4.4: Wire injection module into `textquest/src/main.rs`**

Add `mod inject;` to the module declarations. Do not call it from `main()` yet -- that happens in Task 11 (multi-client management).

- [ ] **Step 4.5: Verification**

```bash
cargo build -p dmft 2>&1      # must compile on macOS with stubs
cargo clippy -p dmft 2>&1     # no warnings
```

On Windows (manual test): build, then inject into a test process (not yet eqgame -- use a benign process) to verify the injection mechanics work.

---

## Task 5: IPC Setup (Shared Memory + Named Pipes)

**Goal:** Implement bi-directional IPC between the orchestrator and each injected DLL. Shared memory carries high-frequency game state (written by DLL, read by orchestrator). Named pipes carry commands (orchestrator -> DLL) and responses (DLL -> orchestrator).

**Files to create/modify:**

- `textquest/src/ipc/mod.rs`, `textquest/src/ipc/pipe.rs`, `textquest/src/ipc/shared.rs` -- orchestrator side
- `textquest-dll/src/ipc/mod.rs`, `textquest-dll/src/ipc/pipe.rs`, `textquest-dll/src/ipc/shared.rs` -- DLL side

### Steps

- [ ] **Step 5.1: Implement shared memory writer in `textquest-dll/src/ipc/shared.rs`**

The DLL side creates the shared memory region and writes `GameState` snapshots using the seqlock pattern from `SharedGameStateHeader`:

```rust
/// Create shared memory and return a writer handle.
/// Name: "dmft-state-{pid}" (from textquest_common::ipc::shmem_name)
/// Size: SHMEM_SIZE (4096 bytes)
///
/// Write pattern (seqlock):
///   1. Increment sequence to odd (signals write in progress)
///   2. Serialize GameState via bincode, write data_len + data
///   3. Increment sequence to even (signals write complete)
///
/// pub fn create_shared_state(pid: u32) -> Result<SharedStateWriter>
/// pub fn write_state(&self, state: &GameState) -> Result<()>
```

Use `CreateFileMappingW` + `MapViewOfFile` on Windows; stub on macOS.

- [ ] **Step 5.2: Implement shared memory reader in `textquest/src/ipc/shared.rs`**

The orchestrator side opens the existing shared memory and reads `GameState`:

```rust
/// Open shared memory created by the DLL.
/// pub fn open_shared_state(pid: u32) -> Result<SharedStateReader>
///
/// Read pattern (seqlock):
///   1. Read sequence -- if odd, spin-wait (write in progress)
///   2. Read data_len + data
///   3. Re-read sequence -- if changed, retry (torn read)
///   4. Deserialize GameState from bincode
///
/// pub fn read_state(&self) -> Result<GameState>
```

- [ ] **Step 5.3: Implement named pipe server in `textquest-dll/src/ipc/pipe.rs`**

The DLL creates a named pipe server and listens for commands in a background thread:

```rust
/// Create a named pipe server: \\.\pipe\dmft-cmd-{pid}
/// Runs in a background thread.
/// Reads Command messages, processes them, writes Response messages.
///
/// pub fn start_command_server(pid: u32, cmd_handler: impl Fn(Command) -> Response + Send + 'static) -> Result<PipeServerHandle>
/// pub fn stop_command_server(handle: PipeServerHandle)
```

Use `CreateNamedPipeW` + `ConnectNamedPipe` on Windows; stub on macOS.

- [ ] **Step 5.4: Implement named pipe client in `textquest/src/ipc/pipe.rs`**

The orchestrator connects to the DLL's named pipe to send commands:

```rust
/// Connect to the DLL's named pipe.
/// pub fn connect_to_client(pid: u32) -> Result<PipeClient>
///
/// Send a command and receive the response.
/// pub fn send_command(&mut self, cmd: &Command) -> Result<Response>
```

Use `CreateFileW` to open the pipe on Windows; stub on macOS.

- [ ] **Step 5.5: Wire up IPC modules**

Add `mod ipc;` to both `textquest/src/main.rs` and `textquest-dll/src/lib.rs`.

- [ ] **Step 5.6: Verification**

```bash
cargo build 2>&1   # full workspace compiles
```

On Windows (manual test): write a small integration test that creates shared memory in one thread, reads from another, and verifies data round-trips correctly. Similarly test named pipe send/receive.

---

## Task 6: Hook EQ's Game Loop Tick Function

**Goal:** Hook the main game loop tick function in eqgame.exe using the `retour` crate. This is the anchor point for all per-frame operations -- reading game state, processing queued commands, and updating shared memory.

**Files to create/modify:**

- `textquest-dll/src/hooks/game_loop.rs`
- `textquest-dll/src/eq/mod.rs` -- add game loop function address

### Steps

- [ ] **Step 6.1: Identify the game loop function address**

The target function is `CEverQuest::MainLoop` (or the render tick). From MQ2 reference, identify the function signature and preferred-base address. Add to `textquest-common/src/offsets.rs`:

```rust
/// CEverQuest::MainLoop -- called once per frame
/// Signature: void CEverQuest::MainLoop()
/// Find via: PINST_CEVERQUEST vtable or xref from MQ2
pub const FN_CEVERQUEST_MAIN_LOOP: u64 = 0x0; // TODO: extract from MQ2 reference
```

The actual address must be extracted from the MQ2 reference source. Look in `mq2-reference/` for `ProcessGameEvents`, `RealRender_World`, or `MainLoop` patterns.

- [ ] **Step 6.2: Implement the game loop hook (`textquest-dll/src/hooks/game_loop.rs`)**

```rust
use retour::static_detour;
use textquest_common::offsets;

// Define the detour
static_detour! {
    static MainLoopHook: unsafe extern "C" fn();
}

/// Our replacement function. Called instead of the original MainLoop.
fn main_loop_detour() {
    // 1. Call the original function first (let EQ do its thing)
    unsafe { MainLoopHook.call(); }

    // 2. Read current game state from EQ memory
    // 3. Write game state to shared memory
    // 4. Process any pending commands from the named pipe
}

/// Install the game loop hook.
pub unsafe fn install(eq_base: usize) -> anyhow::Result<()> {
    let offset = offsets::FN_CEVERQUEST_MAIN_LOOP - offsets::EQ_PREFERRED_BASE;
    let target_addr = eq_base + offset as usize;
    let target_fn: unsafe extern "C" fn() = std::mem::transmute(target_addr);

    MainLoopHook.initialize(target_fn, main_loop_detour)?;
    MainLoopHook.enable()?;

    Ok(())
}

/// Remove the game loop hook.
pub unsafe fn uninstall() -> anyhow::Result<()> {
    MainLoopHook.disable()?;
    Ok(())
}
```

- [ ] **Step 6.3: Call from `initialize()` in `textquest-dll/src/lib.rs`**

Wire the hook installation into the DLL's initialization function.

- [ ] **Step 6.4: Add to `unhook_all()` in `textquest-dll/src/hooks/mod.rs`**

- [ ] **Step 6.5: Verification**

On Windows: inject into eqgame.exe, verify the hook fires by writing a timestamp to the shared memory region. The orchestrator should see the tick counter incrementing. Verify EQ continues to run normally (no crash, no freeze).

---

## Task 7: Hook Movement Functions

**Goal:** Hook EQ's movement functions to enable programmatic control of character movement. This allows the orchestrator to command followers to move to specific coordinates.

**Files to create/modify:**

- `textquest-dll/src/hooks/movement.rs`
- `textquest-common/src/offsets.rs` -- add movement function addresses

### Steps

- [ ] **Step 7.1: Identify movement function addresses**

From MQ2 reference, find:

- `PlayerZoneClient::MovePlayer` or `CLivePlayer::ProcessInput` -- raw movement processing
- `CEverQuest::MoveToLocation` or equivalent -- high-level move-to command
- `PlayerClient::ChangePosition` -- position update

Add to `textquest-common/src/offsets.rs`:

```rust
/// Movement-related function addresses (preferred base).
/// Signatures and addresses from MQ2 reference.
pub const FN_MOVE_PLAYER: u64 = 0x0;         // TODO: extract
pub const FN_SET_HEADING: u64 = 0x0;          // TODO: extract
pub const FN_CHANGE_POSITION: u64 = 0x0;      // TODO: extract
```

- [ ] **Step 7.2: Implement movement hooks (`textquest-dll/src/hooks/movement.rs`)**

```rust
use retour::static_detour;

static_detour! {
    /// Hook on the movement processing function.
    /// When our code wants to move the character, we override the inputs.
    static MovePlayerHook: unsafe extern "thiscall" fn(*mut u8, f32, f32, f32);
}

/// Pending movement command (set by command handler, consumed by hook).
static PENDING_MOVE: std::sync::Mutex<Option<textquest_common::types::Position>> =
    std::sync::Mutex::new(None);

/// Queue a movement command.
pub fn queue_move(pos: textquest_common::types::Position) {
    *PENDING_MOVE.lock().unwrap() = Some(pos);
}

fn move_player_detour(this: *mut u8, x: f32, y: f32, z: f32) {
    // If we have a pending move, override the coordinates
    if let Some(pos) = PENDING_MOVE.lock().unwrap().take() {
        unsafe { MovePlayerHook.call(this, pos.x, pos.y, pos.z); }
    } else {
        // Normal movement -- pass through
        unsafe { MovePlayerHook.call(this, x, y, z); }
    }
}

pub unsafe fn install(eq_base: usize) -> anyhow::Result<()> { todo!() }
pub unsafe fn uninstall() -> anyhow::Result<()> { todo!() }
```

Note: The actual calling convention may be `extern "fastcall"` or `extern "C"` depending on the compiler. Check MQ2 reference for the correct convention. x64 Windows uses the Microsoft x64 calling convention -- first four args in `RCX, RDX, R8, R9`.

- [ ] **Step 7.3: Wire into hook manager and `initialize()`**

- [ ] **Step 7.4: Handle `Command::MoveTo` and `Command::StopMovement` in the command processor**

When the game loop tick fires and there is a pending `MoveTo` command from the pipe, call `queue_move()`.

- [ ] **Step 7.5: Verification**

On Windows: inject into eqgame.exe, send a `MoveTo` command via named pipe from the orchestrator. Verify the character moves to the specified coordinates. Verify normal WASD movement still works when no commands are pending.

---

## Task 8: Hook Spell Casting Functions

**Goal:** Hook EQ's spell casting functions to enable programmatic spell casting. The orchestrator can command a client to cast a specific spell gem on a specific target.

**Files to create/modify:**

- `textquest-dll/src/hooks/casting.rs`
- `textquest-common/src/offsets.rs` -- add casting function addresses

### Steps

- [ ] **Step 8.1: Identify casting function addresses**

From MQ2 reference, find:

- `PcClient::CastSpell` or `EQ_Spell::CastSpell` -- initiates a spell cast
- `PcZoneClient::UseAbility` -- alternative entry point for abilities/AAs
- Spell gem slot mapping

Add to `textquest-common/src/offsets.rs`:

```rust
/// Casting-related function addresses (preferred base).
pub const FN_CAST_SPELL: u64 = 0x0;           // TODO: extract from MQ2 reference
pub const FN_STOP_CAST: u64 = 0x0;            // TODO: extract
```

- [ ] **Step 8.2: Implement casting hooks (`textquest-dll/src/hooks/casting.rs`)**

Two approaches (implement both, use whichever works):

**Approach A -- Direct function call (preferred):** Instead of hooking, directly call `CastSpell` from within EQ's process context. This is safe because we execute on the game loop thread (Task 6).

```rust
/// Cast a spell by gem index on an optional target.
/// Must be called from the game loop thread (via command processing).
pub unsafe fn cast_spell(eq_base: usize, gem: u8, target_id: Option<u32>) -> anyhow::Result<()> {
    // 1. If target_id is provided, set the target first (Task 9)
    // 2. Resolve CastSpell function address
    // 3. Get local PcClient pointer
    // 4. Call CastSpell(gem_index, spell_id, ...)
    todo!()
}
```

**Approach B -- Hook CastSpell to intercept/modify:** Hook the function to add logging, cooldown tracking, or cast interception.

- [ ] **Step 8.3: Wire into command processor**

Handle `Command::CastSpell { gem, target_id }` in the game loop tick command processor.

- [ ] **Step 8.4: Verification**

On Windows: inject into eqgame.exe, send a `CastSpell` command. Verify the character begins casting the spell in the specified gem slot. Verify normal spell casting via the EQ UI still works.

---

## Task 9: Hook Targeting Functions

**Goal:** Hook EQ's targeting functions to enable programmatic target selection by spawn ID.

**Files to create/modify:**

- `textquest-dll/src/hooks/targeting.rs`
- `textquest-common/src/offsets.rs` -- add targeting function addresses

### Steps

- [ ] **Step 9.1: Identify targeting function addresses**

From MQ2 reference, find:

- `EQPlayer::SetTarget` or the global target pointer write
- `CEverQuest::SetTarget` -- higher-level target setter
- The global `PINST_TARGET` pointer (already in offsets.rs: `0x140E8E428`)

Add to `textquest-common/src/offsets.rs`:

```rust
/// Targeting-related function addresses (preferred base).
pub const FN_SET_TARGET: u64 = 0x0;           // TODO: extract from MQ2 reference
```

- [ ] **Step 9.2: Implement targeting (`textquest-dll/src/hooks/targeting.rs`)**

```rust
/// Set the current target by spawn ID.
/// Walks the spawn list to find the PlayerClient* for the given spawn ID,
/// then writes it to the target pointer.
///
/// Must be called from the game loop thread.
pub unsafe fn set_target(eq_base: usize, spawn_id: u32) -> anyhow::Result<()> {
    // 1. Read SpawnManager pointer
    // 2. Walk the spawn linked list (reuse offset constants from textquest_common::offsets)
    // 3. Find the spawn with matching SPAWN_ID
    // 4. Write the spawn pointer to PINST_TARGET
    // Alternatively, call CEverQuest::SetTarget(PlayerClient*) if available
    todo!()
}

/// Clear the current target (write null to PINST_TARGET).
pub unsafe fn clear_target(eq_base: usize) -> anyhow::Result<()> {
    let target_addr = textquest_common::offsets::rebase(
        textquest_common::offsets::PINST_TARGET, eq_base as u64
    );
    std::ptr::write(target_addr as *mut u64, 0);
    Ok(())
}
```

- [ ] **Step 9.3: Wire into command processor**

Handle `Command::TargetSpawn(id)` and `Command::ClearTarget` in the game loop tick.

- [ ] **Step 9.4: Verification**

On Windows: inject into eqgame.exe, send a `TargetSpawn` command with a known spawn ID. Verify the target window updates in EQ. Verify `ClearTarget` clears it. Verify manual targeting via clicking still works.

---

## Task 10: Self-Healing (Process Monitoring + Auto-Restart + Re-Injection)

**Goal:** Monitor all managed EQ processes. If a client crashes, automatically restart eqgame.exe, wait for it to load, and re-inject the DLL.

**Files to create:**

- `textquest/src/client/healing.rs`

### Steps

- [ ] **Step 10.1: Implement process health monitor (`textquest/src/client/healing.rs`)**

```rust
/// Health check result for a single EQ client.
pub enum HealthStatus {
    /// Process is running, DLL is responding to pings.
    Healthy,
    /// Process is running but DLL is not responding (may need re-injection).
    Unresponsive,
    /// Process has exited.
    Dead(u32), // exit code
}

/// Check the health of a single EQ client.
/// 1. Check if the PID is still alive (OpenProcess or similar)
/// 2. Send a Ping command via named pipe
/// 3. If Pong received within 2 seconds -> Healthy
/// 4. If timeout -> Unresponsive
/// 5. If process gone -> Dead
pub fn check_health(pid: u32) -> HealthStatus { todo!() }
```

- [ ] **Step 10.2: Implement EQ restart logic**

```rust
/// Restart an EQ client.
/// 1. Launch eqgame.exe via CreateProcess (from configured EQ install path)
/// 2. Wait for the process to initialize (poll for the main window)
/// 3. Return the new PID
///
/// Config: EQ install path, launch arguments (e.g., patchme)
pub fn restart_eq(eq_path: &Path, args: &[String]) -> Result<u32> { todo!() }
```

- [ ] **Step 10.3: Implement auto-healing loop**

```rust
/// Run in a background thread. Periodically checks all managed clients.
/// On Dead: restart + re-inject + update ClientManager
/// On Unresponsive: attempt re-injection
/// On Healthy: no action
///
/// Interval: configurable, default 5 seconds.
pub fn start_healing_loop(
    manager: Arc<Mutex<ClientManager>>,
    eq_config: EqLaunchConfig,
    source_dll: PathBuf,
) -> JoinHandle<()> { todo!() }
```

- [ ] **Step 10.4: Add EQ launch config to `config/frostreaver.toml`**

```toml
[healing]
enabled = true
check_interval_secs = 5
eq_path = "C:\\EverQuest\\eqgame.exe"
eq_args = ["patchme"]
auto_restart = true
```

- [ ] **Step 10.5: Verification**

On Windows: start the orchestrator with one EQ client. Kill eqgame.exe via Task Manager. Verify the orchestrator detects the death, restarts EQ, waits for initialization, re-injects, and resumes operation. Check logs for the full sequence.

---

## Task 11: Multi-Client Management

**Goal:** Track all running EQ processes, inject into each, maintain IPC connections, and expose the client list to the TUI.

**Files to create/modify:**

- `textquest/src/client/mod.rs`
- `textquest/src/client/manager.rs`
- `textquest/src/client/session.rs`
- `textquest/src/tui/app.rs` -- integrate ClientManager
- `textquest/src/main.rs` -- wire up injection on startup

### Steps

- [ ] **Step 11.1: Define `EqSession` (`textquest/src/client/session.rs`)**

```rust
use std::path::PathBuf;
use textquest_common::types::{ClientId, GameState};

/// Represents a single managed EQ client.
pub struct EqSession {
    pub id: ClientId,
    pub pid: u32,
    pub character_name: String,
    pub remote_hmodule: u64,
    pub dll_path: PathBuf,
    pub pipe_client: Option<crate::ipc::pipe::PipeClient>,
    pub last_state: Option<GameState>,
    pub healthy: bool,
}
```

- [ ] **Step 11.2: Implement `ClientManager` (`textquest/src/client/manager.rs`)**

```rust
use std::sync::{Arc, Mutex};

pub struct ClientManager {
    sessions: Vec<EqSession>,
    source_dll: PathBuf,
}

impl ClientManager {
    /// Discover all running eqgame.exe processes and inject into any
    /// that are not already managed.
    pub fn discover_and_inject(&mut self) -> anyhow::Result<()> { todo!() }

    /// Send a command to a specific client by character name.
    pub fn send_command(&mut self, character: &str, cmd: Command) -> anyhow::Result<Response> { todo!() }

    /// Send a command to all managed clients.
    pub fn broadcast_command(&mut self, cmd: Command) -> Vec<anyhow::Result<Response>> { todo!() }

    /// Read the latest game state from all clients' shared memory.
    pub fn refresh_all_states(&mut self) { todo!() }

    /// Get a snapshot of all sessions (for TUI rendering).
    pub fn sessions(&self) -> &[EqSession] { &self.sessions }

    /// Remove a dead session and clean up its DLL file.
    pub fn remove_session(&mut self, pid: u32) -> anyhow::Result<()> { todo!() }
}
```

- [ ] **Step 11.3: Wire `ClientManager` into `main.rs` startup**

On Windows:

1. Build or locate `textquest-dll.dll` (from the workspace build output)
2. Create `ClientManager` with the DLL path
3. Call `discover_and_inject()` to find and inject all running EQ processes
4. Start the healing loop (Task 10)
5. Pass the `Arc<Mutex<ClientManager>>` to the TUI `App`

On macOS: skip injection, use demo data as before.

- [ ] **Step 11.4: Update TUI `App` to display multi-client state**

Add a client list panel showing:

- Character name
- PID
- Health status (Healthy/Unresponsive/Dead)
- Current zone
- HP/Mana/End percentages

The TUI should refresh client states from the `ClientManager` on each tick.

- [ ] **Step 11.5: Add keyboard shortcut to manually trigger injection**

Add a TUI keybinding (e.g., `I`) to call `discover_and_inject()` on demand.

- [ ] **Step 11.6: Verification**

On Windows:

1. Launch 2-3 eqgame.exe instances
2. Start the orchestrator (`cargo run -p dmft`)
3. Verify all clients appear in the TUI with character names
4. Verify game state (HP, mana, position) updates in real time
5. Send a command to one client, verify it executes
6. Kill one EQ client, verify self-healing restarts and re-injects

On macOS:

1. `cargo run -p dmft` should still work in demo mode
2. No crashes, no compile errors

---

## Cross-Cutting Concerns

### Build verification (run after every task)

```bash
cargo build 2>&1              # full workspace compiles
cargo clippy 2>&1             # no warnings
cargo fmt --check 2>&1        # formatting correct
```

### Windows features checklist

The `dmft/Cargo.toml` Windows dependencies need these features (accumulated across all tasks):

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.54", features = [
    "Win32_Foundation",
    "Win32_System_Diagnostics_Debug",
    "Win32_System_Threading",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_ProcessStatus",
    "Win32_System_Memory",
    "Win32_System_LibraryLoader",
    "Win32_System_Pipes",
    "Win32_Storage_FileSystem",
    "Win32_Security",
] }
```

### MQ2 reference extraction

Several tasks require extracting function addresses from the MQ2 reference source (`mq2-reference/`). For each `TODO: extract` offset:

1. Search `mq2-reference/src/eqlib/` for the function name
2. Find the address in the offset headers (eqgame.h or similar)
3. Record the preferred-base address and calling convention
4. Add to `textquest-common/src/offsets.rs`

### Logging strategy

The injected DLL cannot use stdout. Use file-based logging:

- DLL log file: `C:\dmft\logs\textquest-dll-{pid}.log`
- Use `tracing` with a file appender subscriber initialized in `DllMain`

### Safety notes

- All hook installations must happen on a dedicated thread (never inside `DllMain` with loader lock)
- The game loop detour must call the original function to avoid freezing EQ
- Shared memory writes use seqlock to prevent torn reads
- Named pipe operations must have timeouts to avoid deadlocks
- DLL ejection must unhook before unloading to avoid crash on return to freed code
