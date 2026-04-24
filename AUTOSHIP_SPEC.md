# AutoShip Specification

Platform: OpenCode-only plugin.

AutoShip is an autonomous GitHub issue → pull request pipeline. It reads open issues labeled `agent:ready`, plans them in ascending issue-number order, blocks unsafe/evasion-prone work for human review, dispatches OpenCode workers, verifies results, opens PRs, monitors CI, and reconciles local state.

## Runtime Model

- OpenCode is the only supported worker runtime.
- `openai/gpt-5.5` is the default planner, coordinator, orchestrator, and reviewer model.
- `openai/gpt-5.5-fast` is rejected.
- Worker models come from the live `opencode models` inventory.
- Free models are selected by default.
- Operator-selected models, including Spark and Go-provider models, are allowed when present in the live inventory.
- Worker selection scores task compatibility, cost class, configured strength, and previous success/failure history.

## Concurrency

- Default active worker cap: 15.
- `runner.sh` enforces the cap before starting queued workspaces.
- Dispatch can queue beyond the active cap; queued work starts when capacity is available.

## State

- Runtime state is local to `.autoship/` and should not be committed.
- Durable recovery uses GitHub issue labels plus workspace status files.
- `.autoship/model-routing.json` is user-editable and preserved by setup unless refresh is requested.

## Safety

AutoShip blocks issues that mention anti-cheat evasion, stealth, VM or fingerprint evasion, shellcode, hook signature evasion, detour hiding, or similar abuse-prone work.

## Verification

Completed work must be independently reviewed before PR creation. The reviewer role uses the configured OpenCode reviewer model, defaulting to `openai/gpt-5.5`.
