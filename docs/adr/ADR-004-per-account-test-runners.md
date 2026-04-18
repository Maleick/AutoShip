# ADR-004: Per-Account Test Runners

**Status:** Accepted  
**Date:** 2026-04-18  
**Author:** TextQuest Team

## Context

TextQuest orchestrates up to 36 accounts across 6 groups. Before launching autonomous sessions (combat, farming, vendoring), the system must validate that:

- IPC communication is working (spawn reads, item reads)
- Character state is correct (no stuck buffs, no incomplete transactions)
- Navigation is responsive (zone transitions, teleports)
- Combat rotation can cycle (abilities are available, mana/endurance is good)

Testing all 36 accounts serially takes too long (36 × 5 minutes = 3 hours). Testing all accounts in parallel on a single runner risks:

- **Shared state corruption** — One account's test modifies global state that another test depends on
- **Resource starvation** — 36 IPC clients competing for socket handles, memory
- **Cascading failures** — When one account's DLL crashes, it affects other DLL instances in the same process

## Decision

Implement **per-account test runners** — one async task per account, isolated:

```rust
pub struct AccountTestRunner {
    account: String,
    ipc_client: Arc<IpcClient>,
    scenarios: Vec<Box<dyn TestScenario>>,
}

impl AccountTestRunner {
    /// Run all scenarios for this account with a timeout.
    pub async fn run_all_scenarios(&self, timeout: Duration) -> TestRunResult {
        let mut result = TestRunResult::default();

        for scenario in &self.scenarios {
            let scenario_result = scenario
                .run(timeout / self.scenarios.len() as u32)
                .await;
            result.add_scenario(scenario_result);

            if scenario_result.errors.len() > 0 {
                tracing::warn!(
                    "Scenario {} failed for {}: {:?}",
                    scenario.name(),
                    self.account,
                    scenario_result.errors
                );
                // decide: fail fast vs. continue to next scenario
            }
        }

        result
    }
}
```

The **orchestrator** spawns one runner per account and awaits all runners in parallel:

```rust
pub async fn validate_all_accounts(
    accounts: Vec<String>,
    ipc_clients: HashMap<String, Arc<IpcClient>>,
    timeout: Duration,
) -> HashMap<String, TestRunResult> {
    let mut tasks = Vec::new();

    for account in accounts {
        let client = ipc_clients[&account].clone();
        let task = tokio::spawn(async move {
            let runner = AccountTestRunner::new(account.clone(), client);
            (account, runner.run_all_scenarios(timeout).await)
        });
        tasks.push(task);
    }

    let mut results = HashMap::new();
    for task in tasks {
        if let Ok((account, result)) = task.await {
            results.insert(account, result);
        }
    }

    results
}
```

## Rationale

1. **Isolation** — Each account's test runs in its own task with its own IPC client. Failure in one account doesn't crash another.

2. **Parallelism** — All 36 accounts test simultaneously. Total time ≈ longest single account's test (~5 min) instead of 3 hours serial.

3. **Clear Ownership** — Each runner owns its scenarios and its IPC client. No shared mutable state (Arc<Mutex<>> is only used for read-only shared data like account config).

4. **Recoverable** — If one account fails validation, the orchestrator can retry just that account or skip it, without re-testing all others.

5. **Scalable** — Runners work the same whether testing 6 accounts or 36. Adding a new account adds a new task; no architectural changes.

## Implementation Notes

- **Fail-fast vs. continue** — Each runner can either stop after the first failing scenario or continue to collect all failures. Configurable per-runner.
- **Timeout slicing** — If a runner has 30 seconds and 6 scenarios, each scenario gets ~5 seconds. If a scenario finishes early, the saved time isn't allocated to later scenarios (to preserve the orchestrator's timeout invariant).
- **IPC client per account** — Each account must have a unique IPC client (listening on a different Unix socket or TCP port), otherwise concurrent message sends will interleave.
- **Result aggregation** — The orchestrator collects `HashMap<String, TestRunResult>` and summarizes: "34/36 accounts passed, 2 failed". Failed accounts are reported with their scenario failures.

Example test run result:

```json
{
  "account": "frostreaver01",
  "passed": true,
  "scenarios": [
    {
      "name": "spawn_read_roundtrip",
      "success": true,
      "duration_ms": 2340,
      "metrics": {
        "roundtrip_ms": { "histogram": [10, 12, 11, 10] }
      }
    },
    {
      "name": "vendor_transaction",
      "success": true,
      "duration_ms": 1200,
      "metrics": {}
    }
  ],
  "total_duration_ms": 5000
}
```

## Alternatives Considered

1. **Single global runner with account iteration** — One runner loops over all accounts serially. Simple but slow (3+ hours for 36 accounts).
2. **Thread pool with shared state** — Use a Rayon or threadpool executor. Requires careful locking; risk of deadlocks if scenarios wait on shared resources.
3. **Worker queue with backpressure** — Spawn workers up to a limit (e.g., 4 concurrent runners). Good for resource-constrained systems; adds complexity.

## Related ADRs

- ADR-002: Test Scenario Trait Architecture (runners execute TestScenario implementations)
- ADR-003: JSON Event Streaming (test results are logged as events)

## References

- `textquest/src/testing/runner.rs` — AccountTestRunner implementation
- `textquest/src/orchestrator.rs` — validate_all_accounts orchestration
