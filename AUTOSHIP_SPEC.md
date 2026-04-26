# AutoShip Specification

Platform: OpenCode-only plugin.

AutoShip is an autonomous GitHub issue → pull request pipeline. It reads open issues labeled `agent:ready`, plans them in ascending issue-number order, dispatches OpenCode workers, verifies results, opens PRs, monitors CI, and reconciles local state.

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
- `AUTOSHIP_PLAN_ORDER` can switch planning from ascending to `cadence`, `updated`, or `descending`.
- `AUTOSHIP_CHECKPOINT_EVERY` enables checkpoint timestamps during long runner loops.

## State

- Runtime state is local to `.autoship/` and should not be committed.
- Durable recovery uses GitHub issue labels plus workspace status files.
- Per-issue markdown records live under `.autoship/items/` and can be used for audit/resume.
- Structured events are JSON-lines under `.autoship/logs/events.jsonl`.
- `.autoship/model-routing.json` is user-editable and preserved by setup unless refresh is requested.

## Verification

Completed work must be independently reviewed before PR creation. The reviewer role uses the configured OpenCode reviewer model, defaulting to `openai/gpt-5.5`.
Reviewer output should include a JSON object matching `schema/reviewer-decision.json` plus a `VERDICT: PASS` or `VERDICT: FAIL` line.
Issue bodies are sanitized before prompt insertion, acceptance criteria are extracted into normalized JSON, diff-size and checksum guardrails run before review, and flaky test commands are retried once by default.

## Safety Labels

AutoShip skips issues with protected labels by default: `do-not-automate`, `needs-human`, `wontfix`, `discussion`, and `security`. Operators may tune the deny list in `.autoship/config.json`.

## Release Automation

PRs labeled `autoship:auto-merge` may be merged by `auto-merge.sh` after CI and reviewer checks pass. Workflow concurrency groups are separated by workflow mode to avoid plan/apply collisions.
