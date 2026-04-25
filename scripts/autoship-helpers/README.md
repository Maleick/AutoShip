# AutoShip Helper Scripts

Operator-side tooling that complements the AutoShip plugin for high-throughput
PR generation, draining, and conflict recovery.

## Scripts

### `batch-dispatch-codex.sh <issue-N> [issue-N ...]`
Dispatch multiple GitHub issues to Codex CLI workers in parallel.
Creates worktrees, writes prompts, runs `dispatch-codex-appserver.sh` in background.
Uses model from `~/.codex/config.toml` (set to `gpt-5.4-mini` for low-quota fallback).

### `batch-dispatch-claude.sh <model> <issue-N> [issue-N ...]`
Same pattern but uses `claude -p --model <haiku|sonnet>` subprocess.
Useful when codex quota exhausted.

### `verify-and-pr.sh`
Scans `.autoship/workspaces/issue-*/` for completed workers (presence of
`AUTOSHIP_RESULT.md` + `^COMPLETE$` in pane.log), pushes branch, opens PR
with `--label autoship`. Skips if PR exists or diff empty.

### `bulk-rebase.sh <PR-number> [PR-number ...]`
Rebases multiple open AutoShip PRs onto current master with `-X ours`
strategy (PR's version wins on conflict). Force-pushes after success;
labels `autoship:blocked` on real merge failure.

**Caveats:**
- `-X ours` can silently drop master fixes that touched same lines.
  CI is the safety net.
- Looks up branch via `gh pr view --json headRefName` (PR# != issue#).

## Operator workflow

```bash
# Dispatch wave
bash scripts/autoship-helpers/batch-dispatch-claude.sh haiku 2700 2701 2702

# After workers complete (monitor via [AGENT_STATUS] events)
bash scripts/autoship-helpers/verify-and-pr.sh

# When upstream merges create cascade conflicts
gh pr list --label autoship --state open --json number,mergeable \
  | jq -r '.[] | select(.mergeable=="CONFLICTING") | .number' \
  | xargs bash scripts/autoship-helpers/bulk-rebase.sh
```

## Origin

Extracted from session 2026-04-25 nap-burn run that landed ~390 AutoShip PRs.
