# Overnight Testing - Issue Slices & Sub-Issues

## M7 Milestone: Overnight Testing Infrastructure (M7)

**Epic Goal**: Enable fully autonomous login→test→logout loops that run 8+ hours overnight without manual intervention.

**Dependencies**: M2.5 (login automation) ✅, M4 (combat) ✅

**Critical Path**: #861 → #862 → #863 → #864

---

## PHASE 1: Foundation (Weeks 1-2)

### #861: Logout/Disconnect Mechanism [PARENT]

**Dependencies**: None  
**Blocks**: #862, #863, #864  
**Labels**: `M7`, `testing`, `enhancement`, `phase-1`

#### Sub-Issues

**#861.1: Add Logout phases to LoginPhase enum [LOWEST]**
- Add `Logout`, `DisengagingFromCombat`, `LeavingGroup`, `QuittingGame`, `ProcessExiting` variants
- Update `LoginPhase::is_terminal()` to include Logout states
- Location: `textquest-common/src/login.rs`
- **Labels**: `M7.logout`, `infrastructure`, `low-effort`

**#861.2: Implement LogoutStateMachine [LOW]**
- Create `textquest/src/launcher/logout_sm.rs`
- Implement state transitions with timeout detection
- Support `advance(LogoutEvent) -> LogoutAction` pattern (like LoginStateMachine)
- MAX_ATTEMPTS for logout retries (default: 2)
- **Labels**: `M7.logout`, `core`, `state-machine`

**#861.3: Implement LogoutSequencer [LOW]**
- Create `textquest/src/launcher/logout_sequencer.rs`
- Similar to `PostLoginSequencer`
- Coordinate: cease combat → leave group → send /quit → wait for exit
- Handle timeout + force-kill fallback
- **Labels**: `M7.logout`, `core`, `sequencing`

**#861.4: Add QuitGame IPC command [LOW]**
- Add `Command::QuitGame { target_pid: u32 }` to `textquest-common/src/ipc.rs`
- Add `LogoutEvent` enum to shared types
- DLL-side handler: receive QuitGame → execute `/quit`
- **Labels**: `M7.logout`, `ipc`, `infrastructure`

**#861.5: Integration test: login→logout cycle [MED]**
- Create `textquest/tests/integration_logout.rs`
- Test single client: login → wait for InWorld → logout → verify exit
- Test 3 clients: parallel login→logout
- Mock processes on macOS, live on Windows (behind `#[cfg(windows)]`)
- **Labels**: `M7.logout`, `testing`, `integration`

**#861.6: CLI support for manual logout [LOW]**
- Add `textquest --logout-all` command
- Add `textquest --logout <pid>` for single client
- Useful for operator safety/cleanup
- **Labels**: `M7.logout`, `cli`, `nice-to-have`

---

### #862: Test Scenario Framework & Harness [PARENT]

**Dependencies**: #861 (logout mechanism)  
**Blocks**: #863, #864  
**Labels**: `M7`, `M7.harness`, `testing`, `enhancement`, `phase-2`

#### Sub-Issues

**#862.1: Create TestScenario trait [LOW]**
- Create `textquest/src/testing/scenario.rs`
- Define trait: `name() -> &str`, `run(duration) -> ScenarioResult`
- Define `ScenarioResult` struct with metrics HashMap
- **Labels**: `M7.harness`, `trait`, `low-effort`

**#862.2: Implement CampLoopScenario [MED]**
- Create camp-loop scenario
- Metrics: pulls, mobs_killed, total_damage, deaths, duration
- Config: camp_name, max_duration, pull_count_target
- Should integrate with existing `CampLoop` state machine
- **Labels**: `M7.harness`, `scenario`, `camp-loop`

**#862.3: Implement NavigationScenario [MED]**
- Create navigation scenario
- Metrics: waypoints_visited, navigation_failures, stuck_detections, duration
- Config: route (list of waypoints/camps)
- Should use existing `GroupRouter` and Navigator FSM
- **Labels**: `M7.harness`, `scenario`, `navigation`

**#862.4: Implement CombatRotationScenario [MED]**
- Create combat scenario (single-pull focus)
- Metrics: rotations_completed, spell_casts, melee_hits, dps, duration
- Config: target_mob_name, max_duration
- Useful for per-class validation
- **Labels**: `M7.harness`, `scenario`, `combat`

**#862.5: Implement mock/stub scenarios for CI testing [LOW]**
- Create `MockScenario` that completes instantly
- Create `CountdownScenario` for timing tests
- Usable on all platforms (macOS safe)
- **Labels**: `M7.harness`, `testing`, `ci`

**#862.6: Implement TestLoopRunner [MED]**
- Create `textquest/src/testing/runner.rs`
- Coordinate: login → spawn scenarios → await all → logout → repeat
- Track iteration count, elapsed time, success rate
- Handle graceful shutdown (Ctrl+C → logout all)
- **Labels**: `M7.harness`, `core`, `coordination`

**#862.7: Implement metrics collection framework [LOW]**
- Create `textquest/src/testing/metrics.rs`
- Define `MetricValue` enum (Counter, Gauge, Histogram)
- Aggregation: sum, average, percentiles
- Exportable to JSON
- **Labels**: `M7.harness`, `metrics`, `infrastructure`

**#862.8: Unit tests for TestScenario trait [LOW]**
- Test CampLoopScenario initialization
- Test NavigationScenario route parsing
- Test metrics aggregation (sum, avg, percentile)
- **Labels**: `M7.harness`, `testing`, `unit`

**#862.9: Integration test: full test loop (mock) [MED]**
- Test 1 iteration: login → camp-loop scenario (1 min) → logout
- Test 3 iterations with loop
- Use mock scenarios on macOS, camp loop on Windows
- **Labels**: `M7.harness`, `testing`, `integration`

---

## PHASE 3: Output Capture (Weeks 3)

### #863: Structured Logging & Output Capture [PARENT]

**Dependencies**: #862 (test harness)  
**Blocks**: #864  
**Labels**: `M7`, `testing`, `enhancement`, `phase-3`

#### Sub-Issues

**#863.1: Session directory management [LOW]**
- Create `textquest/src/testing/output.rs`
- Generate UUID-based session directory: `overnight-test-runs/<ISO8601-timestamp>/`
- Create subdirs: `client-logs/`, `scenarios/`
- Create `metadata.json` with runner config
- **Labels**: `M7.output`, `infrastructure`, `low-effort`

**#863.2: Structured JSON event logging [MED]**
- Append-only `events.jsonl` per session
- Event schema: `timestamp`, `session_id`, `iteration`, `event_type`, `details`
- Event types: `LoginStart`, `LoginComplete`, `ScenarioStart`, `ScenarioPull`, `ScenarioMobKill`, `ScenarioComplete`, `LogoutStart`, `LogoutComplete`, `Error`
- Thread-safe EventSink (Arc<Mutex<>>)
- **Labels**: `M7.output`, `logging`, `infrastructure`

**#863.3: Scenario metrics aggregation [MED]**
- Create `textquest/src/testing/metrics_output.rs`
- Aggregate per-scenario: pull count, kill count, DPS, duration, errors
- Write `metrics.json` at session end
- Include percentiles: p50, p95, p99
- **Labels**: `M7.output`, `metrics`, `analysis`

**#863.4: HTML report generation [MED]**
- Create `textquest/src/testing/report.rs`
- Generate `report.html` with charts:
  - Timeline: pulls/hour, kill/hour
  - Success/failure bars
  - Error distribution pie chart
  - Performance metrics table
- Use minimal HTML (no external dependencies)
- **Labels**: `M7.output`, `reporting`, `html`

**#863.5: Log rotation & cleanup [LOW]**
- Implement automatic session cleanup (keep last N sessions)
- Default: keep last 10 sessions
- Configurable via `--keep-sessions N`
- **Labels**: `M7.output`, `operations`, `cleanup`

**#863.6: JSON export of full session [LOW]**
- Export all events + metrics to `session-export.json`
- Useful for external analysis tools
- Include account names, durations, success rates
- **Labels**: `M7.output`, `export`, `integration`

**#863.7: Unit tests for event logging [LOW]**
- Test event serialization
- Test directory creation
- Test metrics aggregation
- **Labels**: `M7.output`, `testing`, `unit`

**#863.8: Integration test: output capture [MED]**
- Run mock loop, verify events.jsonl populated
- Verify metrics.json has aggregated stats
- Verify report.html generated and parseable
- **Labels**: `M7.output`, `testing`, `integration`

---

## PHASE 3 (continued): CLI Integration (Week 3-4)

### #864: Overnight Test Runner CLI [PARENT]

**Dependencies**: #863 (output capture)  
**Blocks**: None (other components can ship separately)  
**Labels**: `M7`, `testing`, `enhancement`, `phase-3`, `cli`

#### Sub-Issues

**#864.1: Add overnight-test subcommand [MED]**
- Modify `textquest/src/main.rs` to add subcommand dispatch
- Create `textquest/src/cli/overnight.rs`
- Parse flags: `--duration`, `--accounts`, `--scenarios`, `--output-dir`, `--log-level`
- Validate flags against config
- **Labels**: `M7.cli`, `cli`, `dispatch`

**#864.2: Config loading and account resolution [LOW]**
- Load `config/frostreaver.toml` and `config/accounts.toml`
- Resolve `--accounts PROFILE` to account list
- Validate account exists in credential store
- **Labels**: `M7.cli`, `config`, `integration`

**#864.3: Dry-run mode (--dry-run) [LOW]**
- Parse config and print test plan
- Show: accounts, scenarios, duration, estimated completion time
- Exit without running
- **Labels**: `M7.cli`, `validation`, `ux`

**#864.4: Multi-runner spawning [MED]**
- Spawn one TestLoopRunner per account (or group)
- Each runner runs in tokio task
- Track task handles for shutdown
- **Labels**: `M7.cli`, `concurrency`, `spawning`

**#864.5: Real-time progress monitoring [MED]**
- Every 30 seconds, print progress line
- Format: `[02:30 / 08:00] Acc1: 6 iter, 27 pulls | Acc2: 5 iter, 23 pulls`
- Update on-screen (no log spam)
- **Labels**: `M7.cli`, `monitoring`, `ux`

**#864.6: Graceful Ctrl+C shutdown [MED]**
- Install signal handler (SIGINT/SIGTERM)
- On signal: set shutdown flag on all runners
- Runners gracefully logout → exit
- Generate final report before exit
- **Labels**: `M7.cli`, `signals`, `shutdown`

**#864.7: Exit codes and error reporting [LOW]**
- Exit 0: all iterations completed successfully
- Exit 1: critical failures (login 0%, logout hangs)
- Exit 2: config/validation errors
- Print error summary before exit
- **Labels**: `M7.cli`, `error-handling`, `ux`

**#864.8: Integration test: full CLI flow [MED]**
- Test: `textquest overnight-test --duration 0.016 --all-accounts --dry-run`
  - Verify plan printed, no launches
- Test: `textquest overnight-test --duration 0.016 --accounts MainRaid` (1 minute actual run)
  - Verify login → 1 iteration → logout
  - Verify report generated
- Test: Ctrl+C during run
  - Verify graceful shutdown and final report
- **Labels**: `M7.cli`, `testing`, `integration`, `e2e`

---

## PHASE 4: Reliability & Safety (Week 4+)

### #865: Error Handling & Recovery [PARENT]

**Dependencies**: #861, #862, #863, #864 (all phases)  
**Blocks**: Release  
**Labels**: `M7`, `error-handling`, `stability`, `phase-4`

#### Sub-Issues

**#865.1: Login failure recovery [MED]**
- Detect login failures mid-loop
- Implement exponential backoff (1s → 2s → 4s)
- Max retries: 3 per account
- Log failure reason and skip iteration
- **Labels**: `M7.stability`, `error-handling`, `recovery`

**#865.2: Circuit breaker for mass failures [MED]**
- If 3+ accounts fail login in 5-minute window
- Pause all launches for 5 minutes
- Alert operator (log warning)
- Resume after timeout
- **Labels**: `M7.stability`, `circuit-breaker`, `resilience`

**#865.3: Process timeout detection [LOW]**
- Monitor process exit for logout (5 second timeout)
- If timeout: force-kill and log
- Verify no orphaned processes
- **Labels**: `M7.stability`, `timeout`, `cleanup`

**#865.4: Crash recovery & checkpoint [MED]**
- Periodically save session state to `session.checkpoint.json`
- On restart with `--resume`: load checkpoint and continue
- Skip completed iterations
- **Labels**: `M7.stability`, `checkpointing`, `recovery`

**#865.5: Memory leak detection [LOW]**
- Monitor process memory per iteration
- Alert if memory grows >20% per iteration
- Can trigger auto-logout + restart
- **Labels**: `M7.stability`, `memory`, `monitoring`

---

### #866: Account Safety & Detection [PARENT]

**Dependencies**: #861, #862, #863, #864  
**Blocks**: Release  
**Labels**: `M7`, `safety`, `phase-4`

#### Sub-Issues

**#866.1: Ban/suspension detection [MED]**
- Monitor for "Account locked" or "Banned" messages
- Immediately stop that account from further logins
- Alert operator and log
- **Labels**: `M7.safety`, `detection`, `critical`

**#866.2: GM alert detection [LOW]**
- Monitor for GM tells or raid warnings
- Log character name and message
- Suggest manual intervention
- **Labels**: `M7.safety`, `detection`, `gm`

**#866.3: Rate limiting [LOW]**
- Enforce max login attempts per account per hour
- Config: default 10 logins/hour
- Prevent account flagging
- **Labels**: `M7.safety`, `rate-limiting`, `prevention`

**#866.4: Server status monitoring [LOW]**
- Check server status before launching
- If server down, pause launches until recovery
- Config: check interval (default: 5 min)
- **Labels**: `M7.safety`, `server-status`, `monitoring`

---

### #867: Monitoring & Observability [PARENT]

**Dependencies**: #863 (output capture)  
**Blocks**: Optional (nice-to-have)  
**Labels**: `M7`, `monitoring`, `observability`, `phase-4`

#### Sub-Issues

**#867.1: Real-time metrics export (Prometheus) [MED]**
- Expose `/metrics` endpoint (if running web server)
- Export: login success rate, scenario completion, loop iterations, error count
- Format: Prometheus text format
- **Labels**: `M7.monitoring`, `prometheus`, `metrics`

**#867.2: Anomaly detection [MED]**
- Alert if DPS drops >30% compared to baseline
- Alert if pull rate drops >20%
- Alert if error rate exceeds 5%
- **Labels**: `M7.monitoring`, `anomaly`, `alerting`

**#867.3: Email/webhook notifications [LOW]**
- Send summary email at end of run
- Webhook POST on critical failures
- Config: SMTP or webhook URL
- **Labels**: `M7.monitoring`, `notifications`, `integration`

**#867.4: Live web dashboard (optional) [LOW]**
- Real-time metrics display
- List of running accounts
- Ability to pause/resume individual accounts
- **Labels**: `M7.monitoring`, `dashboard`, `nice-to-have`

---

### #868: Documentation & Runbooks [PARENT]

**Dependencies**: All other issues (after implementation)  
**Blocks**: Release  
**Labels**: `M7`, `documentation`, `phase-4`

#### Sub-Issues

**#868.1: Operator runbook [MED]**
- Step-by-step guide to start overnight test
- Example commands with expected output
- Troubleshooting decision tree (login fails → check X → try Y)
- **Labels**: `M7.docs`, `runbook`, `operations`

**#868.2: FAQ & troubleshooting guide [LOW]**
- Common issues: hung processes, memory bloat, login timeouts
- Solutions and workarounds
- **Labels**: `M7.docs`, `faq`, `troubleshooting`

**#868.3: Performance tuning guide [LOW]**
- How to tune duration, scenario count, resource limits
- Memory/CPU profiles
- Recommendations for different server loads
- **Labels**: `M7.docs`, `tuning`, `performance`

**#868.4: Video walkthrough (optional) [LOW]**
- 5-10 minute demo video
- Setup, config, run, review results
- **Labels**: `M7.docs`, `video`, `nice-to-have`

---

### #869: CI Integration & Nightly Jobs [PARENT]

**Dependencies**: All core issues (#861-864)  
**Blocks**: Optional  
**Labels**: `M7`, `ci`, `automation`, `phase-4`

#### Sub-Issues

**#869.1: GitHub Actions workflow for nightly tests [MED]**
- Scheduled job: every night at 11 PM CT
- Run on self-hosted Windows runner (Frostreaver)
- Duration: 4 hours (sample subset of fleet)
- **Labels**: `M7.ci`, `github-actions`, `automation`

**#869.2: Nightly test results archiving [LOW]**
- Upload session reports to GitHub Actions artifacts
- Retain for 30 days
- Link to PR/issue for visibility
- **Labels**: `M7.ci`, `artifacts`, `archiving`

**#869.3: Nightly test alerts [LOW]**
- If > 20% failure rate: post comment on latest PR
- If login 0%: page on-call engineer
- Webhook to Discord
- **Labels**: `M7.ci`, `alerts`, `notifications`

---

## Cross-Cutting Concerns (Standalone Issues)

### #870: Testing Infrastructure - Mocks & Stubs [PARENT]

**Labels**: `M7`, `testing`, `infrastructure`

**Sub-Issues**:
- **#870.1: Mock processes for macOS testing [LOW]**
  - Spawn dummy processes with same lifecycle
  - Allows full test coverage on CI
  - **Labels**: `M7.testing`, `mocks`, `macOS`

- **#870.2: Stubbed scenario implementations [LOW]**
  - `MockScenario`, `CountdownScenario`, `FastFailScenario`
  - For unit & integration tests
  - **Labels**: `M7.testing`, `stubs`, `test-helpers`

- **#870.3: Test data generators [LOW]**
  - Generate realistic spawn lists, combat events
  - Useful for scenario testing
  - **Labels**: `M7.testing`, `generators`, `test-data`

---

### #871: Configuration Management [PARENT]

**Labels**: `M7`, `configuration`, `infrastructure`

**Sub-Issues**:
- **#871.1: Test profile definitions [MED]**
  - TOML files for test scenarios per account
  - E.g., `config/test-profiles/daily-farm.toml`
  - Define: which scenarios, duration, camp names
  - **Labels**: `M7.config`, `profiles`, `scenario-config`

- **#871.2: Scenario parameter templates [LOW]**
  - Parameterized scenarios: camp name, max pulls, difficulty
  - Load from TOML
  - **Labels**: `M7.config`, `parameters`, `customization`

- **#871.3: Resource limits configuration [LOW]**
  - Memory per process, max file handles, CPU affinity
  - Config: `max_memory_mb`, `max_open_files`
  - **Labels**: `M7.config`, `resources`, `limits`

---

## Tag Taxonomy

```
Phase Tags:
  phase-1: Foundation (logout, test harness)
  phase-2: Output capture
  phase-3: CLI integration
  phase-4: Validation, polish, optional features

Feature Tags:
  M7.logout: Logout mechanism issues
  M7.harness: Test scenario framework
  M7.output: Logging & output capture
  M7.cli: CLI integration
  M7.stability: Error handling & recovery
  M7.safety: Account safety & detection
  M7.monitoring: Observability & monitoring
  M7.docs: Documentation
  M7.ci: CI/CD integration
  M7.testing: Testing infrastructure
  M7.config: Configuration management

Effort Tags:
  low-effort: < 1 day
  low: 1-2 days
  med: 2-5 days
  high: 5+ days

Type Tags:
  enhancement: New feature
  testing: Test-related
  bug: Bug fix
  documentation: Docs
  infrastructure: Infrastructure
  integration: System integration

Relative Priority (for ordering):
  critical: Blocks release, must fix
  important: Blocks feature, strongly wanted
  nice-to-have: Polish, optional

Platform Tags:
  windows-only: Windows-specific work
  all-platforms: Cross-platform
  ci: CI-related

Example full label set:
  enhancement, M7, M7.logout, phase-1, testing, low, critical
```

---

## Critical Path & Sequencing

```
┌─ #861 (Logout) ─────┐
│                     ├─> #862 (Harness) ─┐
│                     │                    ├─> #863 (Output) ─┐
└─────────────────────┘                    │                  ├─> #864 (CLI)
                                           └──────────────────┘

Parallel (after core 4):
  #865 (Error Handling)
  #866 (Safety)
  #867 (Monitoring)
  #868 (Docs)
  #869 (CI)
  #870-871 (Infrastructure)
```

---

## Success Metrics

- [ ] All 4 core issues (#861-864) shipped and working
- [ ] At least 1 error handling issue (#865.x) shipped
- [ ] Full documentation (#868) written
- [ ] Manual testing on Windows for 2 hours completed
- [ ] Zero critical bugs in nightly runs for 1 week
