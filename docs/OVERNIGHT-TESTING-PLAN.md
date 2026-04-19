# Automated Overnight Testing Infrastructure

**Goal**: Enable fully autonomous login→test→logout loops that run overnight without manual intervention, capturing all output for later analysis.

## Overview

The overnight testing system will consist of four interconnected components:

1. **Logout/Disconnect Mechanism** — graceful client exit with proper cleanup
2. **Test Loop Harness** — orchestrator-side framework for coordinating login→scenario→logout cycles
3. **Output Capture Infrastructure** — structured logging, metrics, and test result archiving
4. **Overnight Test Runner** — CLI tool to spawn, monitor, and collect results from autonomous test loops

## Architecture

```
OvernightTestRunner (CLI)
├── Spawns N test loop instances
├── Each instance coordinates:
│   ├── Login (via LoginCoordinator)
│   ├── TestScenario (camp loop, navigation, combat, etc.)
│   ├── Logout (via LogoutSequencer)
│   └── Result capture + metrics
└── Collects results, generates report
```

## Component 1: Logout/Disconnect Mechanism

### Current State
- Login automation is complete (M2.5)
- No logout sequence exists
- EQ requires explicit `/quit` command or window close

### Design
- **LogoutSequencer** (similar to PostLoginSequencer in `launcher/post_login.rs`)
  - Coordinates group leave, stance changes, trading prohibition
  - Sends `/quit` command via IPC
  - Monitors for process exit with timeout
  - Fallback: force-kill if quit hangs

### Files to Create
- `textquest/src/launcher/logout_sm.rs` — logout state machine
- `textquest/src/launcher/logout_sequencer.rs` — post-logout coordination
- Add `Logout` variant to `LoginPhase` enum (shared types)
- Add `QuitGame` command to IPC protocol

### Expected States
```
InWorld
  ↓
DisengagingFromCombat (stop combat, cease pulling)
  ↓
LeavingGroup (group leave command)
  ↓
QuittingGame (send /quit)
  ↓
ProcessExiting (monitor for clean exit)
  ↓
Complete
```

## Component 2: Test Loop Harness

### Design
Provides a framework for running repeatable autonomous test scenarios:

**`TestScenario` trait** (in `textquest/src/testing/`)
```rust
pub trait TestScenario: Send {
    /// Human-readable scenario name
    fn name(&self) -> &str;
    
    /// Run the scenario for up to `duration`.
    /// Returns metrics summary.
    async fn run(&mut self, duration: Duration) -> ScenarioResult;
}
```

**Built-in scenarios**:
- `CampLoopScenario` — run camp loop for N minutes, collect pull/fight/loot metrics
- `NavigationScenario` — test waypoint navigation between camps
- `CombatRotationScenario` — verify class combat rotations under load
- `ZoningScenario` — test zone transitions (M7+)

**`TestLoopRunner`** (in `textquest/src/testing/`)
```rust
pub struct TestLoopRunner {
    login_coordinator: Arc<LaunchCoordinator>,
    logout_sequencer: Arc<LogoutSequencer>,
    scenarios: Vec<Box<dyn TestScenario>>,
    iterations: u32,
    output_dir: PathBuf,
}

impl TestLoopRunner {
    /// Run the full loop: login → scenarios → logout → repeat
    pub async fn run_loop(&mut self) -> TestLoopResult;
}
```

### Files to Create
- `textquest/src/testing/mod.rs`
- `textquest/src/testing/scenario.rs` — TestScenario trait + builtins
- `textquest/src/testing/runner.rs` — TestLoopRunner
- `textquest/src/testing/metrics.rs` — scenario result collection

## Component 3: Output Capture Infrastructure

### Logging Strategy
- **Structured JSON logs** for machine parsing
- **Append-only event streams** per test loop session
- **Real-time file rotation** to avoid disk bloat on overnight runs

### Session Layout
```
overnight-test-runs/
├── 2026-04-12T02-00-00Z/
│   ├── metadata.json (runner config, target server, account list)
│   ├── events.jsonl (structured event stream)
│   ├── metrics.json (aggregated per-scenario metrics)
│   ├── client-logs/
│   │   ├── account-01.log
│   │   └── account-02.log
│   └── orchestrator.log
└── 2026-04-12T08-00-00Z/
    └── ...
```

### Event Types
```json
{
  "timestamp": "2026-04-12T02:00:15.123Z",
  "session_id": "uuid",
  "event_type": "LoginComplete|ScenarioStart|ScenarioPull|ScenarioMobKill|LogoutStart|LogoutComplete|Error",
  "details": { /* scenario-specific */ }
}
```

### Files to Create
- `textquest/src/testing/output.rs` — session directory setup, event sink
- `textquest/src/testing/report.rs` — report generation (HTML, JSON summary)

## Component 4: Overnight Test Runner (CLI)

### Entry Point
```bash
textquest overnight-test [OPTIONS]
  --duration HOURS          How long to run (default: 8 hours)
  --accounts PROFILE        Profile group to test (default: all)
  --scenarios SCENARIO      Which scenarios to run (default: camp-loop,navigation)
  --output-dir PATH         Where to store results (default: ./overnight-test-runs)
  --log-level LEVEL         Tracing level (default: info)
```

### Behavior
1. Load config, accounts, and test profiles
2. Spawn one TestLoopRunner per account (or group)
3. Each runner: login → run scenarios → logout → loop
4. Aggregate metrics in real-time
5. On interrupt (Ctrl+C): gracefully logout all, generate final report
6. Exit with non-zero if any critical failures

### Files to Create/Modify
- `textquest/src/main.rs` — add `overnight-test` subcommand
- `textquest/src/cli/overnight.rs` — CLI handler

## Testing Strategy

### Unit Tests
- LoginPhase transitions including Logout variants
- LogoutSequencer state machine
- TestScenario trait implementations
- Metrics aggregation

### Integration Tests
- Full login→logout cycle (stubbed on macOS, live on Windows)
- Test loop with mock scenarios
- Output capture and report generation

### Manual Validation (Windows only)
- Run overnight test on real server for 1–2 hours
- Verify clean logins/logouts
- Check account safety (no bans, no stuck characters)
- Validate log output completeness

## Phase Plan

### Phase 1: Foundation (Week 1)
- [ ] Logout state machine + IPC command
- [ ] LogoutSequencer implementation
- [ ] Basic integration tests for login→logout cycle

### Phase 2: Test Harness (Week 2)
- [ ] TestScenario trait + CampLoopScenario
- [ ] TestLoopRunner implementation
- [ ] Basic metrics collection

### Phase 3: Output & CLI (Week 3)
- [ ] Structured logging and event streams
- [ ] Session directory setup and rotation
- [ ] Report generation
- [ ] `overnight-test` CLI subcommand

### Phase 4: Validation & Polish (Week 4)
- [ ] Manual testing on Windows with live server
- [ ] Performance profiling (memory, CPU over 8-hour run)
- [ ] Documentation: runbook + troubleshooting guide
- [ ] CI integration: nightly test job template

## Success Criteria

### Functional
- [ ] Autonomous login→test→logout loops run for 8+ hours without manual intervention
- [ ] All test scenario metrics are captured correctly
- [ ] Logouts are always clean (no hung processes)
- [ ] Output is fully captured and searchable

### Reliability
- [ ] At least 99% login success rate on stable server
- [ ] At least 95% scenario completion rate
- [ ] Zero data loss on Ctrl+C (graceful shutdown)
- [ ] Logs rotate without gaps or duplication

### Usability
- [ ] Single command to start overnight testing
- [ ] Human-readable report generated automatically
- [ ] Clear troubleshooting guide for common failure modes

## Open Questions

1. Should each account run independently, or coordinate as a group?
2. What's the max loop duration before a forced logout (memory safety)?
3. Should we integrate with CI (GitHub Actions overnight job) or keep it local?
4. Do we need real-time monitoring/alerting, or batch reporting is sufficient?

## Related Issues

- See GitHub issues #861 (logout), #862 (test harness), #863 (output capture), #864 (CLI integration) for detailed task breakdowns and sub-issues.
- Phase 1-4 implementation tracked via M7 milestone with 101 total issues across all phases.
