# DLL Hook Specifications and Validation

This document provides formal specifications for all EverQuest DLL hooks used by TextQuest. Hooks intercept EQ internal functions for state capture, control flow, and anti-detection evasion.

**Implementation reference**: `textquest-dll/src/hooks/`

---

## Architecture Overview

TextQuest uses two primary hooking mechanisms:

1. **Hardware Breakpoint (HWBP)** hooks via Vectored Exception Handler (VEH)
   - Uses x86-64 debug registers (DR0-DR3) for execution breakpoints
   - Zero byte modifications — invisible to memory integrity scans
   - Limited to 4 active hooks due to CPU register constraints
   - Ideal for critical main-thread hooks (game loop, chat, state transitions)

2. **Function Detour** hooks via `retour` crate (Windows nightly MSVC only)
   - Inline trampolines that redirect function calls
   - Can hook unlimited functions (no register limit)
   - Used for detours where HWBP is unnecessary or unavailable
   - Requires nightly Rust on Windows (for `retour` unstable features)

---

## Hardware Breakpoint (HWBP) Hooks

### Common Properties

- **Platform**: Windows only (macOS/Linux stubs return `Ok(())`)
- **Register constraint**: 4 total slots (DR0, DR1, DR2, DR3)
- **Handler**: Vectored Exception Handler (VEH) installed once per DLL
- **Execution context**: EQ's main thread (detected and locked on init)
- **Callback signature**: `fn(*mut ()) -> bool` — exception info pointer; return `true` to suppress exception
- **Encryption**: Callbacks live in `.tq` section (remains executable when `.text` is encrypted)
- **Wake/Sleep**: VEH handler calls `crate::stealth::wake()` before and `sleep()` after callback

### HWBP Hook: Game Loop

| Property              | Value                                                                                                     |
| --------------------- | --------------------------------------------------------------------------------------------------------- |
| **Hook name**         | `game_loop`                                                                                               |
| **Target function**   | `CEverQuest::MainLoop`                                                                                    |
| **Target offset**     | `offsets::CEVERQUEST_MAINLOOP` (from `textquest-common/src/offsets.rs`)                                   |
| **Hook type**         | HWBP (Vectored Exception Handler)                                                                         |
| **Slot assignment**   | `HwbpSlot::Dr0`                                                                                           |
| **Parameters**        | None (function takes no parameters)                                                                       |
| **Return type**       | `void`                                                                                                    |
| **Purpose**           | Per-frame update hook for game logic (combat, movement, state polling)                                    |
| **Frequency**         | Every frame (~30 FPS)                                                                                     |
| **Callback**          | `game_loop_callback()` in `hooks/game_loop.rs`                                                            |
| **Installation**      | `hooks::game_loop::install(main_loop_addr)`                                                               |
| **Removal**           | `hooks::game_loop::remove()`                                                                              |
| **Safety invariants** | Must run on main thread; called synchronously; callback must complete quickly (<5ms) to avoid frame drops |
| **Validation**        | Check `hwbp::is_active(HwbpSlot::Dr0)` after install; verify address via pattern scan before hook time    |

**Implementation details**:

- Fires on every call to `MainLoop`, synchronously blocking until callback returns
- Used to dispatch combat rotation, movement, IPC polling, and game state updates
- Must be the first hook installed (slot priority)

---

### HWBP Hook: Chat

| Property                 | Value                                                                                                                                 |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------- |
| **Hook name**            | `chat`                                                                                                                                |
| **Target function**      | `CEverQuest::dsp_chat`                                                                                                                |
| **Target offset**        | `offsets::CEVERQUEST_DSP_CHAT`                                                                                                        |
| **Hook type**            | HWBP (Vectored Exception Handler)                                                                                                     |
| **Slot assignment**      | `HwbpSlot::Dr1`                                                                                                                       |
| **Parameters (x64 ABI)** | RCX = `this` (CEverQuest*); RDX = `text` (const char*); R8 = `color` (int, default 273)                                               |
| **Return type**          | `void`                                                                                                                                |
| **Purpose**              | Capture all in-game text messages (chat, broadcasts, combat messages, system notifications)                                           |
| **Frequency**            | Per message (~10-100/minute depending on activity)                                                                                    |
| **Callback**             | `chat_callback()` in `hooks/chat.rs`                                                                                                  |
| **Installation**         | `hooks::chat::install(dsp_chat_addr)`                                                                                                 |
| **Removal**              | `hooks::chat::remove()`                                                                                                               |
| **Safety invariants**    | Must extract text/color from registers before touching stack; callback must not allocate; must complete in <1ms                       |
| **Validation**           | Verify RDX points to null-terminated ASCII string; verify R8 is valid color index (0-300); check buffered message count stays bounded |

**Implementation details**:

- Reads `text` from RDX and `color` from R8 via exception context
- Buffers messages internally for batch IPC retrieval
- Messages returned via `Command::PollChat` → `Response::ChatBatch`
- No per-message IPC sent; batched to reduce overhead

---

### HWBP Hook: Set Game State

| Property                 | Value                                                                                                                        |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| **Hook name**            | `set_game_state`                                                                                                             |
| **Target function**      | `CEverQuest::SetGameState`                                                                                                   |
| **Target offset**        | `offsets::CEVERQUEST_SETGAMESTATE`                                                                                           |
| **Hook type**            | HWBP (Vectored Exception Handler)                                                                                            |
| **Slot assignment**      | `HwbpSlot::Dr2`                                                                                                              |
| **Parameters (x64 ABI)** | RCX = `this` (CEverQuest\*); RDX = `new_state` (uint32_t)                                                                    |
| **Return type**          | `void`                                                                                                                       |
| **Purpose**              | Detect state transitions (Login → InGame → ZoneLoading) for hook slot rotation                                               |
| **Frequency**            | Per state change (~1-10/session)                                                                                             |
| **Callback**             | `set_game_state_callback()` in `hooks/set_game_state.rs`                                                                     |
| **Installation**         | `hooks::set_game_state::install(setgamestate_addr)`                                                                          |
| **Removal**              | `hooks::set_game_state::remove()`                                                                                            |
| **Safety invariants**    | Must read RDX atomically; must trigger slot rotation before returning; callback must be <500μs                               |
| **Validation**           | Verify state value is valid (0-2); check `HookGameState` enum in `hooks/slot_manager.rs`; verify HWBP slot rotation succeeds |

**Implementation details**:

- Reads new state from RDX; compares against `LAST_GAME_STATE` atomic
- On transition, emits `Response::GameStateChanged` and triggers `HookSlotManager::rotate_hooks()`
- Enables dynamic HWBP slot allocation per game phase (login hooks disabled in-game, combat hooks disabled at login)

---

### HWBP Hook: EQMain (Login)

| Property              | Value                                                                                                       |
| --------------------- | ----------------------------------------------------------------------------------------------------------- |
| **Hook name**         | `eqmain_hook`                                                                                               |
| **Target function**   | `LoginController::GiveTime` (in `eqmain.dll`, not main `eqgame.exe`)                                        |
| **Target offset**     | `offsets::LOGINTROLLER_GIVETIME`                                                                            |
| **Hook type**         | HWBP (Vectored Exception Handler)                                                                           |
| **Slot assignment**   | `HwbpSlot::Dr1` (shared with chat; rotated based on game state)                                             |
| **Parameters**        | None                                                                                                        |
| **Return type**       | `void`                                                                                                      |
| **Purpose**           | Main-thread execution during login phase for credential entry, server selection, character join             |
| **Frequency**         | Every frame during login (~30 FPS)                                                                          |
| **Callback**          | `eqmain_callback()` in `hooks/eqmain_hook.rs`                                                               |
| **Installation**      | `hooks::eqmain_hook::install(givetime_addr)` (during Login state)                                           |
| **Removal**           | `hooks::eqmain_hook::remove()` (on transition to InGame)                                                    |
| **Safety invariants** | Must read atomic action queue before executing UI operations; must not block; callback <10ms                |
| **Validation**        | Verify `LoginController` base address; check action queue atomics for poison state; verify UI thread safety |

**Implementation details**:

- IPC thread queues `LoginAction` (submit credentials, join server) via atomic u8
- Hook callback consumes queue atomically, executes UI operations on main thread
- Replaces background-thread login approach with main-thread pattern matching MQ2's `OnPulse()`
- Requires knowledge of `CEditWnd.InputText` field offset for credential entry

---

## Function Detour Hooks

### Detour Hook: SIDL Screen WND Init

| Property              | Value                                                       |
| --------------------- | ----------------------------------------------------------- |
| **Hook name**         | `hook_sidl_screen_wnd_init`                                 |
| **Target function**   | `CSidlScreenWnd::Initialize` (or variant)                   |
| **Target offset**     | `offsets::SIDL_SCREEN_WND_INIT`                             |
| **Hook type**         | Detour (inline trampoline via `retour`)                     |
| **Parameters**        | `void (*)(this: CSidlScreenWnd*)`                           |
| **Return type**       | `void`                                                      |
| **Purpose**           | UI/screen initialization hook (stub — currently a no-op)    |
| **Installation**      | Called automatically during `detours::install_all(eq_base)` |
| **Removal**           | `detours::remove_all()`                                     |
| **Safety invariants** | Must not modify UI state beyond logging; must complete <1ms |
| **Validation**        | Verify target address is non-zero; log invocation count     |

**Implementation details**:

- Currently a trace-level logging stub
- Placeholder for future UI interception (e.g., wnd resize, theme changes)
- Disabled on non-Windows (stub logs warning)

---

### Detour Hook: CRender Reset Device

| Property              | Value                                                                    |
| --------------------- | ------------------------------------------------------------------------ |
| **Hook name**         | `hook_crender_reset_device`                                              |
| **Target function**   | `CRender::ResetDevice`                                                   |
| **Target offset**     | `offsets::CRENDER_RESET_DEVICE`                                          |
| **Hook type**         | Detour (inline trampoline via `retour`)                                  |
| **Parameters**        | `void (*)(this: CRender*)`                                               |
| **Return type**       | `void`                                                                   |
| **Purpose**           | Device reset hook for graphics state recovery (stub — currently a no-op) |
| **Installation**      | Called automatically during `detours::install_all(eq_base)`              |
| **Removal**           | `detours::remove_all()`                                                  |
| **Safety invariants** | Must not modify device state; must complete <5ms                         |
| **Validation**        | Verify target address is non-zero; log reset count                       |

**Implementation details**:

- Currently a trace-level logging stub
- Placeholder for future graphics resource cleanup or screenshot capture
- Used by DX11 null-render mode to trigger device recreation

---

## Packet Capture Hooks

**Status**: Implemented via static detours (WSASend/WSARecv) but not fully integrated into this spec sheet. See `hooks/packet_hook.rs` for implementation.

| Property           | Value                                                                             |
| ------------------ | --------------------------------------------------------------------------------- |
| **Hook mechanism** | Inline detours (retour static detours)                                            |
| **Targets**        | `ws2_32!WSASend`, `ws2_32!WSARecv`                                                |
| **Purpose**        | Capture network packets before scrambler (send) / after descrambler (recv)        |
| **Installation**   | Not wired into normal DLL startup on current `master`; validation tracked by issue `#1270` |
| **Removal**        | On DLL unload                                                                     |
| **Safety**         | Thread-safe via atomic detour handles                                             |
| **Validation**     | Verify opcode extraction (bytes [2..4] as little-endian u16); check buffer bounds |

---

## Hook Installation Order and Dependencies

### Initialization sequence (from `DllMain` → `init()`)

1. **Stealth initialization** (`stealth::init()`)
   - Page guard setup, PEB unlink, thread pool enumeration, text encryption

2. **EQ base address detection**
   - Pattern scan for EQ module signature; rebase all offsets

3. **Offset validation**
   - Verify critical offsets are non-zero (MainLoop, SetGameState, dsp_chat)

4. **Detour installation** (`detours::install_all(eq_base)`)
   - Install SIDL and CRender detours (non-critical; can fail gracefully)

5. **Fingerprint spoofing** (`fingerprint::init()`)
   - Generate per-client hardware spoofed values from session token

6. **HWBP VEH installation** (automatic on first `hwbp::register()`)
   - One-time `AddVectoredExceptionHandler` call with custom VEH handler

7. **Main-thread detection** (`hwbp::find_main_thread_id()`)
   - Locate EQ window via `FindWindowA("_EverQuestwndclass")`; extract thread ID

8. **Game state detection** (automatic)
   - Read current state from `CEverQuest::GetGameState()`
   - Determine slot assignments via `HookSlotManager`

9. **HWBP hook registration** (state-dependent)
   - **Login state**: Install `eqmain_hook` (GiveTime) on DR1
   - **InGame state**: Install `game_loop` (MainLoop) on DR0, `chat` (dsp_chat) on DR1, `set_game_state` on DR2
   - All via `hwbp::register(slot, address, callback)` with cross-thread `SetThreadContext`

10. **Hook integrity check** (`hooks::integrity::validate()`)
    - Verify all registered slots are consistent (active flag, address, callback all set or all unset)
    - On failure, set `SAFE_MODE` and reject IPC commands

11. **IPC listener startup**
    - Begin accepting `Command` messages from orchestrator

### Shutdown sequence (on DLL unload)

1. `hooks::remove_all()` calls:
   - `detours::remove_all()` — disable inline trampolines
   - `hwbp::remove_all()` — unregister all HWBP slots, remove VEH
   - `fingerprint::remove()` — clear spoofed state (no-op)
   - `chat::remove()` — clear chat buffer
   - `timing::remove()` — disable timing correction
   - `set_game_state::remove()` — unregister SetGameState hook

2. `stealth::deinit()` (if needed)
   - Restore PEB links, unencrypt `.text`

---

## Hook Slot Manager (Dynamic Rotation)

The `HookSlotManager` (`hooks/slot_manager.rs`) allocates the 4 HWBP slots based on game state:

### Login State Slot Plan

| Slot | Hook                     |
| ---- | ------------------------ |
| DR0  | (unused)                 |
| DR1  | `eqmain_hook` (GiveTime) |
| DR2  | (unused)                 |
| DR3  | (unused)                 |

### InGame/ZoneLoading State Slot Plan

| Slot | Hook                            |
| ---- | ------------------------------- |
| DR0  | `game_loop` (MainLoop)          |
| DR1  | `chat` (dsp_chat)               |
| DR2  | `set_game_state` (SetGameState) |
| DR3  | (unused)                        |

**Slot rotation**: On `CEverQuest::SetGameState` transition, the hook manager:

1. Reads new state from RDX
2. Looks up new slot plan in `HookGameState` enum (0=Login, 1=InGame, 2=ZoneLoading)
3. Compares against current assignments
4. Unregisters stale hooks, registers new ones
5. Logs transition: `"HWBP slot plan updated for game state transition"`

**Reserve capacity**: One unused HWBP slot (DR3) always reserved for:

- Ad-hoc rendering hooks (screenshot, screen rotation)
- Anti-cheat detection rotation (periodic rehook on different address)
- Future expansion

---

## Validation Checklist for Each Hook

### Pre-Installation Validation

- [ ] **Offset correctness**: Offset value matches EQ binary version via pattern scan
- [ ] **Address resolution**: `offsets::rebase(preferred_addr, eq_base)` produces non-zero result
- [ ] **Main thread ID**: `hwbp::find_main_thread_id()` succeeds (window exists)
- [ ] **No slot collision**: Target slot is not already active
- [ ] **Callback validity**: Callback function is non-null and safely callable
- [ ] **VEH installed**: `hwbp::is_veh_installed()` returns true (one-time check)

### Post-Installation Validation

- [ ] **Slot active**: `hwbp::is_active(slot)` returns true
- [ ] **Address stored**: `hwbp::get_address(slot) == target_addr`
- [ ] **Callback registered**: `hwbp::has_callback(slot)` returns true
- [ ] **No spurious exceptions**: No `EXCEPTION_SINGLE_STEP` outside expected context
- [ ] **Trampoline integrity**: `hooks::integrity::validate()` passes
- [ ] **IPC commands accepted**: Orchestrator receives `Response::HookReady`

### Runtime Validation (Per-Tick)

- [ ] **Chat buffer bounded**: `chat_buffer.len() < 10000` (prevents OOM)
- [ ] **Game loop latency**: Frame callback completes <5ms (avoid frame drops)
- [ ] **No stack overflow**: VEH handler runs in reserved stack space (Windows default)
- [ ] **Slot consistency**: Active slots never switch without explicit rotation
- [ ] **Safe mode off**: `hooks::integrity::SAFE_MODE` is false unless validation failed

### Uninstall Validation

- [ ] **Slot cleared**: `hwbp::is_active(slot)` returns false
- [ ] **Address zeroed**: `hwbp::get_address(slot) == 0`
- [ ] **Callback null**: `hwbp::has_callback(slot)` returns false
- [ ] **VEH removed** (if no remaining slots): `hwbp::is_veh_installed()` returns false
- [ ] **No lingering exceptions**: No EXCEPTION_SINGLE_STEP on that slot address

---

## Anti-Detection Considerations

### VEH Visibility

- **Windows Defender**: Tolerated — VEH is standard exception handling (used by debuggers, memory managers)
- **Kernelmode scans**: Not detectable at ring-0 (exception handlers are user-mode)
- **Heuristic detectors**: May flag rapid VEH install/removal; TextQuest installs once per DLL load (safe)

### Slot Rotation

`hooks/rotation.rs` provides periodic unhook/rehook to change breakpoint addresses:

- Useful if anti-cheat scans register breakpoint addresses as "always active"
- Rotation window: [5s, 15s] intervals (configurable)
- Executes unhook/rehook on background thread (doesn't block game loop)

### HWBP vs Detour Visibility

- **HWBP**: No code patches — immune to `VirtualProtect` scans, integrity checks
- **Detour**: Inline patches (retour handles this) — may be detectable if anti-cheat walks IAT
- **Strategy**: Prefer HWBP for critical hooks; use detours for non-critical (packet logging)

---

## Debugging and Troubleshooting

### Hook not firing?

1. Verify offset matches current EQ binary version:

   ```rust
   let addr = offsets::rebase(offsets::CEVERQUEST_MAINLOOP, eq_base)?;
   tracing::info!("MainLoop address: {:#x}", addr);
   ```

2. Verify main thread ID:

   ```rust
   let tid = hwbp::find_main_thread_id()?;
   tracing::info!("Main thread TID: {}", tid);
   ```

3. Verify slot not already registered:

   ```rust
   assert!(!hwbp::is_active(HwbpSlot::Dr0));
   ```

4. Check VEH installed:
   ```rust
   assert!(hwbp::is_veh_installed());
   ```

### Frequent exceptions or crashes?

1. Verify callback completes quickly (<5ms for game loop):

   ```rust
   let start = std::time::Instant::now();
   game_loop_callback(/* ... */);
   tracing::info!("Callback took: {:?}", start.elapsed());
   ```

2. Verify slot consistency:

   ```rust
   let report = hooks::integrity::validate();
   if !report.passed {
       tracing::error!("Integrity check failed: {:?}", report);
   }
   ```

3. Enable tracing for VEH dispatch:
   ```rust
   RUST_LOG=textquest_dll::hooks=trace cargo run
   ```

### Chat messages lost?

1. Verify chat buffer not overflowing:

   ```rust
   let buf_size = chat::buffer_size();
   if buf_size > 5000 {
       tracing::warn!("Chat buffer near capacity: {}", buf_size);
   }
   ```

2. Check polling frequency:

   ```rust
   // Orchestrator should poll every tick (<33ms)
   // If poll frequency drops, messages may be dropped on buffer wraparound
   ```

3. Verify text pointer extraction from RDX:
   ```rust
   let text_ptr = context.Rdx as *const u8;
   let text = std::ffi::CStr::from_ptr(text_ptr as *const i8)?;
   tracing::debug!("Chat text: {}", text.to_string_lossy());
   ```

---

## References

- **HWBP implementation**: `textquest-dll/src/hooks/hwbp.rs`
- **Slot management**: `textquest-dll/src/hooks/slot_manager.rs`
- **Integrity validation**: `textquest-dll/src/hooks/integrity.rs`
- **Detour system**: `textquest-dll/src/hooks/detours.rs`
- **Offset database**: `textquest-common/src/offsets.rs`
- **Stealth module**: `textquest-dll/src/stealth/` (VEH, text encryption, page guard)
- **IPC protocol**: `textquest-common/src/ipc.rs` (Command/Response enums)
- **Detection research**: `docs/research/hook-detection-surface.md`
- **Anti-cheat evasion patterns**: `docs/research/veh-obfuscation.md` (if available)

---

## Revision History

| Date       | Version | Author      | Notes                                       |
| ---------- | ------- | ----------- | ------------------------------------------- |
| 2026-04-14 | 1.0     | Claude Code | Initial specification; all hooks documented |
