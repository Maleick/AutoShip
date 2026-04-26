# AutoShip Specification

Platform: OpenCode-only plugin.

AutoShip is an autonomous GitHub issue → pull request pipeline. It reads open issues labeled `agent:ready`, plans them in ascending issue-number order, dispatches OpenCode workers, verifies results, opens PRs, monitors CI, and reconciles local state.

## Runtime Model

- OpenCode is the only supported worker runtime.
- Role models come from the live `opencode models` inventory and project-local `.autoship/model-routing.json`.
- Setup prefers capable free or OpenCode Go Kimi/Kimmy/Ling 2.6-family role models when available and prompts for orchestrator/reviewer on first run.
- `openai/gpt-5.5-fast` is rejected.
- Worker models come from the live `opencode models` inventory.
- Free models are selected by default.
- Operator-selected models, including Spark and Go-provider models, are allowed when present in the live inventory.
- Worker selection scores task compatibility, cost class, configured strength, previous success/failure history, and deterministic issue-number rotation across compatible workers.
- Complex tasks without a sufficiently strong compatible worker use the configured orchestrator model as an advisor fallback.

## Concurrency

- Default active worker cap: 15.
- `runner.sh` enforces the cap before starting queued workspaces.
- Dispatch can queue beyond the active cap; queued work starts when capacity is available.

## State

- Runtime state is local to `.autoship/` and should not be committed.
- Durable recovery uses GitHub issue labels plus workspace status files.
- `.autoship/model-routing.json` is user-editable and preserved by setup unless refresh is requested.

## Verification

Completed work must be independently reviewed before PR creation. The reviewer role uses the configured OpenCode reviewer model from `.autoship/model-routing.json`.
