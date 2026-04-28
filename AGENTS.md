# AutoShip Agent Guide

AutoShip is an OpenCode-only GitHub issue → pull request orchestration plugin.

See [AGENT_CATALOG.md](AGENT_CATALOG.md) for the specialized agent roles, inputs, outputs, boundaries, and default model families.

## Runtime Policy

- OpenCode is the only supported worker runtime.
- Role models are selected from live `opencode models` inventory and `.autoship/model-routing.json`; do not assume `openai/gpt-5.5` is available or preferred.
- Prefer capable free models first, then OpenCode Go role models when available; use Kimi/Kimmy 2.6 only through `opencode-go/*` unless the operator explicitly selects a paid Zen/OpenRouter model.
- `openai/gpt-5.5-fast` is not allowed.
- Worker models come from live `opencode models` inventory and are routed free-first with deterministic rotation across compatible workers.
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
- `monitor-ci.sh` - Monitor PR CI checks
- `auto-merge.sh` - Merge opted-in PRs after CI passes
- `reconcile-state.sh` - Reconcile state

Safety hooks:
- `sanitize-issue.sh` - Prompt-injection guardrails (#271)
- `diff-size-guard.sh` - Diff size guardrail (#280)
- `anti-flake.sh` - Anti-flake test retry (#282)
- `classify-issue.sh` - Protected label guards (#258)
- `worktree-checksum.sh` - Worktree checksum/scope reports (#260)
- `quota-guard.sh` - Free-model quota pause/resume guardrail (#284)

Utilities:
- `check.sh` - Pre-commit verification umbrella (#269)
- `gh-retry.sh` - GitHub API retry with backoff (#257)
- `item-record.sh` - Per-issue durable records (#259)
- `extract-criteria.sh` - Acceptance criteria extraction (#278)
- `audit.sh` - GitHub/local drift audit (#256)
- `dashboard.sh` - Metrics dashboard (#266)
- `pr-body.sh` - Structured PR descriptions (#279)
- `policy-hash.sh` - Plan invalidation policy hash (#264)

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
