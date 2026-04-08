# Login Chain Hardening Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the login automation chain so Phase 1 (credential entry + login click), Phase 2 (server select), and Phase 3 (character select) complete reliably on a live EQ client.

**Architecture:** The login chain spans 3 threads (IPC listener, login-phase2 background, game loop) and 2 EQ modules (eqmain.dll for login/server select, eqgame.exe for character select). The core bug pattern is using `queue_button_click()` during eqmain phases where `ProcessGameEvents` isn't hooked — the queue never drains. The fix introduces a phase-aware click helper and proper screen-transition detection.

**Tech Stack:** Rust, Windows API (PostMessageW, GetModuleHandleW), EQ CXWnd vtable manipulation, tracing for structured logging.

---

## File Structure

| File                                 | Responsibility                     | Action                                                    |
| ------------------------------------ | ---------------------------------- | --------------------------------------------------------- |
| `textquest-dll/src/eq/widgets.rs`    | Phase-aware click helper           | Modify: add `click_button_for_phase()`                    |
| `textquest-dll/src/ipc/mod.rs`       | IPC handler + Phase 2/3 polling    | Modify: use new click helper, improve screen detection    |
| `textquest-dll/src/login/widgets.rs` | Credential writing + Phase 1 click | Modify: use new click helper                              |
| `textquest-dll/src/login/mod.rs`     | Login FSM tick functions           | Modify: use new click helper in `tick_selecting_server()` |
| `textquest-common/src/offsets.rs`    | Offset constants                   | Read-only reference                                       |

---

### Task 1: Add Phase-Aware Click Helper

**Why this matters:** The root cause of the login bug is calling `queue_button_click()` during eqmain phases where the game loop isn't active. This has happened twice (Phase 1 login button, Phase 2 PLAY EVERQUEST button). A single helper function that picks the right strategy based on whether eqmain.dll is loaded prevents this entire class of bugs.

**Files:**

- Modify: `textquest-dll/src/eq/widgets.rs:440-477`
- Test: `textquest-dll/src/eq/widgets.rs` (in-file `#[cfg(test)]` module)

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)]` module in `textquest-dll/src/eq/widgets.rs`:

```rust
#[test]
#[cfg(not(windows))]
fn click_button_for_phase_noop_on_non_windows() {
    // On macOS, both paths are no-ops — just verify no panic.
    unsafe { click_button_for_phase(0, true) };
    unsafe { click_button_for_phase(0, false) };
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p textquest-dll click_button_for_phase_noop`
Expected: FAIL with "cannot find function `click_button_for_phase`"

- [ ] **Step 3: Implement the phase-aware click helper**

Add after `click_button_via_vtable` (around line 477) in `textquest-dll/src/eq/widgets.rs`:

```rust
/// Click a button, choosing the correct mechanism for the current EQ phase.
///
/// During eqmain (login, server select): calls `click_button_via_vtable()` directly
/// because ProcessGameEvents is not hooked yet and queue_button_click() would never drain.
///
/// During eqgame (character select, in-world): uses `queue_button_click()` to schedule
/// the click on the game loop thread where it's safe to manipulate EQ UI state.
///
/// # Safety
/// `button_wnd` must be a valid `CXWnd` pointer with an intact vtable.
pub unsafe fn click_button_for_phase(button_wnd: usize, in_eqmain: bool) {
    if button_wnd == 0 {
        tracing::warn!("click_button_for_phase: null button pointer");
        return;
    }
    if in_eqmain {
        tracing::debug!(
            ptr = format!("{:#x}", button_wnd),
            "Direct vtable click (eqmain phase)"
        );
        click_button_via_vtable(button_wnd);
    } else {
        tracing::debug!(
            ptr = format!("{:#x}", button_wnd),
            "Queueing click on game loop (eqgame phase)"
        );
        crate::hooks::game_loop::queue_button_click(button_wnd);
    }
}

#[cfg(not(windows))]
pub unsafe fn click_button_for_phase(_button_wnd: usize, _in_eqmain: bool) {}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p textquest-dll click_button_for_phase_noop`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add textquest-dll/src/eq/widgets.rs
git commit -m "feat(login): add click_button_for_phase() phase-aware click helper

Prevents the class of bugs where queue_button_click() is used during
eqmain phases. The helper picks direct vtable click (eqmain) vs game
loop queue (eqgame) based on a boolean flag."
```

---

### Task 2: Wire Phase 1 (Login Button) to New Helper

**Why this matters:** Phase 1's login button click was fixed in commit 0ec24b77d to use direct vtable click. Migrating it to the new helper makes the intent explicit and prevents future regressions.

**Files:**

- Modify: `textquest-dll/src/login/widgets.rs:613-629`

- [ ] **Step 1: Replace direct vtable call with phase-aware helper**

In `textquest-dll/src/login/widgets.rs`, replace the login button click block (around lines 613-629):

```rust
            // Click the Login button using phase-aware helper.
            // During eqmain, this calls click_button_via_vtable() directly
            // (ProcessGameEvents is NOT hooked yet, queue would never drain).
            if login_button != 0 {
                // Small delay to let credential writes settle before clicking.
                std::thread::sleep(std::time::Duration::from_millis(150));
                tracing::info!(
                    ptr = format!("{:#x}", login_button),
                    "Clicking Login button (eqmain phase)"
                );
                crate::eq::widgets::click_button_for_phase(login_button, true);
                tracing::info!("Login button clicked");
            } else {
                tracing::warn!("Login button not found — credentials written but not submitted");
            }
```

- [ ] **Step 2: Build and run tests**

Run: `cargo build && cargo test -p textquest-dll`
Expected: Build succeeds, all DLL tests pass.

- [ ] **Step 3: Commit**

```bash
git add textquest-dll/src/login/widgets.rs
git commit -m "refactor(login): use click_button_for_phase for Phase 1 login button"
```

---

### Task 3: Wire Phase 2 (PLAY EVERQUEST) to New Helper + Screen Guard

**Why this matters:** Phase 2 had two bugs: (1) `queue_button_click()` never drains during eqmain, and (2) "PLAY EVERQUEST!" can be a false positive on the login screen. The SIDL guard was added in commit 6c42cda73; this task migrates to the new helper.

**Files:**

- Modify: `textquest-dll/src/ipc/mod.rs:219-288`

- [ ] **Step 1: Replace queue_button_click with phase-aware helper**

In `textquest-dll/src/ipc/mod.rs`, in the `login_chain_phase2()` function, find the PLAY EVERQUEST click section and ensure it uses the new helper:

```rust
        if let Some(play_btn) = find_button_by_text(eqmain_base, "PLAY EVERQUEST!") {
            tracing::info!(
                ptr = format!("{:#x}", play_btn),
                attempt,
                "Phase 2: Found PLAY EVERQUEST! on server select screen"
            );
            // Phase-aware click — eqmain is still loaded, so direct vtable.
            std::thread::sleep(std::time::Duration::from_millis(150));
            unsafe { crate::eq::widgets::click_button_for_phase(play_btn, true) };
            // Also press Enter via PostMessage as backup
            std::thread::sleep(std::time::Duration::from_millis(200));
            crate::login::widgets::simulate_enter_key(eqmain_base);
            tracing::info!("Phase 2: PLAY EVERQUEST clicked + Enter");
            found = true;
            break;
        }
```

- [ ] **Step 2: Build and run tests**

Run: `cargo build && cargo test -p textquest-dll`
Expected: Build succeeds, all DLL tests pass.

- [ ] **Step 3: Commit**

```bash
git add textquest-dll/src/ipc/mod.rs
git commit -m "refactor(login): use click_button_for_phase for Phase 2 PLAY EVERQUEST"
```

---

### Task 4: Wire FSM Server Select to New Helper

**Why this matters:** The LoginFsm's `tick_selecting_server()` function (login/mod.rs) is a second code path that clicks buttons during server select. It must also use the phase-aware helper.

**Files:**

- Modify: `textquest-dll/src/login/mod.rs` (in `tick_selecting_server()`, around line 436)

- [ ] **Step 1: Find all click_button / click_button_via_vtable calls in tick_selecting_server**

Search for button click calls in the `tick_selecting_server()` function:

```bash
grep -n "click_button\|queue_button" textquest-dll/src/login/mod.rs
```

- [ ] **Step 2: Replace with phase-aware helper**

Each button click in `tick_selecting_server()` should use `click_button_for_phase(ptr, true)` since server select is still eqmain territory.

In `textquest-dll/src/login/mod.rs`, in `tick_selecting_server()`, replace any `click_button()` or `click_button_via_vtable()` calls:

```rust
// Server select is eqmain territory — use phase-aware click
unsafe { crate::eq::widgets::click_button_for_phase(btn_ptr, true) };
```

- [ ] **Step 3: Build and run tests**

Run: `cargo build && cargo test -p textquest-dll`
Expected: Build succeeds, all DLL tests pass.

- [ ] **Step 4: Commit**

```bash
git add textquest-dll/src/login/mod.rs
git commit -m "refactor(login): use click_button_for_phase in FSM tick_selecting_server"
```

---

### Task 5: Add Transition Logging Between Phases

**Why this matters:** The Discord debugging session was difficult because we couldn't tell which screen EQ was actually on. Adding structured tracing at every phase transition makes future debugging trivial.

**Files:**

- Modify: `textquest-dll/src/ipc/mod.rs` (login_chain_phase2)

- [ ] **Step 1: Add screen-state logging to Phase 2 polling**

In `login_chain_phase2()`, add a log line every 5 seconds that reports the current detected screen state:

```rust
        // Log screen state every 5s for debugging
        if attempt % 10 == 0 {
            let has_serverselect = crate::login::widgets::is_sidl_window_visible(
                eqmain_base, "serverselect"
            );
            let has_connect = crate::login::widgets::is_sidl_window_visible(
                eqmain_base, "connect"
            );
            tracing::info!(
                attempt,
                has_serverselect,
                has_connect,
                eqmain_base = format!("{:#x}", eqmain_base),
                "Phase 2: screen state check"
            );
        }
```

- [ ] **Step 2: Add screen-state logging to Phase 3 polling**

In the Phase 3 section of `login_chain_phase2()`, add similar logging:

```rust
        // Log screen state every 10s for debugging
        if attempt % 20 == 0 {
            tracing::info!(
                attempt,
                eqmain_base = format!("{:#x}", eqmain_base),
                "Phase 3: eqmain still loaded, waiting for character select"
            );
        }
```

- [ ] **Step 3: Build and run tests**

Run: `cargo build && cargo test -p textquest-dll`
Expected: Build succeeds, all DLL tests pass.

- [ ] **Step 4: Commit**

```bash
git add textquest-dll/src/ipc/mod.rs
git commit -m "feat(login): add screen-state tracing to Phase 2/3 polling loops"
```

---

### Task 6: Live Verification on Frostreaver

**Why this matters:** All fixes are meaningless without live verification. This task ensures Kara pulls clean master and tests the full chain.

**Files:**

- Read: DLL log at `%TEMP%\textquest\textquest-dll.log` on Frostreaver

- [ ] **Step 1: Ensure Frostreaver is on clean master**

On Frostreaver, run:

```bash
cd /path/to/TextQuest
git stash  # Stash any of Kara's local changes
git checkout master
git pull origin master
```

Verify: `git log --oneline -5` should show the login fix commits:

- `6c42cda73` fix(login): use direct vtable click for Phase 2 PLAY EVERQUEST button
- `0ec24b77d` fix(login): click login button directly during eqmain phase
- `1936c4b07` fix(login): scan for both LOGIN candidates before breaking

- [ ] **Step 2: Release build**

```bash
cargo build --release
```

Expected: Clean build, no warnings related to login.

- [ ] **Step 3: Launch EQ and inject DLL**

```bash
# Launch EQ with patchme flag
# Inject DLL into eqgame.exe
# Send StartLogin IPC command
```

- [ ] **Step 4: Verify Phase 1 in DLL log**

Check `%TEMP%\textquest\textquest-dll.log` for:

```
Found Login button ptr="0x..." candidates=2
Clicking Login button (eqmain phase)
Login button clicked
```

Then verify on screen: EQ should advance past the login screen.

- [ ] **Step 5: Verify Phase 2 in DLL log**

Check for:

```
Phase 2: screen state check has_serverselect=true
Phase 2: Found PLAY EVERQUEST! on server select screen
Phase 2: PLAY EVERQUEST clicked + Enter
```

Then verify on screen: EQ should advance to character select.

- [ ] **Step 6: Verify Phase 3 in DLL log**

Check for:

```
Phase 3: eqmain.dll unloaded — at character select
```

- [ ] **Step 7: Report results**

If all phases pass: login chain is confirmed working.
If any phase fails: capture the full DLL log and report which phase + what's on screen.

---

## Bug Reference

| Bug                                                    | Commit      | Root Cause                                                       | Fix                                    |
| ------------------------------------------------------ | ----------- | ---------------------------------------------------------------- | -------------------------------------- |
| Phase 1: Scan finds wrong LOGIN button                 | `1936c4b07` | Early-break after 1 candidate, needs 2                           | Break at `candidates.len() >= 2`       |
| Phase 1: Login click queued but never executes         | `0ec24b77d` | `queue_button_click` during eqmain, ProcessGameEvents not hooked | Direct `click_button_via_vtable`       |
| Phase 2: PLAY EVERQUEST click never executes           | `6c42cda73` | Same queue bug as Phase 1                                        | Direct vtable + SIDL guard             |
| Phase 2: False positive PLAY EVERQUEST on login screen | `6c42cda73` | No screen detection before clicking                              | Check `serverselect` SIDL window first |

## Thread Model Reference

```
IPC Thread ──────────────── Login Phase2 Thread ──────── Game Loop Thread
    │                              │                           │
    │ StartLogin received          │                           │
    │ ├─ type_credentials_to_window│                           │
    │ │  ├─ write CXStr fields     │                           │
    │ │  └─ click_button_for_phase │                           │
    │ │     (in_eqmain=true)       │                           │
    │ │     → click_button_via_vtable                          │
    │ ├─ start_login(FSM)          │                           │
    │ └─ spawn ────────────────────┤                           │
    │                              │ Phase 2: poll serverselect│
    │                              │ ├─ click_button_for_phase │
    │                              │ │  (in_eqmain=true)       │
    │                              │ │  → click_button_via_vtable
    │                              │ Phase 3: poll eqmain unload
    │                              │ └─ exit when eqmain == 0  │
    │                              │                           │ FSM tick fires
    │                              │                           │ ├─ queue_enter_world
    │                              │                           │ │  (atomics)
    │                              │                           │ ├─ SelectCharacter
    │                              │                           │ └─ EnterWorld
    │                              │                           │    → InWorld
```
