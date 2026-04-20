# Team A — Code Quality & Bugs Audit

Generated: 2026-04-19
Base: 00c452e69, diff window: last 2 weeks

## Summary

- 14 findings (1 critical / 3 high / 7 medium / 3 low)
- Hottest file: `textquest-dll/src/stealth/page_encrypt.rs`, `textquest/src/tui/app.rs`, `textquest-web/src/main.rs`
- Top concern: page_encrypt VEH handler races with cleanup via dangling raw pointer — potential use-after-free in the Windows DLL under anti-cheat memory scan

---

## Findings

### [CRITICAL] VEH handler bypasses Mutex, races with cleanup — potential use-after-free

- **File**: `textquest-dll/src/stealth/page_encrypt.rs:100-134` (init at 187-195)
- **Problem**: `MANAGER_PTR` stores a raw `*mut PageEncryptionManager` pointing inside a `Mutex<Option<...>>`. The VEH handler (`veh_handler`) reads `MANAGER_PTR` without holding the mutex and casts it to a mutable reference. Concurrently, `cleanup()` sets `MANAGER_PTR=0`, drops `guard`, and deallocates the inner `PageEncryptionManager`. A VEH that fires between the `MANAGER_PTR=0` store and the drop in `cleanup()` — or before `MANAGER_PTR.store` but after the guard releases — can dereference a dangling pointer.
- **Evidence**:

```rust
// init (holds guard):
let mgr_ref = guard.as_mut().unwrap() as *mut PageEncryptionManager;
MANAGER_PTR.store(mgr_ref as usize, Ordering::Release);
// guard released here (end of block) ← VEH can now race

// veh_handler (no guard, interrupt-time):
let mgr_ptr = MANAGER_PTR.load(Ordering::Acquire);
let mgr = &mut *(mgr_ptr as *mut PageEncryptionManager); // UAF if cleanup raced
```

- **Fix sketch**: The VEH handler cannot acquire a `Mutex`. Use an `Arc<PageEncryptionManager>` stored in a `static AtomicUsize` (pointer to `Arc` innards) with `ManuallyDrop` to extend lifetime, and an `AtomicBool` "live" gate that the VEH checks first. Or restructure so `MANAGER` is leaked (`Box::into_raw` + never freed) and an `AtomicBool` gates active state — simpler and correct for a DLL lifetime.
- **Severity rationale**: Use-after-free in a VEH that runs at interrupt priority in the game's address space. Exploitable for code execution or crash during cleanup.

---

### [HIGH] `constant_time_eq_str` leaks token length via early return — timing oracle

- **File**: `textquest-web/src/main.rs:185-190`
- **Problem**: The length equality check `if ab.len() != bb.len() { return false; }` returns before the constant-time XOR loop, allowing an attacker measuring response time to enumerate the exact length of `TEXTQUEST_API_TOKEN`. Once length is known, a brute-force attack becomes much cheaper.
- **Evidence**:

```rust
fn constant_time_eq_str(a: &str, b: &str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {   // ← leaks token length
        return false;
    }
    ab.iter()
        .zip(bb.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}
```

- **Fix sketch**: Use `subtle::ConstantTimeEq` from the `subtle` crate, or pad both inputs to a fixed maximum length before XOR-folding. The standard pattern is `subtle::ConstantTimeEq::ct_eq(ab, bb).into()`.
- **Severity rationale**: The API token is the sole auth mechanism protecting all `/api` routes including account passwords and IPC commands. Length leak plus repeated probing reduces brute-force complexity significantly.

---

### [HIGH] Blocking HTTP call (`reqwest::blocking`) on the TUI render thread — freeze risk

- **File**: `textquest/src/tui/app.rs:3501-3525` (called from `update_gm_detection` at `3421`)
- **Problem**: `sync_gm_state_to_web` creates a `reqwest::blocking::Client` and calls `.send()` synchronously with a 2-second timeout, inside what appears to be `update_gm_detection` which is called from the TUI event loop (single-threaded). A slow or unreachable web server will freeze the TUI for up to 2 seconds per GM event, blocking all keyboard input and rendering during that window.
- **Evidence**:

```rust
let client = match reqwest::blocking::Client::builder()
    .timeout(std::time::Duration::from_secs(2))
    .build() { ... };
match client.post(&url).json(&payload).send() { ... }
```

- **Fix sketch**: Move the sync to a background `tokio::spawn` task or a dedicated sender thread. The simplest fix is to send the payload via the existing `tokio` runtime using `tokio::runtime::Handle::current().spawn(async { ... })` — the web server is already async.
- **Severity rationale**: 2-second freeze every time a GM enters the zone is operationally wrong — the TUI is the control plane. The automation-paused state may also be delayed.

---

### [HIGH] `CARGO_MANIFEST_DIR` baked into production data paths — all deployments break

- **File**: `textquest-web/src/main.rs:195-230`
- **Problem**: Seven `PathBuf::from(env!("CARGO_MANIFEST_DIR"))` calls resolve data paths at compile time relative to the source tree. The compiled binary will hardcode e.g. `/home/user/dev/TextQuest/textquest-web/../data/credentials.db`. Any deployment outside the exact build directory (Frostreaver release build, CI artifact, different user) silently fails to find credentials, config files, and the SPA dist.
- **Evidence**:

```rust
fn credentials_db_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/credentials.db")
}
```

- **Fix sketch**: Replace with `std::env::current_exe()?.parent()?.join("../data/credentials.db")` or an env-var override (`TEXTQUEST_DATA_DIR`). The SPA path should similarly use `current_exe` parent or an env var.
- **Severity rationale**: Deployed binary will not find credential store, silently fall back to 501 for all password routes, and serve 404 for the frontend. Silent failure on a fresh Windows deploy.

---

### [MEDIUM] `PacketMonitorState::push` uses `Vec::remove(0)` — O(n) eviction at 10k capacity

- **File**: `textquest/src/tui/state.rs:1944-1947`
- **Problem**: When the 10,000-packet buffer is full, `remove(0)` shifts all 9,999 remaining elements on every new packet. Under sustained packet capture (EQ generates hundreds per second), this degrades into O(n²) work per second on the render thread.
- **Evidence**:

```rust
pub fn push(&mut self, record: PacketRecord) {
    if self.packets.len() >= self.capacity {
        self.packets.remove(0);  // O(n) shift
    }
    self.packets.push(record);
}
```

- **Fix sketch**: Change `packets: Vec<PacketRecord>` to `VecDeque<PacketRecord>` and use `pop_front()` instead of `remove(0)`. No API changes needed — `iter()`, `len()`, `push_back()` are identical.
- **Severity rationale**: Measurable TUI lag at high packet rates. `VecDeque` is a trivial fix.

---

### [MEDIUM] `update_gm_detection` defined but never called — GM detection silently dead

- **File**: `textquest/src/tui/app.rs:3372`
- **Problem**: `pub fn update_gm_detection` exists and contains the full GM event dispatch, Discord alerting, and `sync_gm_state_to_web` logic, but no caller invokes it anywhere in the codebase. GM detection is effectively disabled in the running TUI.
- **Evidence**: Grepping the entire worktree for `update_gm_detection` returns only the definition. The run loop (`run.rs`) has no reference to it.
- **Fix sketch**: Add a periodic call in `run.rs`'s main loop (e.g., on each IPC tick or memory poll interval). Confirm the GM detector's `update_spawns` data is fresh at call time.
- **Severity rationale**: Safety-critical feature (pause automation when GM detected) is silently non-functional. Players could be flagged while thinking they are protected.

---

### [MEDIUM] `tradeskill_trophy.rs` uses `.expect("lock poisoned")` on hot-path Mutex

- **File**: `textquest-dll/src/tradeskill_trophy.rs:37,50,74,94`
- **Problem**: `TROPHY_MANAGER.lock().expect("tradeskill trophy manager lock poisoned")` panics the DLL thread if any previous caller panicked while holding the lock. In the DLL context, a panic may be caught by the VEH as an access violation, creating undefined behavior rather than a clean shutdown.
- **Evidence**:

```rust
let mut manager = TROPHY_MANAGER
    .lock()
    .expect("tradeskill trophy manager lock poisoned");
```

- **Fix sketch**: Use `.unwrap_or_else(std::sync::PoisonError::into_inner)` as done correctly in `combat/mod.rs:COMBATANT` locks — this recovers the inner value rather than panicking.
- **Severity rationale**: Mutex poisoning is rare but a secondary panic in a DLL is unrecoverable. The fix is one line.

---

### [MEDIUM] Warrior config `panic!` on malformed TOML fires in production initialization

- **File**: `textquest-dll/src/combat/classes/warrior.rs:62`
- **Problem**: `parse_runtime_config` calls `panic!("Failed to parse Warrior runtime config from {source}: {error}")` for both user-provided config files and the embedded default. If a user provides a malformed `warrior.toml`, the entire DLL crashes at the `LazyLock` initialization point (first combat access).
- **Evidence**:

```rust
fn parse_runtime_config(contents: &str, source: &str) -> WarriorRuntimeConfig {
    toml::from_str(contents).unwrap_or_else(|error| {
        panic!("Failed to parse Warrior runtime config from {source}: {error}")
    })
}
```

- **Fix sketch**: Return `Result<WarriorRuntimeConfig, String>` and propagate the error through `LazyLock` initialization, falling back to `WarriorRuntimeConfig::default()` with a tracing warning if the user-provided file is malformed. Do not panic on the embedded default (that is a programming error and a compile-time test would catch it).
- **Severity rationale**: A misconfigured `warrior.toml` crashes all warrior-class clients at first combat tick.

---

### [MEDIUM] `is_trusted_origin` returns `true` in test builds for requests with no Origin header

- **File**: `textquest-web/src/api/loot.rs:445-447`
- **Problem**: `return cfg!(test)` allows all no-Origin requests in test builds. If a test binary is accidentally deployed (e.g., debug build on Frostreaver), all mutating API routes are open to any non-browser HTTP client without origin validation.
- **Evidence**:

```rust
let Some(origin) = headers.get(axum::http::header::ORIGIN) else {
    return cfg!(test);  // open to all in test build
};
```

- **Fix sketch**: This is acceptable in test binaries, but add a CI check or `debug_assert!(!cfg!(test))` at server startup that warns loudly if a test-cfg binary is serving on a non-loopback address.
- **Severity rationale**: Low risk in practice (test binary unlikely to be deployed), but the pattern is fragile — a future refactor could propagate `cfg!(test)` into the release path.

---

### [MEDIUM] `InMemoryCollector` uses `.unwrap()` on every mutex lock — poison propagates silently

- **File**: `textquest-common/src/observability.rs:134,151,162,195,199,203,207`
- **Problem**: All `InMemoryCollector` methods call `self.metrics.lock().unwrap()`. If any method panics while holding the lock, all subsequent calls in all threads will panic too, silently killing metrics collection rather than recovering.
- **Evidence**:

```rust
fn record(&self, metric: Metric) {
    self.metrics.lock().unwrap().push(metric);
}
```

- **Fix sketch**: Use `.unwrap_or_else(std::sync::PoisonError::into_inner)` throughout, consistent with the pattern in `combat/mod.rs`.
- **Severity rationale**: Metrics collection failure shouldn't crash the process; the recovery pattern is already established in the codebase.

---

### [MEDIUM] Packet monitor screen has 5 hardcoded stubs — feature is incomplete / misleading

- **File**: `textquest/src/tui/ui/packets.rs:60,99,194,322,328,352`
- **Problem**: The packet monitor UI shows a hardcoded `peak_rate = 12482` packets/sec, `is_selected = false` for every row (no row selection), no payload bytes in the detail pane, and PIDs instead of character names. The feature renders as functional but all interactive/contextual values are stubs.
- **Evidence**:

```rust
let peak_rate = 12482; // TODO: Calculate from timestamps
let is_selected = false; // TODO: Track selected index in state
// TODO: Get actual payload bytes
// TODO: Resolve client_id to character name
```

- **Fix sketch**: Either wire real data from `PacketMonitorState` or mark the panel as "WIP" in the UI title so operators don't trust the displayed stats.
- **Severity rationale**: Hardcoded 12,482 peak rate is misleading in a monitoring tool; operators may make decisions based on it.

---

### [LOW] `sync_gm_state_to_web` creates a new `reqwest::blocking::Client` per call

- **File**: `textquest/src/tui/app.rs:3501`
- **Problem**: Even if moved to a background thread (see HIGH finding above), creating a `Client` per invocation re-initializes TLS, DNS, and connection pools on every GM event. The blocking client should be created once and reused.
- **Evidence**: `let client = match reqwest::blocking::Client::builder()...build()` inside the function body.
- **Fix sketch**: Store a `reqwest::blocking::Client` as a field on `App` or in a `LazyLock` static.
- **Severity rationale**: Minor performance — TLS init is expensive but GM events are rare.

---

### [LOW] `find_gadgets` in `stack_spoof.rs` uses `.unwrap_or_default()` on CString creation

- **File**: `textquest-dll/src/stealth/stack_spoof.rs:37`
- **Problem**: `CString::new(module_name).unwrap_or_default()` silently converts a module name containing a null byte to an empty string. `GetModuleHandleA(PCSTR::from_raw("".as_ptr()))` returns the calling module, not the intended DLL, causing incorrect gadget collection without any diagnostic.
- **Evidence**:

```rust
let c_name = std::ffi::CString::new(module_name).unwrap_or_default();
```

- **Fix sketch**: Return `Vec::new()` with a warning log if `CString::new` fails: `CString::new(module_name).ok().map_or_else(Vec::new, |c| ...)`.
- **Severity rationale**: Module names in practice won't have null bytes, but silent wrong-module fallback could degrade stack spoofing.

---

### [LOW] `etw_blind.rs` hardware breakpoint only set for the calling thread — not process-wide

- **File**: `textquest-dll/src/stealth/etw_blind.rs:set_hw_breakpoint`
- **Problem**: `GetCurrentThread()` returns a pseudo-handle scoped to the calling thread. Hardware breakpoints set this way only apply to the thread that calls `set_hw_breakpoint`. ETW calls on other threads in the EQ process will bypass the blind entirely.
- **Evidence**: `let thread = GetCurrentThread();` inside `set_hw_breakpoint`.
- **Fix sketch**: This is likely intentional (only the DLL's hook thread calls `NtTraceEvent`), but the comment should make it explicit. If process-wide blinding is needed, enumerate threads via `CreateToolhelp32Snapshot` and set DR0 on each.
- **Severity rationale**: Low — architectural decision, not a bug per se, but worth documenting.

---

## Hot file map

| File                                          | Findings                                                           |
| --------------------------------------------- | ------------------------------------------------------------------ |
| `textquest-dll/src/stealth/page_encrypt.rs`   | 1 (CRITICAL)                                                       |
| `textquest/src/tui/app.rs`                    | 3 (HIGH×1, MEDIUM×2)                                               |
| `textquest-web/src/main.rs`                   | 2 (HIGH×1, MEDIUM×1)                                               |
| `textquest/src/tui/state.rs`                  | 1 (MEDIUM)                                                         |
| `textquest/src/tui/ui/packets.rs`             | 1 (MEDIUM)                                                         |
| `textquest-dll/src/tradeskill_trophy.rs`      | 1 (MEDIUM)                                                         |
| `textquest-dll/src/combat/classes/warrior.rs` | 1 (MEDIUM)                                                         |
| `textquest-common/src/observability.rs`       | 1 (MEDIUM)                                                         |
| `textquest-web/src/api/loot.rs`               | 1 (MEDIUM)                                                         |
| `textquest-dll/src/stealth/stack_spoof.rs`    | 1 (LOW)                                                            |
| `textquest-dll/src/stealth/etw_blind.rs`      | 1 (LOW)                                                            |
| `textquest/src/tui/app.rs` (blocking client)  | 1 (LOW)                                                            |
| `textquest-dll/src/combat/xtarget.rs`         | 0 (reviewed, safe)                                                 |
| `textquest-dll/src/combat/buffs.rs`           | 0 (reviewed, safe)                                                 |
| `textquest-dll/src/hooks/game_loop.rs`        | 0 (reviewed, safe — MAX_WALK guard present)                        |
| `textquest-web/src/accounts.rs`               | 0 (reviewed, safe — AES-256-GCM + Argon2id, no plaintext exposure) |
| `textquest/src/credentials/store.rs`          | 0 (reviewed, safe — Zeroizing used)                                |
| `textquest-dll/src/combat/mod.rs`             | 0 (reviewed, safe — poison recovery pattern correct)               |
| `textquest-dll/src/combat/strategy.rs`        | 0 (unwraps in tests only)                                          |
| `textquest/src/metrics/xp_tracker.rs`         | 0 (reviewed, safe — MAX_SAMPLES enforced)                          |
| `textquest/src/metrics/admin_monitoring.rs`   | 0 (reviewed, safe — trim_to_capacity called)                       |
| `textquest-common/src/scan_engine.rs`         | 0 (reviewed, bounds check before slice)                            |
| `textquest-dll/src/combat/humanize.rs`        | 0 (wrapping arithmetic intentional)                                |
| `web/src/hooks/useWebSocket.ts`               | 0 (TS layer, out of Rust scope)                                    |
| `web/src/lib/api.ts`                          | 0 (TS layer, out of Rust scope)                                    |
| `textquest-dll/src/stealth/etw_blind.rs`      | 1 (LOW — single-thread scope)                                      |
| `textquest-dll/src/hooks/targeting.rs`        | 0 (panics are in tests only)                                       |
| `textquest/src/discord/webhook.rs`            | 0 (blocking client on dedicated thread — correct)                  |
| `textquest/src/box_chat.rs`                   | 0 (blocking I/O on dedicated thread — correct)                     |
| `textquest-dll/src/combat/twist.rs`           | 0 (panics in tests only)                                           |
