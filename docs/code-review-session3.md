# Code Review — Session 3 (2026-03-28)

Scope: Full codebase review covering dmft, dmft-common, dmft-dll.
Reviewed: `cargo clippy` output + manual inspection of TUI, spawn reading, game loop hook, DLL init, IPC pipe, offsets, and crypto.

---

## Critical Issues

### C1 — Spawn list selection goes off-screen (rendering bug)
**File:** `dmft/src/tui/app.rs:124`, `dmft/src/tui/ui.rs:857-903`
**Confidence:** 90

`App` has a `spawn_scroll: usize` field but it is **never read or updated**. `draw_spawn_list` builds all filtered rows into a `Vec<Row>` and calls `frame.render_widget(table, area)` (stateless render). Ratatui truncates rows to the visible area height. The per-row highlight style (`bg(Color::DarkGray)`) is applied at row index `i == app.spawn_selected`, but if `spawn_selected` exceeds visible rows (~20-25), the highlight is off-screen and invisible. Navigation with arrows works internally (counter advances correctly) but the user sees no highlighted row and can't tell where they are.

**Fix:** Compute a scroll offset (`scroll = spawn_selected.saturating_sub(visible_height - 1)`) and slice `filtered[scroll..]` before building rows, OR switch to `render_stateful_widget` with `TableState::with_selected`.

---

### C2 — `shutdown()` performs heavy work under the loader lock
**File:** `dmft-dll/src/lib.rs:198-203`
**Confidence:** 85

`DllMain` with `DLL_PROCESS_DETACH` calls `shutdown()` directly:

```rust
DLL_PROCESS_DETACH => {
    super::shutdown();
    TRUE
}
```

`shutdown()` calls `ipc::stop()` and `hooks::remove_all()`. Both of these touch OS synchronization primitives (mutex locks on `SHARED_WRITER`, `PENDING_COMMANDS`, retour hook disable). `DLL_PROCESS_DETACH` runs under the Windows loader lock — taking other locks here is a classic deadlock scenario. The comment in the function says "must be minimal — just set the flag" but the code does much more.

**Fix:** `DLL_PROCESS_DETACH` should only do `SHUTTING_DOWN.store(true, Ordering::SeqCst)`. Hook removal and IPC teardown must happen before this via the eject command path.

---

### C3 — Named pipe server doesn't disconnect between connections
**File:** `dmft-dll/src/ipc/pipe.rs:83-143`, `dmft-dll/src/ipc/mod.rs:125-144`
**Confidence:** 80

After `receive()` successfully reads one command, `listener_loop` goes back to the top and calls `receive()` again. `receive()` starts with `ConnectNamedPipe(self.handle, None)`. On Windows, calling `ConnectNamedPipe` on a handle that is **already connected** fails with `ERROR_PIPE_CONNECTED`. This error propagates as `Err(...)`, triggers `reset_auth()` in the listener loop, and then immediately retries — creating a tight error loop. No `DisconnectNamedPipe` is called in the success path.

The design intent seems to be a persistent single-connection pipe, but the implementation doesn't disconnect/reconnect cleanly.

**Fix:** After successfully processing a command (or after a command error that doesn't indicate shutdown), call `DisconnectNamedPipe(self.handle)` before looping back to `ConnectNamedPipe`.

---

## Important Issues

### I1 — Panic leaves terminal in raw mode
**File:** `dmft/src/tui/run.rs:22-38`
**Confidence:** 90

```rust
let result = run_loop(&mut terminal, &mut app);
// Restore terminal — always runs even if loop panicked
disable_raw_mode()?;
```

The comment is wrong: in Rust, code after a panicking call does **not** run. If `run_loop` or any callback panics, the terminal is left in raw mode and alternate screen, making the terminal unusable after exit.

**Fix:** Register a panic hook before `run_loop` that calls `disable_raw_mode()` and `LeaveAlternateScreen`, or use a guard struct with `Drop` that restores the terminal.

---

### I2 — `generate_session_token` is predictable
**File:** `dmft-dll/src/lib.rs:186-193`
**Confidence:** 85

```rust
fn generate_session_token(pid: u32) -> SessionToken {
    let pid_bytes = pid.to_le_bytes();
    let mut token = [0u8; 32];
    for (i, byte) in token.iter_mut().enumerate() {
        *byte = pid_bytes[i % 4] ^ (i as u8);
    }
    token
}
```

PID is public information visible to all users on the system. Any local process that knows the EQ client's PID (trivial via `tasklist`) can reconstruct this token exactly and authenticate to the pipe. The code comment says "In production, the token is provided by the orchestrator during injection" — but this placeholder is active code.

**Fix:** Either pass the token from the orchestrator via the injection payload (write it into shared memory before injection completes), or generate it with `OsRng` inside the DLL and have the orchestrator read it from shared memory after injection. The token must not be derivable from public data.

---

### I3 — `decrypt()` panics on wrong nonce length
**File:** `dmft/src/credentials/crypto.rs:52-58`
**Confidence:** 85

```rust
pub fn decrypt(ciphertext: &[u8], key: &[u8; 32], nonce: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce);  // panics if nonce.len() != 12
```

`Nonce::from_slice` panics via `assert_eq!` if the slice is not exactly 12 bytes. The `nonce` argument is `&[u8]` which can be any length. A corrupted credential database row with a wrong-length nonce blob would crash the process instead of returning an error.

**Fix:** Validate nonce length before calling `from_slice`:
```rust
anyhow::ensure!(nonce.len() == 12, "invalid nonce length: {}", nonce.len());
```
Same issue applies to `Key::from_slice` in `encrypt`/`decrypt` if the key argument were ever passed incorrectly (currently it's `&[u8; 32]` so compile-time safe, but worth noting).

---

### I4 — Named pipe has default (world-accessible) security descriptor
**File:** `dmft-dll/src/ipc/pipe.rs:46-57`
**Confidence:** 80

```rust
// TODO(security-H1): Add restrictive security descriptor to limit pipe access
// to the current process SID.
let handle = unsafe {
    CreateNamedPipeA(
        ...
        None, // default security
    )
}?;
```

The default security descriptor on a named pipe allows any authenticated user on the system to open it. With 36 EQ clients running, the pipe names (`\pipe\dmft_cmd_<pid>`) are predictable. The session token is the only guard, and as noted in I2 it is currently predictable. Together these form a local privilege escalation path.

**Fix:** Pass an explicit `SECURITY_ATTRIBUTES` that restricts pipe access to the current process SID (i.e., only the orchestrator process running as the same user can connect).

---

### I5 — `STANDSTATE` offset is known-wrong for sitting characters
**File:** `dmft-common/src/offsets.rs:165`
**Confidence:** 95 (known, documented)

```rust
/// TODO: 0x0574 reads 110 (FD) when character is sitting on March 10, 2026 build.
/// Needs hex dump calibration scan on frostreaver to find correct offset.
pub const STANDSTATE: usize = 0x0574;
```

`StandState::from_id(stand_state_id)` will map `0xFD = 253` to `StandState::Unknown(253)` rather than `Sitting`. The stand state column in the TUI shows incorrect values for all non-standing characters. This directly affects the combat FSM's ability to detect sitting/feigned states.

**Fix:** Run offset calibration scan on live EQ client. The correct value for Sitting is 2 (from EQ source: `Standing=0, Frozen=1, Looting=2, Sitting=3, Ducking=4, Feigned=110, Dead=111`). The raw value 0xFD suggests a sign-extension issue or wrong field — the offset may still be wrong.

---

## Minor Issues

### M1 — Deprecated AES-GCM API (4 warnings)
**File:** `dmft/src/credentials/crypto.rs:38,42,53,54`
**Confidence:** 100 (clippy confirms)

`Key::from_slice` and `Nonce::from_slice` from `generic-array` 0.x are deprecated. No functional impact, but the warnings will grow. Upgrade `aes-gcm` to a version using `generic-array` 1.x to resolve.

---

### M2 — Render skip infrastructure built but never used
**File:** `dmft-dll/src/hooks/game_loop.rs:68-137`
**Confidence:** 95

`WINDOW_IS_FOREGROUND` is updated every 30 ticks and `is_foreground()` is exported, but nothing calls `is_foreground()` to actually skip rendering. The `CDisplay::RealRender_World` hook doesn't exist yet. This is expected TODO work, but worth flagging since it's described in the PR as "foreground detection and render skipping" — the detection part is done, the skipping part isn't.

---

### M3 — `filtered_spawns()` called multiple times per frame
**File:** `dmft/src/tui/app.rs:269-287`
**Confidence:** 70 (performance, not correctness)

`spawn_list_down()`, `spawn_list_up()`, `spawn_list_page_down()`, `spawn_list_page_up()`, and `spawn_list_end()` each call `self.filtered_spawns()` to compute the maximum index. With 200+ spawns, 36 clients, and text filtering active, this iterates and filters the full spawn list on every keypress. Consider caching the filtered list or the count in `App` state.

---

### M4 — Demo data loads on every frame until clients exist (macOS)
**File:** `dmft/src/tui/run.rs:182-186`
**Confidence:** 80

```rust
if app.clients.is_empty() {
    load_demo_data(app);
}
```

`load_demo_data` pushes 6 demo clients on first call. On subsequent refreshes, `clients` is non-empty so it's skipped. However, this means if all clients are removed (hypothetically), demo data would be re-appended creating duplicates. The real risk is that the guard `is_empty()` ties demo behavior to client count, not to a "demo mode" flag. Not a current bug, but fragile.

---

### M5 — `spawn_info_lines` signature claims `'static` lifetime
**File:** `dmft/src/tui/ui.rs:664`
**Confidence:** 75

```rust
fn spawn_info_lines(spawn: &SpawnInfo, app: &App) -> Vec<Line<'static>> {
```

All `String` values are owned so this compiles, but the function signature is misleading — future refactors that try to return borrowed spans will fail unexpectedly. Consider returning `Vec<Line<'_>>` with a bound on the input lifetimes.

---

### M6 — No test coverage for IPC pipe handshake path
**File:** `dmft-dll/src/ipc/pipe.rs`

`constant_time_eq`, `validate_command`, and `CommandListener::new` have zero tests. Given the security-critical nature of the pipe authentication, these should have unit tests covering: correct token accepts, wrong token rejects, `token_bytes_read != 32` rejects.

---

## Clippy Summary (from `cargo clippy 2>&1`)

All 15 clippy warnings are non-blocking style suggestions. Notably:

| Warning | File | Fix |
|---------|------|-----|
| `question_mark` | `dmft-dll/src/combat/classes/cleric.rs:35` | `?` operator |
| `unnecessary_map_or` | `dmft-dll/src/combat/holyshit.rs:43` | `.is_some_and()` |
| `manual_is_multiple_of` | `dmft-dll/src/hooks/game_loop.rs:84` | `.is_multiple_of(30)` |
| `collapsible_if` (×3) | `dmft-dll`, `dmft/src/soul`, `dmft/src/tui/app.rs` | flatten with `&&` |
| `manual_div_ceil` | `dmft/src/tui/sprites.rs:19` | `.div_ceil(2)` |
| `needless_range_loop` | `dmft/src/tui/sprites.rs:34` | use `.iter()` |
| `useless_vec` | `dmft/src/tui/run.rs:246` | use array literal |
| 4× `deprecated` | `dmft/src/credentials/crypto.rs` | upgrade generic-array |

---

## Priority Order

| # | Issue | Severity | Effort |
|---|-------|----------|--------|
| C1 | Spawn list doesn't scroll | High | Low (add scroll offset) |
| C2 | `shutdown()` heavy work under loader lock | High | Low (move to eject path) |
| I1 | Panic leaves terminal in raw mode | Medium | Low (Drop guard) |
| I3 | `decrypt()` panics on bad nonce length | Medium | Trivial (one `ensure!`) |
| I5 | `STANDSTATE` offset calibration | Medium | Requires live EQ test |
| C3 | Pipe doesn't disconnect between connections | Medium | Low (add `DisconnectNamedPipe`) |
| I2 | Session token predictable from PID | Medium | Medium (injection payload) |
| I4 | Pipe default security descriptor | Low (local attacker) | Medium |
| M1 | Deprecated crypto API warnings | Low | Trivial (dep upgrade) |
| M2 | Render skip not connected | Info | Planned future work |
