# TextQuest Performance SLAs and Scaling Targets

This document defines formal performance Service Level Agreements (SLAs) for TextQuest's 36-client multibox orchestration. All targets are measured on standard hardware (Frostreaver mini PC: 12-core, 16GB RAM, SSD) with a live TLP server connection.

## 1. Executive Summary

TextQuest targets **36 simultaneous EverQuest clients** on a single Windows machine with deterministic performance. SLAs are organized by resource domain: memory, IPC latency, UI responsiveness, state synchronization, and operational timelines.

| Metric                       | Target   | P99 | Notes                                           |
| ---------------------------- | -------- | --- | ----------------------------------------------- |
| Per-client memory (resident) | < 50 MB  | —   | Includes client executable + DLL + local state  |
| IPC command latency          | < 10 ms  | —   | Command → DLL execution (named pipe round-trip) |
| TUI frame rate               | > 30 FPS | —   | With 36 live clients, full state refresh        |
| State sync delay             | < 5 ms   | —   | Shared memory write → TUI read                  |
| Zone transition end-to-end   | < 30 s   | —   | Portal zone-in → navigator reports safe zone    |
| Fleet startup (36 clients)   | < 5 min  | —   | All 36 clients launched, DLL injected, ready    |
| Per-client DLL injection     | < 2 s    | —   | Process spawn → DLL loaded → first IPC ready    |

---

## 2. Memory SLAs

### 2.1 Per-Client Resident Memory

**Target:** < 50 MB per EverQuest client process

**Definition:** Resident Set Size (RSS) measured via `Get-Process | Select WorkingSet` on Windows, excluding virtual memory and shared system DLLs.

**What this includes:**

- EverQuest client executable heap and stack
- Textquest DLL injected code + state
- Local game state cache (player buffs, spawn list, etc.)
- IPC named pipe and shared memory handles

**What this excludes:**

- EverQuest graphical assets (textures, models) — managed by EQ's streaming cache
- Shared system libraries (`msvcrt.dll`, kernel DLLs)
- Virtual memory (committed but not paged in)

**Measurement:**

```powershell
Get-Process eq | Select-Object Name, @{Name="MemMB"; Expression={[math]::Round($_.WorkingSet / 1MB, 2)}}
```

**Typical breakdown per client (target < 50 MB):**

- EQ client base: ~30 MB
- DLL injected code + hooks: ~2 MB
- Shared state cache: ~8 MB
- IPC buffers: ~10 MB

**Scaling:** Total resident memory for 36 clients should stay under 1.8 GB (50 × 36), leaving > 14 GB available for graphical rendering.

---

### 2.2 Shared State Memory

**Target:** < 100 MB total for all 36 shared memory buffers

**Definition:** Summed size of all named shared-memory segments (one per client) used for broadcasting game state from DLL to orchestrator.

**Typical shared buffer layout per client:**

```
PlayerInfo struct       : ~1.5 KB
TargetInfo struct       : ~0.5 KB
SpawnList (up to 200)   : ~50 KB
Equipment slots (32)    : ~2 KB
Buffs (48 slots)        : ~2 KB
Extended target data    : ~5 KB
─────────────────
Total per client        : ~61 KB
```

**36 clients × 61 KB ≈ 2.2 MB shared memory** (well under 100 MB target).

---

### 2.3 Orchestrator Memory

**Target:** < 200 MB for the external orchestrator process (textquest.exe)

**What this includes:**

- Command queue and response buffers for all 36 IPC pipes
- In-memory game state snapshots
- TUI framework (ratatui) + terminal state
- Camp loop state machine per group
- Logger and metrics buffers

**Expected breakdown:**

- Game state snapshots (36 clients): ~8 MB
- IPC command queues: ~20 MB
- TUI + terminal: ~50 MB
- Camp/rotation logic: ~30 MB
- DLL lifecycle + health monitor: ~20 MB
- Logs (in-memory ring buffer): ~50 MB

---

## 3. IPC Latency SLAs

### 3.1 Command Execution Latency (Named Pipes)

**Target:** < 10 ms p99 for command → DLL execution

**Definition:** Time from `WriteFile()` on orchestrator → `ReadFile()` returns on DLL.

**Breakdown (typical localhost performance on Windows):**

```
Orchestrator: WriteFile(pipe)              : ~0.1 ms
Network/IPC transit                        : ~0.2 ms
DLL: ReadFile(pipe) + context switch       : ~0.5 ms
DLL: Execute command (simple, e.g., /sit)  : ~1 ms
DLL: WriteFile(response)                   : ~0.1 ms
Orchestrator: ReadFile(response)           : ~0.2 ms
─────────────────────────────
Total round-trip                           : ~2.1 ms (typical)
```

**Measurement:** Inject a `Ping` IPC command and measure wall-clock time between `send()` and response received.

**Scaling:** With 36 concurrent IPC connections:

- Baseline per-command: ~2 ms
- Queued commands may experience buffering delays up to 10 ms if orchestrator is blocked on expensive operations (state refresh, UI draw)
- Target ensures p99 stays under 10 ms

**Critical assumption:** IPC pipes use non-blocking I/O and do not serialize commands. Commands from different clients process in parallel on the DLL side.

---

### 3.2 State Broadcast Latency (Shared Memory)

**Target:** < 5 ms from DLL write → orchestrator read

**Definition:** Time between DLL updating shared memory with new game state → orchestrator's next shared-memory read observes the update.

**Breakdown:**

```
DLL: Capture game state into buffer        : ~1 ms
DLL: Write to shared memory                : ~0.1 ms
Orchestrator: Read interval (next tick)    : ~0 ms - 1 s (variable)
─────────────────────────────
Sync delay (on worst-case read)            : 1 s (one full orchestrator tick)
Target achieved when read happens soon after write: < 5 ms
```

**Measurement strategy:**

1. DLL writes a timestamp to shared memory
2. Orchestrator reads timestamp and computes (now - timestamp)
3. Log all samples and compute p99

**Scaling:** Shared memory reads are per-client and happen in parallel. Total read time for 36 clients is `36 × 1μs ≈ 36μs` (negligible).

---

### 3.3 Response Jitter Under Load

**Target:** < 50 ms p99 latency variance with 36 active clients

**Definition:** Maximum difference between fastest and slowest command responses during sustained load.

**Measurement:**

- Send 1000 commands in sequence across 36 clients
- Measure each command's latency
- Compute p1, p50, p99, p99.9
- Jitter = (p99 - p50)

**Expected profile:**

- p50: 2-3 ms
- p99: 8-10 ms
- Jitter: ~7 ms

**Causes of jitter:**

- GC pauses in orchestrator
- TUI frame draw blocking IPC reads
- Windows scheduler context-switching between threads

---

## 4. TUI Responsiveness SLAs

### 4.1 Frame Rate

**Target:** > 30 FPS with 36 live clients

**Definition:** Frames rendered per second to terminal, including full game state refresh.

**Breakdown (per frame, target ~33 ms):**

```
Poll keyboard input                        : ~5 ms
Read game state from 36 shared memory bufs : ~2 ms (36 × 50μs)
TUI layout calculation                     : ~10 ms
Terminal render to stdout                 : ~10 ms
─────────────────────────────
Total per frame                            : ~27 ms (target < 33 ms)
```

**Measurement:** Use `Instant::now()` before/after each frame loop, compute FPS over 60-frame window.

**Frame skipping strategy:** If a frame takes > 33 ms, skip the next input poll but continue rendering at best-effort rate.

---

### 4.2 Input Responsiveness

**Target:** < 100 ms latency from keypress → command executed

**Definition:** Time from operator pressing a key (e.g., `:` to enter command mode) → command bar shows the character.

**Breakdown:**

```
Terminal event poll                        : ~50 ms (fixed poll interval)
Event processing + routing                 : ~5 ms
Input buffer update                        : ~2 ms
Frame rendering                            : ~33 ms (worst case)
─────────────────────────────
Total                                      : ~90 ms (typical)
```

**Scaling:** Not affected by client count — purely TUI framework performance.

---

### 4.3 Panel Update Latency

**Target:** < 500 ms from game state change → panel reflects update

**Definition:** Example: A player's HP drops on server → Orchestrator reads shared memory → Player HP panel updates visually.

**Breakdown:**

```
DLL broadcasts state                       : 0 ms (trigger)
Orchestrator reads on next tick (~100 ms) : +100 ms
App state updated                          : +5 ms
Panel rendered in next frame (~33 ms)      : +33 ms
─────────────────────────────
Total visibility latency                   : ~138 ms (typical, < 500 ms target)
```

---

## 5. State Synchronization SLAs

### 5.1 Shared Memory Write Frequency

**Target:** Updates at least every 500 ms per client

**Definition:** DLL writes fresh game state to shared memory at minimum frequency.

**Rationale:** If no updates happen for > 500 ms, operator cannot distinguish between "client is fine but idle" and "client is stuck."

**Implementation:** DLL-side timer triggers state broadcast every 500 ms if no game events occurred; events (like spell cast) trigger immediate broadcast.

---

### 5.2 State Consistency Across Clients

**Target:** All 36 clients' state consistent within ±1 second clock skew

**Definition:** If player A sees spawn B move to location X at time T, player C's local view of spawn B should show location X by time T±1s.

**Rationale:** Combat positioning and pull execution depend on clients agreeing on spawn locations within 1-second tolerance.

**Implementation:** Each shared state buffer includes a timestamp. Orchestrator detects clock skew and can re-sync on request.

---

### 5.3 Stale Data Detection

**Target:** Orchestrator detects stale state within 3 seconds

**Definition:** If DLL stops writing to shared memory (client crash, DLL hang), orchestrator detects and alerts operator.

**Mechanism:**

```rust
// Orchestrator checks timestamp in shared memory
let now = Instant::now();
let state_timestamp = read_from_shared_memory().timestamp;
if now - state_timestamp > Duration::from_secs(3) {
    alert("Client X: stale state for 3s");
}
```

---

## 6. Zone Transition SLAs

### 6.1 End-to-End Zone Transition

**Target:** < 30 seconds from portal activation → navigator reports safe in new zone

**Definition:** Time to complete a zone transition, including load time, EQ's zone-in sequence, and navigation validation.

**Breakdown (typical zone transition in Gfaydark → Qeynos):**

```
Portal click / zone line trigger            : 0 ms
EverQuest zone-out                          : ~3 s
Network handoff + zone init                 : ~3 s
Zone geometry loaded                        : ~2 s
Client ready (zone data packet received)    : ~8 s
Navigator: pathfind + movement validation   : ~5 s
─────────────────────────────
Total                                       : ~21 s (typical, < 30 s target)
```

**Measurement:**

1. Inject zone-transition command (e.g., `/zone newzone`)
2. Start timer
3. Poll shared memory for `in_zone_id == target`
4. Confirm DLL reports safe movement (no stuck flags)
5. Stop timer

**Scaling:** Zone transitions are per-client, run in parallel. Total time for 36 clients = time for slowest client (~30 s).

---

### 6.2 Cross-Zone Navigation

**Target:** < 3 minutes for navigation between distant zones (e.g., Commonlands → Kunark)

**Definition:** Time to navigate through multiple zones using waypoints and zone lines.

**Factors:**

- Number of zone transitions (e.g., 5 zones = 5 × 30 s = 150 s)
- Pathfinding complexity per zone
- Server lag on movement validation

**Scaling:** Parallel navigation across 36 clients takes time of slowest client.

---

## 7. Startup and Shutdown SLAs

### 7.1 Fleet Startup (36 Clients)

**Target:** All 36 clients launched, DLL injected, ready for commands within 5 minutes

**Definition:** Time from operator issuing `:fleet start` → all 36 clients report `Live` state in TUI.

**Breakdown:**

```
EQ client process spawn (1 per second stagger) : ~36 s (36 × 1 s)
Client window initialization + login screen   : ~60 s (overlapped)
DLL injection + initialization                : ~36 s (2 s per client, parallel)
Login automation (character select + login)   : ~120 s (overlapped per group)
Post-login setup (group join, buff, nav)      : ~60 s (overlapped per group)
─────────────────────────────
Total (with overlap)                          : ~240 s (~4 minutes)
```

**Stagger strategy:**

- Launch clients in batches of 3 per second (once per second per client)
- DLL injection overlaps with login
- Multiple groups advance login phases in parallel
- Total time ≈ 4 min (well under 5 min target)

**Measurement:** Timestamp from command to last client reaching `Live` state.

---

### 7.2 Per-Client DLL Injection

**Target:** < 2 seconds per client

**Definition:** Time from process spawn → DLL loaded, initialized, and first IPC command succeeds.

**Breakdown:**

```
Process spawn (CreateProcess)                : ~0.2 s
Process ready (entry point reached)          : ~0.5 s
Reflective DLL loader inject                 : ~0.3 s
DLL initialization (hooks installed)         : ~0.5 s
First IPC ping succeeds                      : ~0.3 s
─────────────────────────────
Total                                        : ~1.8 s (< 2 s target)
```

**Measurement:** Timestamp from spawn → first successful IPC `Ping` response.

---

### 7.3 Graceful Shutdown

**Target:** All 36 clients camped out and terminated within 2 minutes

**Definition:** Time from operator issuing `:fleet stop` → all processes exited, no characters left in-world.

**Sequence:**

```
TUI command `:fleet stop`                    : 0 ms
Send /camp to each client                    : ~1 s (parallel)
Wait for camp-out timer (~30s per client)    : ~30 s (sequential)
Eject DLL from each client                   : ~36 s (1 s per client)
Kill remaining processes                     : ~5 s
─────────────────────────────
Total                                        : ~70 s (~1.2 minutes, < 2 min target)
```

**Verification:** Check that no characters remain in-world on the server after shutdown completes.

---

## 8. Scaling Tiers

SLAs are defined for four discrete scaling targets. Performance should remain linear with client count up to 36.

### 8.1 Tier 1: 6 Clients

**Use case:** Small group testing, low-risk development

| Metric                 | Target   |
| ---------------------- | -------- |
| Total resident memory  | < 300 MB |
| Per-client IPC latency | < 10 ms  |
| TUI frame rate         | > 40 FPS |
| Fleet startup time     | < 60 s   |

**Hardware:** Any modern Windows 10/11 machine with 4GB+ RAM.

---

### 8.2 Tier 2: 12 Clients

**Use case:** Small raid group, operational testing

| Metric                 | Target   |
| ---------------------- | -------- |
| Total resident memory  | < 600 MB |
| Per-client IPC latency | < 10 ms  |
| TUI frame rate         | > 35 FPS |
| Fleet startup time     | < 2 min  |

**Hardware:** Mid-range workstation (8GB+ RAM, quad-core CPU).

---

### 8.3 Tier 3: 18 Clients

**Use case:** Medium raid composition (multi-group)

| Metric                 | Target   |
| ---------------------- | -------- |
| Total resident memory  | < 900 MB |
| Per-client IPC latency | < 10 ms  |
| TUI frame rate         | > 32 FPS |
| Fleet startup time     | < 3 min  |

**Hardware:** Frostreaver-class (12-core, 16GB RAM, SSD).

---

### 8.4 Tier 4: 36 Clients (Maximum)

**Use case:** Full TLP composition

| Metric                 | Target   |
| ---------------------- | -------- |
| Total resident memory  | < 1.8 GB |
| Per-client IPC latency | < 10 ms  |
| TUI frame rate         | > 30 FPS |
| Fleet startup time     | < 5 min  |

**Hardware:** Frostreaver-class (12-core, 16GB RAM, SSD) — minimum viable hardware.

**Scaling law:** Linear up to 36 clients. Beyond 36, memory and CPU usage scale sub-linearly due to shared thread pools and bulk state reads.

---

## 9. Known Bottlenecks and Mitigation

### 9.1 TUI Frame Rate Degradation

**Bottleneck:** Rendering 36 spawn rows × 5 screen panels = 180 UI elements per frame.

**Severity:** Medium — FPS may drop to 25 FPS during intense screen redraws.

**Mitigation:**

- Implement dirty-flag tracking: only re-render panels whose underlying data changed
- Use incremental terminal updates (only redraw changed lines) instead of full-screen redraws
- Cache panel layouts between frames

**Status:** Deferred to post-M8 optimization pass.

---

### 9.2 IPC Pipe Contention

**Bottleneck:** With 36 concurrent IPC pipes, orchestrator may block on `WriteFile()` if a single pipe's buffer fills.

**Severity:** Low — each pipe has a 64 KB buffer; command messages are <1 KB.

**Mitigation:**

- Use non-blocking I/O on pipes (overlapped I/O)
- Implement command queue depth limiting: if orchestrator has > 10 pending commands per client, drop lowest-priority commands (e.g., cosmetic animations)
- Monitor pipe buffer usage; alert if any client's pipe is > 80% full

**Status:** Non-blocking I/O implemented in `ipc::send_command()`. Queue limiting: pending.

---

### 9.3 Shared Memory Bus Contention

**Bottleneck:** Reading 36 shared memory buffers sequentially may take > 100 ms if any single buffer is slow to access (e.g., disk paged).

**Severity:** Low — shared memory is in process space, not disk-backed.

**Mitigation:**

- Pin shared memory buffers to physical RAM (VirtualLock on Windows)
- Parallelize reads across threads (one thread per group)
- Implement read timeouts: if a single read takes > 50 ms, skip that client for this tick

**Status:** VirtualLock not implemented. Timeout logic: pending.

---

### 9.4 Zone Transition Latency Spikes

**Bottleneck:** EQ's zone loading is unpredictable; some zones may take 15+ seconds to load due to network lag or asset streaming.

**Severity:** High — affects user experience and camp loop timing.

**Mitigation:**

- Pre-load zone assets during idle time (M8 feature: zone prediction)
- Stagger zone transitions across multiple clients to avoid server overload
- If transition exceeds 60 s, force-resync client state (DLL sends full game state dump)

**Status:** Stagger logic exists in `LaunchCoordinator`. Pre-load: pending (M8).

---

### 9.5 Crash Recovery Cascades

**Bottleneck:** If one client crashes during zone transition, recovery may trigger zone transitions for other clients, cascading failures.

**Severity:** High — can cause fleet-wide lockup.

**Mitigation:**

- Implement exponential backoff: first crash → retry at 5 s, second → 10 s, third → 20 s
- If > 3 crashes in 60 s, pause fleet and alert operator
- Isolate recovery: only restart one client at a time, stagger by 2 s

**Status:** Backoff logic in `HealthMonitor`. Cascade detection: pending.

---

### 9.6 DLL Hook Interference

**Bottleneck:** Multiple DLL hooks (navigation, combat, IPC) may interfere with each other's frame-time budgets if hooks run serially.

**Severity:** Medium — can add 5-10 ms latency per hook.

**Mitigation:**

- Profile hook execution time per frame
- If hook takes > 5 ms, defer non-critical operations to next frame
- Parallelize hooks across DLL worker threads (Windows ThreadPool)

**Status:** Profiling infrastructure: pending (M8).

---

## 10. Measurement Methodology

### 10.1 Telemetry Infrastructure

All SLA measurements rely on instrumentation in three components:

**Orchestrator (textquest.exe):**

```rust
struct LatencyMetrics {
    ipc_command_latency: HistogramMetric,        // Named pipe round-trip
    shared_memory_read_latency: HistogramMetric, // Per-client read time
    state_sync_delay: HistogramMetric,           // DLL write → read
    tui_frame_time: HistogramMetric,             // Per-frame render time
}
```

**DLL (injected):**

```c
struct DllMetrics {
    command_execution_latency,  // Command receive → execute
    state_write_latency,        // Capture → write to shared memory
    hook_execution_time,        // Per-hook profiling
}
```

**TUI:**

```rust
fn measure_frame_latency() -> Duration {
    let start = Instant::now();
    terminal.draw(...);
    let elapsed = start.elapsed();
    metrics.tui_frame_time.record(elapsed);
    elapsed
}
```

---

### 10.2 Automated SLA Validation

**Validation script:** `scripts/validate-slas.sh`

Runs stress tests and compares results against targets:

```bash
#!/bin/bash
# Assumes 36-client fleet is Live and idle in camp zone

echo "Testing IPC latency (1000 ping commands)..."
cargo run -- stress-test --mode ipc --count 1000

echo "Testing memory usage..."
Get-Process eq | Measure-Object -Property WorkingSet -Sum

echo "Testing TUI frame rate (60 seconds)..."
cargo run -- stress-test --mode ui --duration 60

echo "Testing state sync latency..."
cargo run -- stress-test --mode state-sync --count 100

echo "Generating SLA report..."
python3 scripts/generate_sla_report.py .autoship/metrics.json
```

**Output:** JSON report with measured p50/p99 vs target SLAs, pass/fail per metric.

---

### 10.3 Continuous Monitoring

SLA violations are logged to:

- **File:** `./logs/sla-violations.log` (append)
- **Discord:** Alert in `#sla-alerts` channel (if bridge active)

Example violation:

```
2026-04-15T14:23:42Z [SLA] IPC latency p99 exceeded: 15.3 ms > 10 ms target
  Context: 36 clients Live, TUI rendering at 28 FPS
  Client 5 command latency: 15.3 ms (outlier)
```

---

## 11. Tuning Recommendations

### 11.1 Reducing IPC Latency

**If p99 > 10 ms:**

1. Profile orchestrator with Windows Performance Toolkit (ETW)
   - Look for blocking I/O, context switches, GC pauses
2. Reduce command queue depth (drop cosmetic commands under load)
3. Move DLL hook logic to async worker threads
4. Profile TUI frame rendering; parallelize if needed

---

### 11.2 Reducing Memory Usage

**If resident memory > 50 MB per client:**

1. Check for memory leaks in DLL (Valgrind on Windows, or manual audit)
2. Reduce spawn cache size (keep only spawns within 200 units)
3. Compress buffs array (remove padding, use bitfields)
4. Implement shared memory pooling (reuse allocations)

---

### 11.3 Improving Zone Transition Speed

**If zone transitions > 30 s:**

1. Check server connection quality (latency, packet loss)
2. Verify DLL state capture isn't blocking zone loading
3. Pre-load zone geometry in background thread
4. Implement predictive zone transition (start loading next zone 10 s early)

---

## 12. Future SLAs (Post-M8)

| Metric                         | Target       | Status       |
| ------------------------------ | ------------ | ------------ |
| Navmesh pathfinding            | < 50 ms      | Pending (M8) |
| Combat rotation tick           | < 5 ms       | Pending (M8) |
| Skill cast chain latency       | < 100 ms     | Pending (M8) |
| Multi-group synchronization    | < 2 s        | Pending (M8) |
| Economy transaction throughput | > 100 tx/min | Pending (M9) |

---

## 13. References

- **Orchestration Loop Design:** `docs/design/orchestration-loop.md`
- **DLL Hook Specification:** `docs/specs/dll-hooks.md`
- **Performance Baselines:** `docs/performance-baselines.md`
- **IPC Protocol:** `textquest-common/src/ipc/protocol.rs`
- **Shared Memory Layout:** `textquest-common/src/state.rs`
- **TUI Framework:** `textquest/src/tui/run.rs`
- **CLI Stress Tests:** `textquest/src/bin/stress.rs` (placeholder)
