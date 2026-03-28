# DLL Injection + Integration Loop + Group Invite Plan

**Date:** 2026-03-28
**Status:** Ready for implementation
**Estimated effort:** ~200-300 lines for integration loop, ~50 lines for offset fix, ~30 lines for group invite command

---

## Table of Contents

1. [DLL Injection Sequence](#1-dll-injection-sequence)
2. [Critical Bug: MAIN_LOOP_OFFSET is Zero](#2-critical-bug-main_loop_offset-is-zero)
3. [Integration Loop Design](#3-integration-loop-design)
4. [Group Invite Sequence](#4-group-invite-sequence)
5. [Risk Assessment](#5-risk-assessment)
6. [EQ Function Address Inventory](#6-eq-function-address-inventory)
7. [Testing Checklist](#7-testing-checklist)

---

## 1. DLL Injection Sequence

### Step-by-step injection flow

The injection uses the classic **CreateRemoteThread + LoadLibraryW** technique. Here's the exact sequence:

#### Phase A: Preparation (orchestrator, `dll_prep.rs`)

1. `prepare_dll(source_dll)` copies `dmft_dll.dll` from `target/release/` to `%TEMP%\dmft_payloads\` with a randomized system-looking name (e.g., `dx_core4821.dll`)
2. Returns the full path to the staged DLL

#### Phase B: Injection (orchestrator, `loader.rs`)

1. **Open target process** — `OpenProcess` with `PROCESS_CREATE_THREAD | PROCESS_VM_OPERATION | PROCESS_VM_WRITE | PROCESS_VM_READ | PROCESS_QUERY_INFORMATION`
2. **Allocate remote memory** — `VirtualAllocEx` in the target process, sized for the UTF-16 DLL path string
3. **Write DLL path** — `WriteProcessMemory` writes the UTF-16 path into the allocated buffer
4. **Resolve LoadLibraryW** — `GetModuleHandleW("kernel32.dll")` + `GetProcAddress("LoadLibraryW")` to get the address (kernel32 is at the same address in all processes on the same boot)
5. **Create remote thread** — `CreateRemoteThread` calling `LoadLibraryW` with the remote buffer as argument
6. **Wait for completion** — `WaitForSingleObject` with 10-second timeout
7. **Cleanup** — `VirtualFreeEx` the remote buffer, `CloseHandle` the thread and process handles

#### Phase C: DLL initialization (inside eqgame.exe, `lib.rs`)

When `LoadLibraryW` loads our DLL, Windows calls `DllMain` with `DLL_PROCESS_ATTACH`:

1. **DllMain** (under loader lock — minimal work):
   - `DisableThreadLibraryCalls` to suppress THREAD_ATTACH/DETACH notifications
   - `CreateThread` (raw Win32, NOT `std::thread::spawn`) to spawn `init_thread`
   - Returns `TRUE` immediately

2. **init_thread** (off loader lock — real init):
   - **Tracing** — Sets up file-based logging to `%TEMP%\dmft\dmft-dll.log` (daily rolling)
   - **Resolve EQ base** — `GetModuleHandleW(None)` returns eqgame.exe's base address (since the DLL is loaded INTO eqgame.exe)
   - **Store base** — `EQ_BASE.store(eq_base, Ordering::Release)`
   - **Install hooks** — Calls `install_hooks(eq_base)` which computes `eq_base + MAIN_LOOP_OFFSET` and installs a `retour::static_detour` on `CEverQuest::MainLoop`
   - **Start IPC** — Creates shared memory region `dmft_state_{pid}` and named pipe `\\.\pipe\dmft_cmd_{pid}`, spawns listener thread

#### Phase D: Game loop hook active (`game_loop.rs`)

Once the hook is installed, every EQ game tick:

1. Original `MainLoop(this)` runs first (EQ processes normally)
2. Our `main_loop_detour` runs `on_game_tick()`:
   - `crate::nav::tick()` — navigation FSM
   - TODO: Read game state, publish to shared memory, poll IPC commands
   - TODO: Combat FSM tick

### Log verification

- **Orchestrator logs**: Check for `"DLL injected successfully"` with pid and dll path
- **DLL logs**: `%TEMP%\dmft\dmft-dll.log` — look for:
  - `"DMFT DLL initializing (pid=XXXX)"`
  - `"EQ base address resolved"` with `base=0x...`
  - `"Game loop hook installed"` with the address
  - `"IPC started"`
  - `"DMFT DLL initialized successfully"`

### Error handling & recovery

| Failure | Detection | Recovery |
|---------|-----------|----------|
| `OpenProcess` fails | Returns error immediately | Check process is still running, check admin privileges |
| `VirtualAllocEx` fails | Returns null | Process may be out of memory or access denied |
| `WriteProcessMemory` fails | Returns error | Cleanup allocated memory, retry |
| `CreateRemoteThread` fails | Returns error | Process may have thread creation restrictions |
| `WaitForSingleObject` timeout | Returns non-zero | DLL is stuck in DllMain — **do not free remote_buf** (would crash target) |
| Hook install fails | Logged as warning | DLL continues without hooks (IPC still works for diagnostics) |
| IPC startup fails | Logged as warning | DLL continues without IPC (hooks still run) |

---

## 2. Critical Bug: MAIN_LOOP_OFFSET is Zero

### The problem

`dmft-dll/src/eq/mod.rs` has:
```rust
pub const MAIN_LOOP_OFFSET: usize = 0x0; // placeholder
```

But `dmft-common/src/offsets.rs` has the real address:
```rust
pub const PROCESS_GAME_EVENTS: u64 = 0x14028E0F0;
```

In `lib.rs:install_hooks()`:
```rust
let main_loop_offset = eq::MAIN_LOOP_OFFSET; // reads 0!
if main_loop_offset == 0 {
    tracing::info!("Game loop hook skipped (offset not yet resolved)");
    return Ok(()); // SILENTLY SKIPS THE HOOK
}
```

### The fix

The DLL should use `dmft_common::offsets` to compute the runtime address. Replace the placeholder approach:

```rust
// In lib.rs:install_hooks()
fn install_hooks(eq_base: u64) -> Result<(), Box<dyn std::error::Error>> {
    use dmft_common::offsets;

    let main_loop_addr = offsets::rebase(offsets::PROCESS_GAME_EVENTS, eq_base)
        .ok_or("Failed to rebase PROCESS_GAME_EVENTS")?;

    hooks::game_loop::install(main_loop_addr)?;
    Ok(())
}
```

And remove or deprecate `dmft-dll/src/eq/mod.rs` placeholder constants — they duplicate `dmft-common/src/offsets.rs` and are all zero.

**This must be fixed before the first live injection test or the hook won't install.**

---

## 3. Integration Loop Design

### Overview

The integration loop is the missing coordinator that ties together:
- Process discovery (working)
- DLL injection (built, needs offset fix)
- IPC setup (built)
- Command dispatch (built but not wired)
- Health monitoring (built)

### Where it lives

New function in `dmft/src/client/manager.rs` or a new file `dmft/src/orchestrator.rs`.

### Data flow

```
┌─────────────────────────────────────────────┐
│              Orchestrator Loop               │
│  (runs on a background thread, ~100ms tick)  │
├─────────────────────────────────────────────┤
│                                             │
│  1. discover_new_clients()                  │
│  2. inject_pending_clients()                │
│  3. setup_ipc_channels()                    │
│  4. poll_shared_memory()   ← GameState      │
│  5. health_check()         ← Ping/Pong      │
│  6. dispatch_commands()    → Command pipe    │
│  7. handle_responses()     ← Response pipe   │
│                                             │
└─────────────────────────────────────────────┘
        │                          ▲
        │ Named Pipe               │ Shared Memory
        ▼                          │
┌──────────────────────────────────────────────┐
│         eqgame.exe (per client)              │
│  ┌────────────────────────────────────────┐  │
│  │  dmft-dll.dll                          │  │
│  │  - GameLoop hook → on_game_tick()      │  │
│  │  - SharedStateWriter (publishes state) │  │
│  │  - CommandListener (receives commands) │  │
│  └────────────────────────────────────────┘  │
└──────────────────────────────────────────────┘
```

### Struct additions

```rust
// In session.rs — add IPC handles to EqSession
pub struct EqSession {
    // ... existing fields ...
    pub pipe: Option<CommandPipe>,           // orchestrator → DLL command channel
    pub state_reader: Option<SharedStateReader>, // DLL → orchestrator state channel
    pub session_token: Option<SessionToken>, // for pipe authentication
}
```

### Integration loop — code outline

```rust
// dmft/src/orchestrator.rs (new file, ~200-300 lines)

use std::time::{Duration, Instant};
use std::path::Path;
use std::collections::HashMap;

use anyhow::Result;
use dmft_common::ipc::{Command, Response};
use dmft_common::types::ClientId;

use crate::client::manager::ClientManager;
use crate::ipc::pipe::CommandPipe;
use crate::ipc::shared::SharedStateReader;

/// Pending commands queued by the TUI or automation systems.
pub struct PendingCommand {
    pub target_client: ClientId,
    pub command: Command,
}

/// The main orchestrator loop state.
pub struct Orchestrator {
    manager: ClientManager,
    dll_path: std::path::PathBuf,
    pipes: HashMap<ClientId, CommandPipe>,
    state_readers: HashMap<ClientId, SharedStateReader>,
    command_queue: Vec<PendingCommand>,
    tick_interval: Duration,
}

impl Orchestrator {
    pub fn new(process_name: &str, dll_path: std::path::PathBuf) -> Self {
        Self {
            manager: ClientManager::new(process_name),
            dll_path,
            pipes: HashMap::new(),
            state_readers: HashMap::new(),
            command_queue: Vec::new(),
            tick_interval: Duration::from_millis(100),
        }
    }

    /// Queue a command for a specific client.
    pub fn queue_command(&mut self, target: ClientId, cmd: Command) {
        self.command_queue.push(PendingCommand {
            target_client: target,
            command: cmd,
        });
    }

    /// Queue a command for ALL active clients.
    pub fn queue_broadcast(&mut self, cmd: Command) {
        let ids: Vec<ClientId> = self.manager.active_sessions()
            .iter().map(|s| s.client_id).collect();
        for id in ids {
            self.queue_command(id, cmd.clone());
        }
    }

    /// Single tick of the orchestrator loop.
    pub fn tick(&mut self) -> Result<()> {
        // 1. Discover new EQ processes
        self.discover_and_inject()?;

        // 2. Set up IPC for newly injected clients
        self.setup_ipc();

        // 3. Read game state from all clients
        self.poll_game_state();

        // 4. Health check (ping unresponsive clients)
        self.health_check();

        // 5. Dispatch queued commands
        self.dispatch_commands();

        Ok(())
    }

    /// Discover new EQ processes and inject DLL.
    fn discover_and_inject(&mut self) -> Result<()> {
        let new_clients = self.manager.discover()?;

        for client_id in new_clients {
            match self.manager.inject(client_id, &self.dll_path) {
                Ok(()) => {
                    tracing::info!(client_id, "Injected DLL into new client");
                    // Generate session token matching what the DLL generates
                    // (derived from PID — see lib.rs:generate_session_token)
                    // In production: orchestrator would write the token to
                    // shared memory before injection.
                }
                Err(e) => {
                    tracing::error!(client_id, error = %e, "Failed to inject DLL");
                }
            }
        }

        Ok(())
    }

    /// Set up IPC pipes and shared memory readers for injected clients.
    fn setup_ipc(&mut self) {
        for session in self.manager.all_sessions() {
            let id = session.client_id;

            // Skip if already connected or not yet injected
            if self.pipes.contains_key(&id) {
                continue;
            }
            if !matches!(
                session.hook_status,
                dmft_common::types::HookStatus::Injected
                    | dmft_common::types::HookStatus::HooksActive
            ) {
                continue;
            }

            // Give the DLL a moment to set up its pipe server
            // (DLL creates pipe in init_thread, which runs after DllMain returns)

            // Connect command pipe
            match CommandPipe::connect(id) {
                Ok(pipe) => {
                    tracing::info!(client_id = id, "Command pipe connected");
                    self.pipes.insert(id, pipe);
                }
                Err(e) => {
                    tracing::debug!(
                        client_id = id,
                        error = %e,
                        "Pipe not ready yet (DLL may still be initializing)"
                    );
                    continue; // Will retry next tick
                }
            }

            // Open shared memory reader
            match SharedStateReader::new(id) {
                Ok(reader) => {
                    tracing::info!(client_id = id, "Shared memory reader opened");
                    self.state_readers.insert(id, reader);
                }
                Err(e) => {
                    tracing::warn!(
                        client_id = id,
                        error = %e,
                        "Failed to open shared memory"
                    );
                }
            }
        }
    }

    /// Read game state from all connected clients.
    fn poll_game_state(&mut self) {
        let updates: Vec<(ClientId, dmft_common::types::GameState)> = self
            .state_readers
            .iter()
            .filter_map(|(id, reader)| reader.read().map(|state| (*id, state)))
            .collect();

        for (id, state) in updates {
            if let Some(session) = self.manager.get_mut(id) {
                session.update_state(state);
            }
        }
    }

    /// Check client health — send pings, detect unresponsive clients.
    fn health_check(&mut self) {
        let needs_ping: Vec<ClientId> = self
            .manager
            .all_sessions()
            .filter(|s| {
                s.health_monitor.ping_interval()
                    <= s.health_monitor.current_health()
                        .eq(&crate::client::healing::ClientHealth::Healthy)
                        .then_some(Duration::from_secs(5))
                        .unwrap_or(Duration::ZERO)
            })
            .map(|s| s.client_id)
            .collect();

        // Send pings to clients that have pipes
        for id in &needs_ping {
            if let Some(pipe) = self.pipes.get(id) {
                match pipe.send(&Command::Ping) {
                    Ok(Response::Pong { .. }) => {
                        if let Some(session) = self.manager.get_mut(*id) {
                            session.health_monitor.record_pong();
                        }
                    }
                    _ => {
                        tracing::warn!(client_id = id, "Ping failed");
                    }
                }
            }
        }

        // Check for crashed/unresponsive clients
        let dead = self.manager.check_health();
        for id in dead {
            tracing::warn!(client_id = id, "Client needs restart");
            // Remove stale IPC handles
            self.pipes.remove(&id);
            self.state_readers.remove(&id);
        }
    }

    /// Send all queued commands to their target clients.
    fn dispatch_commands(&mut self) {
        let commands = std::mem::take(&mut self.command_queue);

        for pending in commands {
            let Some(pipe) = self.pipes.get(&pending.target_client) else {
                tracing::warn!(
                    client_id = pending.target_client,
                    "No pipe for client, dropping command"
                );
                continue;
            };

            match pipe.send(&pending.command) {
                Ok(response) => {
                    tracing::debug!(
                        client_id = pending.target_client,
                        ?response,
                        "Command response"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        client_id = pending.target_client,
                        error = %e,
                        "Failed to send command"
                    );
                    // Pipe may be broken — remove it so we reconnect
                    self.pipes.remove(&pending.target_client);
                }
            }
        }
    }

    /// Get a reference to the client manager (for TUI to read state).
    pub fn manager(&self) -> &ClientManager {
        &self.manager
    }

    pub fn manager_mut(&mut self) -> &mut ClientManager {
        &mut self.manager
    }
}
```

### Running the loop

The orchestrator should run on a background thread, with the TUI on the main thread:

```rust
// In main.rs or run.rs
let orchestrator = Arc::new(Mutex::new(Orchestrator::new("eqgame.exe", dll_path)));

// Background orchestrator thread
let orch_clone = orchestrator.clone();
std::thread::spawn(move || {
    loop {
        if let Ok(mut orch) = orch_clone.lock() {
            if let Err(e) = orch.tick() {
                tracing::error!(error = %e, "Orchestrator tick failed");
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
});

// TUI reads state from orchestrator.manager()
// TUI queues commands via orchestrator.queue_command()
```

### IPC naming convention

Note a **naming mismatch** in the current code:

- **DLL side** (pipe server): Creates pipe named `\\.\pipe\dmft_cmd_{pid}` — uses the **process PID** as identifier
- **Orchestrator side** (pipe client): Connects to `\\.\pipe\dmft_cmd_{client_id}` — uses the **internal client ID**

These are different numbers! The `client_id` is assigned by `ClientManager` (sequential: 1, 2, 3...) while the DLL uses `std::process::id()` (the actual Windows PID).

**Fix options:**
- A) Change DLL to accept client_id via shared memory/injection parameter (cleaner)
- B) Change orchestrator to use PID as pipe name (simpler, do this first)
- **Recommendation:** Option B for initial implementation. The `CommandPipe::connect()` should take `pid: u32` instead of `client_id: ClientId`. The `SharedStateReader::new()` similarly.

---

## 4. Group Invite Sequence

### Available EQ functions

From `dmft-common/src/offsets.rs`:

| Function | Address | Use for grouping |
|----------|---------|------------------|
| `EXECUTE_CMD` | `0x1402235B0` | Execute any EQ command by ID (includes `/invite`) |
| `INTERPRET_CMD` | `0x140283FB0` | Execute a slash command string directly |
| `CLICKED_PLAYER` | `0x1402724F0` | Click-target a player |
| `SetTarget` (targeting.rs) | via `PINST_TARGET` write | Set target by spawn address |

### Group invite flow

#### Method 1: InterpretCmd (recommended — simplest)

`InterpretCmd` lets us execute any slash command as if typed in chat. This is the MQ2 approach.

```
Step 1: Character A targets Character B
  → DLL walks spawn list by name, writes spawn address to PINST_TARGET
  → Or uses SetTarget{spawn_id} command via IPC

Step 2: Character A sends /invite
  → DLL calls InterpretCmd(pLocalPlayer, "/invite")
  → EQ processes the invite, sends it to Character B

Step 3: Character B accepts the invite
  → DLL on Character B calls InterpretCmd(pLocalPlayer, "/invite")
  → In EQ, /invite with no argument while you have a pending invite = accept

Step 4: Verify group formed
  → Read group member data from memory (need group struct offsets from MQ2)
  → Or: check GameState for group-related fields (not yet implemented)
```

#### Method 2: ExecuteCmd by command ID

EQ has numbered command IDs. The invite command ID needs to be extracted from MQ2 headers (`EQ_CMD_INVITE`). This is more robust but requires knowing the exact command ID.

### Sequence diagram

```
Orchestrator                 DLL (Char A)              DLL (Char B)
    │                            │                          │
    │  SetTarget{name="CharB"}   │                          │
    ├───────────────────────────►│                          │
    │                            │ walks spawn list,        │
    │                            │ writes to PINST_TARGET   │
    │                            │                          │
    │  Say{"/invite"}            │                          │
    ├───────────────────────────►│                          │
    │                            │ InterpretCmd("/invite")  │
    │                            │                          │
    │                            │          invite arrives  │
    │                            │         ┌────────────────┤
    │  Say{"/invite"}            │         │                │
    ├──────────────────────────────────────────────────────►│
    │                            │         │  InterpretCmd  │
    │                            │         │  ("/invite")   │
    │                            │         │  = accept      │
    │                            │                          │
    │  [poll GameState — check group fields]                │
    │◄──────────────────────────────────────────────────────┤
```

### Targeting by name (not yet implemented)

The current `TargetingController::set_target(spawn_id)` works by spawn ID. For group invites we need to target by **character name**. Two options:

**Option A: Add `set_target_by_name()` to `TargetingController`**
```rust
pub fn set_target_by_name(&self, name: &str) -> Result<(), TargetError> {
    // Walk spawn list, compare DISPLAYED_NAME at each node
    // When found, write address to PINST_TARGET
}
```

**Option B: Use InterpretCmd with `/target CharName`**
```rust
// Simpler — let EQ handle the name resolution
interpret_cmd(local_player, "/target CharB");
```

**Recommendation:** Option B for initial implementation — it handles partial names, visibility checks, and edge cases that EQ already knows about.

### New IPC commands needed

Add to `dmft-common/src/ipc.rs`:

```rust
pub enum Command {
    // ... existing ...

    /// Execute a slash command string (e.g., "/invite", "/target Foo")
    SlashCommand { text: String },

    /// Invite current target to group
    InviteTarget,

    /// Accept pending group invite
    AcceptGroupInvite,
}
```

### InterpretCmd calling convention

From MQ2 reference, `CEverQuest::InterpretCmd` signature:
```cpp
void InterpretCmd(PlayerClient* pChar, const char* szFullLine);
```

In Rust DLL:
```rust
fn call_interpret_cmd(eq_base: u64, command: &str) -> Result<()> {
    let addr = offsets::rebase(offsets::INTERPRET_CMD, eq_base)
        .ok_or("rebase failed")?;

    let local_player_ptr = /* read from PINST_LOCAL_PLAYER */;

    let cmd_cstr = std::ffi::CString::new(command)?;

    type InterpretCmdFn = unsafe extern "C" fn(*mut c_void, *const c_char);
    let func: InterpretCmdFn = unsafe { std::mem::transmute(addr) };

    unsafe { func(local_player_ptr as *mut c_void, cmd_cstr.as_ptr()); }
    Ok(())
}
```

**Important:** This must be called from the **game loop thread** (inside `on_game_tick`), not from the IPC listener thread. EQ functions are not thread-safe. The IPC listener buffers commands; the game loop thread drains and executes them.

### Full group formation for 36-box

```
For each group of 6:
  1. Designate group leader (first character)
  2. Leader: /target Char2 → /invite → wait 500ms
  3. Char2: /invite (accept)
  4. Leader: /target Char3 → /invite → wait 500ms
  5. Char3: /invite (accept)
  ... repeat for Char4, Char5, Char6
  6. Verify 6-member group via memory read
  7. Move to next group

Total: 6 groups × 5 invites × ~1s each = ~30 seconds
```

---

## 5. Risk Assessment

### High risk

| Risk | Impact | Mitigation |
|------|--------|------------|
| **MAIN_LOOP_OFFSET = 0** | Hook never installs, DLL is inert | Fix before first test (Section 2) |
| **IPC naming mismatch** (client_id vs PID) | Pipe connection fails | Use PID for pipe names (Section 3) |
| **Calling EQ functions from wrong thread** | Crash/UB | Buffer commands in IPC, execute only in game loop |
| **Offset drift between patches** | Hooks crash EQ | Verify offsets match current eqgame.exe build date |

### Medium risk

| Risk | Impact | Mitigation |
|------|--------|------------|
| **Anti-cheat detection** | Account ban | Randomized DLL name already implemented; don't inject during loading screens |
| **InterpretCmd with bad input** | EQ crash | Validate slash commands before passing |
| **Race condition: DLL init vs pipe connect** | Connection refused | Retry with backoff in `setup_ipc()` |
| **Multiple injections** | Double hooks → crash | Check `HookStatus` before injecting |

### Low risk

| Risk | Impact | Mitigation |
|------|--------|------------|
| **Shared memory too small** | Truncated GameState | 64KB is plenty for current data |
| **Pipe buffer overflow** | Dropped commands | 4KB buffers fine for command size |

---

## 6. EQ Function Address Inventory

All addresses from `dmft-common/src/offsets.rs`, preferred base `0x140000000`, client date 2026-03-10.

| Function | Preferred Address | Offset | Confidence | Used for |
|----------|-------------------|--------|------------|----------|
| `PROCESS_GAME_EVENTS` | `0x14028E0F0` | `0x28E0F0` | **Confirmed** (MQ2 live branch) | Main game loop hook |
| `INTERPRET_CMD` | `0x140283FB0` | `0x283FB0` | **Confirmed** | Slash commands (/invite, /target) |
| `EXECUTE_CMD` | `0x1402235B0` | `0x2235B0` | **Confirmed** | Command ID execution |
| `CLICKED_PLAYER` | `0x1402724F0` | `0x2724F0` | **Confirmed** | Click-target |
| `CAST_SPELL` | `0x1400D9F20` | `0x0D9F20` | **Confirmed** | Spell casting |
| `DO_ATTACK` | `0x14031B890` | `0x31B890` | **Confirmed** | Melee attacks |
| `ISSUE_PET_COMMAND` | `0x1402856A0` | `0x2856A0` | **Confirmed** | Pet control |
| `PINST_LOCAL_PLAYER` | `0x140E8E380` | `0xE8E380` | **Confirmed** (live tested) | Local player pointer |
| `PINST_TARGET` | `0x140E8E428` | `0xE8E428` | **Confirmed** (live tested) | Current target pointer |
| `PINST_SPAWN_MANAGER` | `0x140F0CD90` | `0xF0CD90` | **Confirmed** (live tested) | Spawn list traversal |
| Group member struct | Unknown | Unknown | **Unknown** | Group verification |
| EQ_CMD_INVITE ID | Unknown | Unknown | **Unknown** | ExecuteCmd group invite |

### Offsets that need discovery

1. **Group member struct offsets** — needed to verify group formation after invite. Check MQ2 `GroupMember.h` or `PCClient` for group array.
2. **EQ_CMD_INVITE command ID** — for `ExecuteCmd` approach. May be in MQ2 `EQ_Commands.h`. Not required if using `InterpretCmd("/invite")` instead.

---

## 7. Testing Checklist

### Pre-injection (on Windows machine)

- [ ] `cargo build --release` succeeds on Windows
- [ ] `dmft_dll.dll` exists in `target/release/`
- [ ] Single eqgame.exe running, character logged in and in-world

### First injection test

- [ ] Run orchestrator, verify it discovers the EQ process (PID logged)
- [ ] Trigger injection (manually or via TUI command)
- [ ] Check `%TEMP%\dmft\dmft-dll.log` exists and contains init messages
- [ ] Verify "EQ base address resolved" shows correct base
- [ ] Verify "Game loop hook installed" appears (after MAIN_LOOP_OFFSET fix)
- [ ] EQ client still running normally (no crash, no freeze)

### IPC test

- [ ] Verify `\\.\pipe\dmft_cmd_{pid}` exists (`Get-ChildItem \\.\pipe\ | findstr dmft`)
- [ ] Send `Ping` command, receive `Pong` response
- [ ] Verify shared memory `dmft_state_{pid}` is being written (non-zero sequence)
- [ ] Read `GameState` from shared memory — local player name matches in-game

### Group invite test (requires 2+ clients)

- [ ] Client A: `/target CharB` via InterpretCmd — verify target changes
- [ ] Client A: `/invite` via InterpretCmd — verify invite sent
- [ ] Client B: `/invite` via InterpretCmd — verify invite accepted
- [ ] Read group data from memory to confirm group formed

### Stress test (multi-client)

- [ ] 6 clients injected simultaneously — all hooks active
- [ ] All 6 pipes connected and responding to Ping
- [ ] All 6 shared memory regions publishing GameState
- [ ] Full group formed via automated invite sequence
- [ ] No memory leaks after 30+ minutes
