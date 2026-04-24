---
name: autoship-orchestrate
description: AutoShip orchestration for OpenCode-only workers
platform: opencode
tools: ["Bash", "Agent", "Read", "Write", "Edit", "Glob", "Grep", "Skill", "TaskCreate", "TaskUpdate"]
---

# AutoShip Orchestration Protocol

AutoShip uses OpenCode as its only worker runtime. Do not dispatch external CLIs or non-OpenCode runtimes.

## Startup

```bash
gh auth status
git rev-parse --show-toplevel
command -v opencode >/dev/null 2>&1
bash hooks/opencode/init.sh
bash hooks/opencode/setup.sh
```

## Planning

Always plan eligible issues in ascending issue-number order:

```bash
PLAN=$(bash hooks/opencode/plan-issues.sh --limit 10)
```

The plan must exclude running, blocked, human-required, and unsafe/evasion-prone issues.

## Dispatch

Dispatch each planned issue through the OpenCode hooks:

```bash
TASK_TYPE=$(bash hooks/opencode/classify-issue.sh <issue-number>)
bash hooks/opencode/dispatch.sh <issue-number> "$TASK_TYPE"
bash hooks/opencode/runner.sh
```

Default active worker cap is 10. The runner enforces the cap from `.autoship/state.json` or routing config.

## Monitoring

Workers write one of these to `.autoship/workspaces/<issue-key>/status`:

- `QUEUED`
- `RUNNING`
- `COMPLETE`
- `BLOCKED`
- `STUCK`

Reconcile state from workspace statuses:

```bash
bash hooks/opencode/reconcile-state.sh
bash hooks/opencode/status.sh
```

## Completion

On `COMPLETE`, run verification, ensure committed changes exist, and create PRs with conventional titles:

```bash
bash hooks/opencode/pr-title.sh --issue <number>
```

On repeated failures, unsafe scope, or unclear requirements, mark the issue blocked for human review.
