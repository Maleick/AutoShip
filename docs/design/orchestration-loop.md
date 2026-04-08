# Orchestration Loop Design

> Design document for the main orchestration loop — the missing integration layer
> that wires `ClientManager`, `LaunchCoordinator`, `LoginStateMachine`,
> `PostLoginSequencer`, `HealthMonitor`, and `Orchestrator` into a single
> autonomous lifecycle.

## 1. Problem Statement

All building blocks for autonomous multi-client operation exist as independent
subsystems, but nothing drives them together:

| Subsystem            | Location                  | What it does                               | What drives it today                |
| -------------------- | ------------------------- | ------------------------------------------ | ----------------------------------- |
| `ClientManager`      | `client/manager.rs`       | Discover PIDs, inject DLLs, track sessions | Manual `discover()` calls           |
| `LaunchCoordinator`  | `launcher/coordinator.rs` | Staggered spawn + login FSM orchestration  | Not wired to TUI loop               |
| `LoginStateMachine`  | `launcher/login_sm.rs`    | Per-client login phase tracking            | Ticked by `LaunchCoordinator`       |
| `PostLoginSequencer` | `launcher/post_login.rs`  | Group join, buff, nav-to-camp              | Has API but no caller               |
| `HealthMonitor`      | `client/healing.rs`       | Ping/pong, crash detection, restart gating | `check_health()` not called in loop |
| `Orchestrator`       | `orchestrator.rs`         | Camp/hunt loop, IPC dispatch, combat coord | Ticked every 1s from `run_loop`     |

**The gap:** `run_loop` in `tui/run.rs` ticks the `Orchestrator` for camp/hunt
and scans for processes, but there is no lifecycle manager that:

- Launches clients that should be running but aren't
- Drives login through to InWorld
- Runs post-login setup (group, buff, nav)
- Monitors health and triggers re-launch on crash
- Performs graceful camp-out before intentional shutdown

## 2. Session Lifecycle State Machine

Each client slot progresses through a linear lifecycle. The `SlotLifecycle` enum
already exists in `client/session.rs` with the right states:

```
Configured ──► Launching ──► WaitingForLogin ──► EnteringWorld ──► Live
                   ▲                                                │
                   │                                                ▼
              Relaunching ◄── Recovering ◄─────────────────── [crash/DC]
                                  │
                                  ▼
                              Blocked (max retries exceeded)
```

### State Transitions

| From              | To                | Trigger                                         | Action                                          |
| ----------------- | ----------------- | ----------------------------------------------- | ----------------------------------------------- |
| `Configured`      | `Launching`       | Orchestration loop picks slot from launch queue | `LaunchCoordinator::enqueue()`                  |
| `Launching`       | `WaitingForLogin` | `CoordinatorEvent::ClientLaunched { pid }`      | Store PID in `EqSession`, start login tracking  |
| `WaitingForLogin` | `EnteringWorld`   | `LoginStateMachine` reaches `InWorld`           | `PostLoginSequencer::new()` starts              |
| `EnteringWorld`   | `Live`            | `PostLoginSequencer::is_ready()` returns true   | Register with `Orchestrator`, add to camp loop  |
| `Live`            | `Recovering`      | `HealthMonitor` detects crash/unresponsive      | `Orchestrator::remove_client()`, begin recovery |
| `Recovering`      | `Launching`       | Restart count < max (10)                        | Kill stale process, re-enqueue for launch       |
| `Recovering`      | `Blocked`         | Restart count >= max                            | Alert operator via TUI toast + Discord          |
| `Live`            | `Configured`      | Graceful shutdown requested                     | Camp-out sequence, eject DLL, terminate process |

### Integration with Existing `SlotLifecycle`

The `SlotLifecycle` enum in `session.rs` already has `Configured`, `Launching`,
`WaitingForLogin`, `EnteringWorld`, `Live`, `Recovering`, `Blocked`. This is
the exact set needed. The orchestration loop simply drives transitions by
calling existing subsystem methods and updating `session.slot_lifecycle`.

## 3. Event Loop Architecture

### Current Loop Structure (tui/run.rs:run_loop)

```
while app.running {
    terminal.draw()             // ~16ms
    handle_events(50ms poll)    // keyboard input
    scan_for_clients()          // every 10s
    refresh_eq_data()           // every refresh_rate_ms
    orchestrator.tick()         // every 1s (camp/hunt)
    poll_log_watchers()         // every 2s
    poll_packets()              // every 500ms
    poll_discord()              // every iteration
    tick_soul_engine()          // every 5s
}
```

### Proposed Addition: Orchestration Tick

Add a new periodic tick at **2-second intervals** that drives the full lifecycle.
This is a new method on a new `LifecycleManager` struct, called from the
existing `run_loop`:

```
// In run_loop, after the camp tick:
if last_lifecycle_tick.elapsed() >= LIFECYCLE_TICK_INTERVAL {  // 2s
    lifecycle_manager.tick(&mut app, &mut orchestrator);
    last_lifecycle_tick = Instant::now();
}
```

**Why 2 seconds?** Login phases take 5-60 seconds each. Health checks use a
15-second timeout. A 2-second tick is responsive enough to catch transitions
without burning CPU on 36 clients. This also aligns with the health monitor's
5-second ping interval — we check twice per ping cycle.

### Sync vs Async

**Decision: Keep synchronous.** The existing loop is synchronous and
single-threaded. All IPC is blocking (named pipe connect → write → read → drop).
The DLL disconnects after each command, so there's no long-lived connection to
manage async. The tick-based model is natural for game automation (matches EQ's
own 6-second server tick granularity).

If IPC latency becomes a bottleneck at 36 clients (unlikely — each pipe
round-trip is <1ms on localhost), the `dispatch_action` calls can be moved to a
`tokio::spawn_blocking` pool later without changing the lifecycle model.

## 4. LifecycleManager Design

```rust
pub struct LifecycleManager {
    /// Client manager owns all sessions.
    client_manager: ClientManager,
    /// Launch coordinator handles staggered spawning.
    launch_coordinator: LaunchCoordinator,
    /// Post-login sequencers, keyed by ClientId.
    post_login: HashMap<ClientId, PostLoginSequencer>,
    /// Group assignments from config.
    group_config: Vec<GroupConfig>,
    /// Whether the full fleet should be running.
    fleet_active: bool,
    /// Graceful shutdown in progress.
    shutting_down: bool,
}
```

### tick() Method — What Happens Every 2 Seconds

```
fn tick(&mut self, app: &mut App, orchestrator: &mut Orchestrator) {
    // Phase 1: Health check all Live clients
    let needs_restart = self.client_manager.check_health();
    for client_id in needs_restart {
        self.begin_recovery(client_id, orchestrator);
    }

    // Phase 2: Tick the launch coordinator (advances login FSMs, spawns queued)
    let events = self.launch_coordinator.tick();
    for event in events {
        self.handle_coordinator_event(event, orchestrator);
    }

    // Phase 3: Tick post-login sequencers
    self.tick_post_login(orchestrator);

    // Phase 4: Check for Configured slots that should be Launching
    if self.fleet_active && !self.shutting_down {
        self.enqueue_configured_slots();
    }

    // Phase 5: Sync TUI state
    self.sync_tui_state(app);
}
```

### Phase Details

**Phase 1 — Health Check:**

- Calls `ClientManager::check_health()` which iterates all sessions
- For each client that `should_restart()`, transitions to `Recovering`
- Calls `orchestrator.remove_client(pid)` to clean up camp membership
- If restart count < max: kills process, transitions to `Launching`, re-enqueues
- If restart count >= max: transitions to `Blocked`, sends TUI toast

**Phase 2 — Launch Coordinator Events:**

- `ClientLaunched { client_id, pid }`: Update `EqSession.pid`, transition to `WaitingForLogin`
- `ClientReady { client_id }`: Transition to `EnteringWorld`, create `PostLoginSequencer`
- `ClientFailed { client_id, error }`: Increment failure count, re-enqueue or block
- `AllPaused { reason }`: Set `fleet_active = false`, alert operator

**Phase 3 — Post-Login Tick:**

- For each active `PostLoginSequencer`, check if game state shows phase completion
- `GroupJoined` → advance to buffing
- `BuffsApplied` → advance to navigation
- `CampReached` → mark Ready, transition `SlotLifecycle` to `Live`
- On `Live` transition: call `orchestrator.register_client(pid)`, add to camp members

**Phase 4 — Enqueue Configured Slots:**

- Scan all sessions in `Configured` state
- Look up their account info from `group_config`
- Call `launch_coordinator.enqueue(client_id, account_info)`
- Transition to `Launching`

**Phase 5 — TUI Sync:**

- Update `app.clients` with current lifecycle states
- Update status bar with fleet summary: "24/36 Live | 6 Logging In | 3 Launching | 3 Blocked"

## 5. Integration Points — Existing Methods

The lifecycle manager calls into existing APIs. No new methods needed on
subsystems except the manager itself:

| Action               | Existing Method                       | Called When                                     |
| -------------------- | ------------------------------------- | ----------------------------------------------- |
| Spawn EQ process     | `spawner::spawn_eq_client()`          | Via `LaunchCoordinator::tick()`                 |
| Track login progress | `LoginStateMachine::advance()`        | Via `LaunchCoordinator::tick()`                 |
| Inject DLL           | `ClientManager::inject(id, dll_path)` | After `ClientLaunched` event                    |
| Group invite         | `post_login::group_invite_commands()` | `PostLoginSequencer::next_command()`            |
| Accept invite        | `post_login::group_accept_command()`  | `PostLoginSequencer::next_command()`            |
| Send IPC command     | `Orchestrator::send_ipc_command()`    | Post-login commands dispatched via orchestrator |
| Register for camp    | `Orchestrator::register_client(pid)`  | When `PostLoginSequencer::is_ready()`           |
| Add to camp loop     | `Orchestrator::start_camp()`          | When all group members are Live                 |
| Health ping          | `HealthMonitor::check()`              | Via `ClientManager::check_health()`             |
| Crash recovery       | `HealthMonitor::record_restart()`     | `begin_recovery()`                              |
| Eject DLL            | `Orchestrator::eject_client(pid)`     | Graceful shutdown or pre-recovery               |
| Remove from camp     | `Orchestrator::remove_client(pid)`    | On crash detection                              |

### New Methods Needed

| Method                                       | On                 | Purpose                                               |
| -------------------------------------------- | ------------------ | ----------------------------------------------------- |
| `LifecycleManager::new(config, groups)`      | `LifecycleManager` | Constructor from config                               |
| `LifecycleManager::tick(app, orch)`          | `LifecycleManager` | Main lifecycle driver                                 |
| `LifecycleManager::activate_fleet()`         | `LifecycleManager` | Begin launching all configured slots                  |
| `LifecycleManager::shutdown_fleet()`         | `LifecycleManager` | Begin graceful camp-out + shutdown                    |
| `LifecycleManager::begin_recovery(id, orch)` | `LifecycleManager` | Handle a crashed client                               |
| `send_ipc_command` (pub)                     | `Orchestrator`     | Currently private — needs pub for post-login dispatch |

## 6. Error Handling & Recovery

### Crash During Login

```
WaitingForLogin + process dies  →  HealthMonitor detects Crashed
    → Remove from launch_coordinator.active_logins
    → Increment restart count
    → If restarts < max: re-enqueue with backoff (5s × attempt)
    → If restarts >= max: Blocked + alert
```

### Crash During Camp (Live)

```
Live + process dies  →  HealthMonitor detects Crashed
    → orchestrator.remove_client(pid) — removes from camp members
    → Camp loop auto-adjusts (CampLoop handles member removal gracefully)
    → begin_recovery: kill stale process, re-enqueue
    → On return to Live: orchestrator.register_client() re-adds to camp
```

### Crash During Post-Login

```
EnteringWorld + process dies  →  HealthMonitor detects Crashed
    → Remove PostLoginSequencer for this client
    → Same recovery path as login crash
```

### Mass Failure (>3 failures in 60s)

```
LaunchCoordinator detects mass failure  →  AllPaused event
    → Set fleet_active = false
    → TUI toast: "Fleet paused: mass failure detected"
    → Discord alert (if bridge active)
    → Operator must manually :fleet resume
```

### Unresponsive (Frozen Client)

```
Live + no pong for 15s  →  HealthMonitor detects Unresponsive
    → Same as Crashed path but with 30s grace period first
    → Send a Ping IPC command as last-chance check
    → If still unresponsive after grace: treat as crashed
```

## 7. Graceful Shutdown Sequence

When the operator issues `:fleet stop` or the app is exiting:

```
1. Set shutting_down = true
2. For each Live client:
   a. If in combat: wait for combat to end (max 30s timeout)
   b. Send /camp command (begins EQ's 30-second camp-out timer)
   c. Transition to CampingOut (new state, or reuse Recovering)
3. After /camp completes (detect via game state or 35s timeout):
   a. Eject DLL via orchestrator.eject_client(pid)
   b. Wait 2s for DLL cleanup
   c. Terminate process
   d. Transition to Configured
4. For non-Live clients (Launching, WaitingForLogin, EnteringWorld):
   a. Cancel login (abort LaunchCoordinator queue)
   b. Terminate process immediately
   c. Transition to Configured
5. Set fleet_active = false
6. TUI status: "Fleet stopped — all clients camped out"
```

### Why /camp Instead of Kill

EQ's `/camp` command performs a safe logout (30s timer, saves character state).
Killing the process without camping can cause:

- Character stuck in-world for 5+ minutes (LD state)
- Missed autosave (lost exp/items)
- Server-side lockout preventing re-login

The orchestrator already has `send_slash_command()` for this.

## 8. TUI Integration

### How the TUI Event Loop Interacts

The `LifecycleManager` is owned alongside `App` and `Orchestrator` at the
`run_loop` level. It does NOT run its own thread — it ticks synchronously in the
existing loop:

```rust
fn run_loop(
    terminal: &mut Terminal<...>,
    app: &mut App,
    orchestrator: &mut Orchestrator,
    lifecycle: &mut LifecycleManager,  // NEW parameter
) -> Result<()> {
    // ...existing timers...
    let mut last_lifecycle_tick = Instant::now();

    while app.running {
        terminal.draw(|frame| draw(frame, app))?;
        handle_events(app, poll_timeout, orchestrator)?;

        // ...existing periodic ticks...

        // NEW: Lifecycle tick (every 2s)
        if last_lifecycle_tick.elapsed() >= Duration::from_secs(2) {
            lifecycle.tick(app, orchestrator);
            last_lifecycle_tick = Instant::now();
        }

        // ...existing camp tick, log poll, packet poll, etc...
    }
}
```

### TUI Commands

New commands for the `:` command bar:

| Command         | Action                                                              |
| --------------- | ------------------------------------------------------------------- |
| `:fleet start`  | `lifecycle.activate_fleet()` — begin launching all configured slots |
| `:fleet stop`   | `lifecycle.shutdown_fleet()` — graceful camp-out and shutdown       |
| `:fleet status` | Show per-slot lifecycle state summary                               |
| `:fleet pause`  | Pause all launches (keep Live clients running)                      |
| `:fleet resume` | Resume launches after pause or mass failure                         |

### Groups Panel Updates

The existing Groups TUI panel (`tui/ui/groups.rs`) should display `SlotLifecycle`
state per member. The data is already in `EqSession.slot_lifecycle` — the panel
just needs to read it. Color coding:

| State                                             | Color  | Icon |
| ------------------------------------------------- | ------ | ---- |
| `Live`                                            | Green  | `✓`  |
| `Launching` / `WaitingForLogin` / `EnteringWorld` | Yellow | `◌`  |
| `Recovering`                                      | Orange | `↻`  |
| `Blocked`                                         | Red    | `✗`  |
| `Configured`                                      | Gray   | `·`  |

## 9. Tick Budget

At 36 clients with a 2-second lifecycle tick:

| Operation                    | Per client | Total (36) | Notes                               |
| ---------------------------- | ---------- | ---------- | ----------------------------------- |
| `HealthMonitor::check()`     | ~1μs       | ~36μs      | Process alive check via OpenProcess |
| `LaunchCoordinator::tick()`  | —          | ~50μs      | Iterates active login FSMs          |
| Post-login sequencer tick    | ~10μs      | ~360μs     | Only for clients in EnteringWorld   |
| TUI state sync               | ~5μs       | ~180μs     | HashMap lookups                     |
| **Total per lifecycle tick** |            | **~626μs** | Well under the 2s budget            |

The existing `Orchestrator::tick()` (camp loop) runs every 1s and does shared
memory reads (~100μs each × 36 = ~3.6ms). Adding a 626μs lifecycle tick every
2s is negligible.

## 10. Configuration

### New Config Section (`config/frostreaver.toml`)

```toml
[fleet]
auto_start = false          # Start launching on app boot?
max_concurrent_launches = 3 # Parallel login slots
health_check_interval = 5   # Seconds between health pings
max_restarts_per_client = 10
mass_failure_threshold = 3  # Failures in 60s triggers pause
grace_period_secs = 30      # Wait before treating unresponsive as crashed
camp_out_timeout_secs = 35  # Max wait for /camp to complete
```

This maps directly to the existing `LaunchConfig` + `RetryConfig` structs with
minor additions for the lifecycle-specific settings.

## 11. Implementation Sequence

Suggested order for implementing (each is a single PR):

1. **`LifecycleManager` struct + constructor** — Wire into `run_loop` with empty `tick()`
2. **Fleet start/stop TUI commands** — `:fleet start` enqueues all configured slots
3. **Launch integration** — `tick()` Phase 2 + Phase 4: enqueue → spawn → track login
4. **Post-login integration** — `tick()` Phase 3: drive post-login through to Live
5. **Health recovery** — `tick()` Phase 1: crash detection → recovery → re-launch
6. **Graceful shutdown** — `/camp` sequence, DLL eject, process termination
7. **TUI state sync** — Groups panel lifecycle colors, fleet status bar
8. **Discord integration** — Fleet alerts (mass failure, blocked clients, fleet ready)

Each PR is independently testable. The lifecycle manager is inert until
`:fleet start` is issued, so partially-merged PRs don't break existing behavior.

## 12. Open Questions

1. **DLL injection timing**: Should injection happen immediately after process
   spawn (in `ClientLaunched` handler) or after login screen is detected?
   Current code in `client/manager.rs` injects eagerly. The login DLL module
   needs the injection to detect login phases, so **inject immediately** is
   likely correct.

2. **Group formation order**: When multiple group members reach `EnteringWorld`
   simultaneously, who invites whom? The current `PostLoginSequencer` assumes a
   `group_id` but doesn't specify the leader. **Proposal**: First member to
   reach `EnteringWorld` becomes the inviter; subsequent members accept.

3. **Camp loop re-entry**: When a crashed client returns to `Live`, should it
   re-join the existing camp loop mid-cycle or wait for a clean Idle state?
   **Proposal**: Add immediately — `CampLoop` already handles dynamic member
   lists and the camp state machine will assign the returning member a role.

4. **Zone mismatch on recovery**: A relaunched client starts in their bind point,
   which may not be the camp zone. The `PostLoginSequencer` handles nav-to-camp,
   but cross-zone navigation (M7) isn't complete. **Interim**: If the client
   zones into the wrong zone, mark as `Blocked` with a "wrong zone" message
   until M7 zone routing is implemented.
