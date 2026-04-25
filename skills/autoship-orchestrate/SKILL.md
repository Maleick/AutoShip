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
AUTOSHIP_HOME="${OPENCODE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/.autoship"
gh auth status
git rev-parse --show-toplevel
command -v opencode >/dev/null 2>&1
bash "$AUTOSHIP_HOME/hooks/opencode/init.sh"
bash "$AUTOSHIP_HOME/hooks/opencode/setup.sh"
```

## Planning

Always plan eligible issues in ascending issue-number order:

```bash
AUTOSHIP_HOME="${OPENCODE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/.autoship"
PLAN=$(bash "$AUTOSHIP_HOME/hooks/opencode/plan-issues.sh" --limit 10)
```

The plan must exclude running, blocked, and human-required issues.

## Dispatch

Dispatch each planned issue through the OpenCode hooks:

```bash
AUTOSHIP_HOME="${OPENCODE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/.autoship"
TASK_TYPE=$(bash "$AUTOSHIP_HOME/hooks/opencode/classify-issue.sh" <issue-number>)
bash "$AUTOSHIP_HOME/hooks/opencode/dispatch.sh" <issue-number> "$TASK_TYPE"
bash "$AUTOSHIP_HOME/hooks/opencode/runner.sh"
```

Default active worker cap is 15. The runner enforces the cap from `.autoship/state.json` or routing config.

## Monitoring

Workers write one of these to `.autoship/workspaces/<issue-key>/status`:

- `QUEUED`
- `RUNNING`
- `COMPLETE`
- `BLOCKED`
- `STUCK`

Reconcile state from workspace statuses:

```bash
AUTOSHIP_HOME="${OPENCODE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/.autoship"
bash "$AUTOSHIP_HOME/hooks/opencode/reconcile-state.sh"
bash "$AUTOSHIP_HOME/hooks/opencode/status.sh"
```

## Completion

On `COMPLETE`, run verification, ensure committed changes exist, and create PRs with conventional titles:

```bash
AUTOSHIP_HOME="${OPENCODE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/.autoship"
bash "$AUTOSHIP_HOME/hooks/opencode/pr-title.sh" --issue <number>
```

On repeated failures or unclear requirements, mark the issue blocked for human review.
