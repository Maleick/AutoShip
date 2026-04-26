# AutoShip Agent Guide

AutoShip is an OpenCode-only GitHub issue → pull request orchestration plugin.

See [AGENT_CATALOG.md](AGENT_CATALOG.md) for the specialized agent roles, inputs, outputs, boundaries, and default model families.

## Runtime Policy

- OpenCode is the only supported worker runtime.
- `openai/gpt-5.5` is the planner, lead, coordinator, orchestrator, reviewer, simplifier, and release role model.
- `openai/gpt-5.5-fast` is not allowed.
- Worker models come from live `opencode models` inventory and `.autoship/model-routing.json`.
- Default active worker cap is 15.
- Plan `agent:ready` issues in ascending issue-number order (unless `AUTOSHIP_PLAN_ORDER` is set).

## Available Hooks

Core orchestration hooks in `hooks/opencode/`:
- `plan-issues.sh` - Plan eligible issues from GitHub
- `dispatch.sh` - Dispatch workers to issues
- `runner.sh` - Execute workers
- `reviewer.sh` - Review worker output
- `create-pr.sh` - Create PR from approved work
- `verify-result.sh` - Verify worker results
- `monitor-agents.sh` - Monitor running workers
- `reconcile-state.sh` - Reconcile state

Safety hooks:
- `sanitize-issue.sh` - Prompt-injection guardrails (#271)
- `diff-size-guard.sh` - Diff size guardrail (#280)
- `anti-flake.sh` - Anti-flake test retry (#282)
- `classify-issue.sh` - Protected label guards (#258)

Utilities:
- `check.sh` - Pre-commit verification umbrella (#269)
- `gh-retry.sh` - GitHub API retry with backoff (#257)
- `item-record.sh` - Per-issue durable records (#259)
- `extract-criteria.sh` - Acceptance criteria extraction (#278)

## Local State

`.autoship/` is runtime state and must not be committed.

## Verification

Before claiming work is complete, run:

```bash
bash hooks/opencode/check.sh
bash -n hooks/opencode/*.sh hooks/*.sh
```

## Docs

Keep README, `docs/`, and the GitHub Wiki aligned with OpenCode-only messaging.
