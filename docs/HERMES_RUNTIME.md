# HERMES_RUNTIME.md
# Hermes Runtime Developer / Operator Guide
# AutoShip Issue #325

---

## 1. Prerequisites and Setup

### 1.1 Hermes CLI Installation

Hermes must be installed globally to be detected by the runtime:

```bash
npm install -g hermes-agent
```

Verify installation:

```bash
hermes --version
```

### 1.2 Initial Runtime Configuration

Run the Hermes setup hook to discover capabilities and generate routing config:

```bash
bash hooks/hermes/setup.sh
```

This creates `.autoship/hermes-model-routing.json` with:
- `available` — whether `hermes` CLI is on `$PATH`
- `active_session` — whether running inside a Hermes session (detected via `HERMES_SESSION_ID`, `HERMES_CWD`, or `HERMES_PROVIDER`)
- `max_concurrent` — hard cap of 3 (Hermes subagent limit)
- `dispatch_method` — `cronjob`

### 1.3 GitHub CLI (`gh`) Authentication

All Hermes hooks use `gh` for GitHub API calls. Ensure you are authenticated:

```bash
gh auth status
gh auth login  # if needed
```

### 1.4 Target Repository

The default target repo is `Maleick/TextQuest`. Override via `HERMES_TARGET_REPO` or `HERMES_TARGET_REPO_PATH` (see Section 2).

---

## 2. Environment Variable Reference

| Variable | Default | Purpose |
|----------|---------|---------|
| `HERMES_TARGET_REPO` | `Maleick/TextQuest` | GitHub repo slug for issue/PR operations |
| `HERMES_TARGET_REPO_PATH` | `$HOME/Projects/TextQuest` | Local filesystem path to the target repo (used for worktree discovery) |
| `HERMES_SESSION_ID` | *(unset)* | Set when running inside a Hermes session; triggers `delegate_task` dispatch instead of `hermes chat` CLI |
| `HERMES_CWD` | *(unset)* | Hermes session working directory; part of active-session detection |
| `HERMES_PROVIDER` | *(unset)* | Active Hermes provider name; part of active-session detection |
| `HERMES_LABELS` | `autoship:ready-simple` | Comma-separated label filter for `plan-issues.sh` |

### 2.1 Active Session Detection

`setup.sh` and `status.sh` consider a session active if **any** of these are non-empty:
- `HERMES_SESSION_ID`
- `HERMES_CWD`
- `HERMES_PROVIDER`

When active, `dispatch.sh` immediately invokes `runner.sh` via `delegate_task` rather than queuing for external cron dispatch.

---

## 3. Hook Dependency Chain Diagram

```
plan-issues.sh
    |
    v
dispatch.sh  <-->  model-router.sh  (model selection)
    |
    +--->  HERMES_PROMPT.md  (written to workspace)
    |
    +--->  worktree-path.txt  (written to workspace)
    |
    v
runner.sh  (single-issue or batch mode)
    |
    +--->  Inside Hermes session?  -->  delegate_task marker (DELEGATED)
    |
    +--->  hermes CLI available?   -->  `timeout 600 hermes chat --prompt ...`
    |
    v
status transitions:  QUEUED -> RUNNING -> COMPLETE / BLOCKED / STUCK
    |
    +--->  COMPLETE  -->  create-pr.sh  -->  post-merge-cleanup.sh
    +--->  BLOCKED   -->  (manual intervention required)
    +--->  STUCK     -->  (timeout or retry)
    |
    v
cleanup-worktrees.sh  (batch cleanup of terminal states)
    |
    v
auto-prune.sh  (threshold-based pruning)
```

### 3.1 Cron-Dispatch Path

```
cronjob-dispatch.sh
    |
    +--->  Reads HERMES_PROMPT.md from workspace
    +--->  Generates cronjob spec for `hermes cronjob create`
    +--->  Worktree path: workspace_dir/worktree-path.txt or fallback to workspace_dir
```

---

## 4. Status Lifecycle

### 4.1 States

| State | Meaning | Writable By |
|-------|---------|-------------|
| `QUEUED` | Issue planned, prompt written, awaiting runner slot | `dispatch.sh` |
| `RUNNING` | Worker actively executing | `runner.sh` |
| `COMPLETE` | Work finished, PR created, ready for merge | Hermes agent / `runner.sh` |
| `BLOCKED` | Cannot proceed (missing CLI, missing worktree, etc.) | `dispatch.sh`, `runner.sh` |
| `STUCK` | Timeout exceeded (10 min) or agent hung | `runner.sh` |
| `DELEGATED` | Marker for parent-agent `delegate_task` handoff | `runner.sh` (inside session) |

### 4.2 Transition Rules

```
QUEUED  --runner.sh-->  RUNNING
RUNNING --success-->     COMPLETE  -->  create-pr.sh  -->  close-issue.sh
RUNNING --failure-->     BLOCKED   -->  (manual fix + re-queue)
RUNNING --timeout-->     STUCK     -->  (retry or escalate)
```

- `QUEUED` is set by `dispatch.sh` after writing `HERMES_PROMPT.md`.
- `RUNNING` is set by `runner.sh` at the start of execution.
- Terminal states (`COMPLETE`, `BLOCKED`, `STUCK`) are written by the agent or `runner.sh` on exit.
- `cleanup-worktrees.sh` removes workspaces whose status is `COMPLETE`, `BLOCKED`, `STUCK`, or `unknown`.
- `auto-prune.sh` skips any workspace with `RUNNING` status.

---

## 5. Cronjob Setup Instructions

### 5.1 Manual Cronjob Creation

For a single queued issue:

```bash
bash hooks/hermes/cronjob-dispatch.sh issue-325
```

This outputs a spec. Create the actual cronjob:

```bash
hermes cronjob create \
  --name "autoship-issue-325" \
  --schedule "every 10m" \
  --workdir "/path/to/worktree" \
  --prompt-file "/path/to/.autoship/workspaces/issue-325/HERMES_PROMPT.md"
```

### 5.2 Batch Dispatch via Runner

The preferred batch mode:

```bash
bash hooks/hermes/runner.sh
```

This:
1. Counts `RUNNING` workspaces.
2. Calculates available slots (`max_concurrent - running`).
3. Dispatches up to that many `QUEUED` issues in parallel background jobs.
4. Runs `cleanup-worktrees.sh` and `auto-prune.sh` after the batch.

### 5.3 Scheduling Recommendations

| Schedule | Use Case |
|----------|----------|
| `every 10m` | High-throughput burn-down (atomic issues <= 10 min) |
| `every 30m` | Standard throughput |
| `0 */6 * * *` | Low-frequency maintenance / review mode |

> **Note:** The 10-minute timeout in `runner.sh` (`timeout 600`) is aligned with the `every 10m` schedule. If an issue exceeds this, it is marked `STUCK` and the next cron tick can retry or escalate.

---

## 6. Troubleshooting Common Issues

### 6.1 "Hermes CLI not found" (BLOCKED)

**Symptom:** `dispatch.sh` writes `BLOCKED` with reason `hermes CLI not found`.

**Fix:**
```bash
npm install -g hermes-agent
bash hooks/hermes/setup.sh
```

### 6.2 "Worktree not found" (BLOCKED)

**Symptom:** `runner.sh` cannot locate the git worktree for an issue.

**Causes & Fixes:**
- `HERMES_TARGET_REPO_PATH` is wrong or unset. Verify the path exists:
  ```bash
  ls "$HERMES_TARGET_REPO_PATH"
  ```
- Worktree was removed by `cleanup-worktrees.sh` or `auto-prune.sh`. Re-run `dispatch.sh` to recreate.
- The `create-worktree.sh` shared hook failed. Check git worktree list:
  ```bash
  git worktree list
  ```

### 6.3 Max Concurrent Reached

**Symptom:** `dispatch.sh` prints `CAP_REACHED: N active / 3 max`.

**Fix:** Wait for running jobs to finish, or increase `max_concurrent_children` in `~/.hermes/config.yaml` (not recommended above 3 for Hermes subagent stability).

### 6.4 Timeout / STUCK Issues

**Symptom:** Status `STUCK` after 10 minutes.

**Investigate:**
```bash
cat .autoship/workspaces/issue-*/status
cat .autoship/workspaces/issue-*/HERMES_RESULT.md  # if agent wrote it
```

**Common causes:**
- Issue too large for atomic work (> 10 min). Re-slice into smaller issues.
- Agent waiting on interactive input. Ensure `HERMES_PROMPT.md` has no interactive prompts.
- Network or model provider lag. Check `hermes status` or provider dashboard.

### 6.5 Dirty Worktree Skipped During Cleanup

**Symptom:** `cleanup-worktrees.sh` reports `Skipping worktree: issue-N (dirty)`.

**Fix:** The agent left uncommitted changes. Manually inspect:
```bash
cd /path/to/worktree
git status
```
Either commit, stash, or reset, then re-run cleanup.

### 6.6 Auto-Prune Thresholds Exceeded

**Symptom:** `auto-prune.sh` returns non-zero and logs warnings.

**Environment overrides:**
```bash
export AUTOSHIP_MAX_WORKTREE_SIZE_GB=2        # per-worktree limit
export AUTOSHIP_MAX_TOTAL_WORKTREES_GB=10     # total limit
export AUTOSHIP_MAX_WORKSPACE_COUNT=20        # max workspace dirs
export AUTOSHIP_MAX_WORKSPACE_AGE_DAYS=7      # auto-remove after N days
```

### 6.7 Model Selection Empty

**Symptom:** `dispatch.sh` exits with `Error: model selection returned empty`.

**Fix:**
- Ensure `config/model-routing.json` exists and has valid tiers.
- Install `jq` (required by `model-router.sh`):
  ```bash
  brew install jq  # macOS
  ```
- Fallback model is `kimi-k2.6` if routing fails.

### 6.8 State File Corruption

**Symptom:** `jq` errors reading `.autoship/state.json`.

**Fix:**
```bash
bash hooks/update-state.sh reset
bash hooks/hermes/setup.sh
```

---

## 7. File Reference

| File | Purpose |
|------|---------|
| `hooks/hermes/setup.sh` | Runtime discovery & routing config generation |
| `hooks/hermes/plan-issues.sh` | Fetch and filter issues by label |
| `hooks/hermes/dispatch.sh` | Create worktree, write prompt, queue issue |
| `hooks/hermes/runner.sh` | Execute worker (delegate_task or hermes chat) |
| `hooks/hermes/status.sh` | Display runtime status summary |
| `hooks/hermes/cronjob-dispatch.sh` | Generate cronjob specs |
| `hooks/hermes/model-router.sh` | Tier-based model selection with round-robin |
| `hooks/hermes/close-issue.sh` | Close GitHub issue with completion comment |
| `hooks/hermes/cleanup-worktrees.sh` | Remove completed/abandoned worktrees |
| `hooks/hermes/auto-prune.sh` | Threshold-based disk/workspace pruning |
| `hooks/hermes/post-merge-cleanup.sh` | Full cleanup after PR merge |
| `.autoship/hermes-model-routing.json` | Hermes runtime config |
| `.autoship/workspaces/issue-*/status` | Per-issue state file |
| `.autoship/workspaces/issue-*/HERMES_PROMPT.md` | Agent instruction prompt |
| `.autoship/workspaces/issue-*/HERMES_RESULT.md` | Agent output report (written by agent) |

---

## 8. Quick Start Checklist

- [ ] `hermes` CLI installed and on `$PATH`
- [ ] `gh` CLI authenticated
- [ ] `HERMES_TARGET_REPO` and `HERMES_TARGET_REPO_PATH` set correctly
- [ ] `bash hooks/hermes/setup.sh` run once
- [ ] Issues labeled with `autoship:ready-simple` (or `HERMES_LABELS` override)
- [ ] `config/model-routing.json` present (for tiered model routing)
- [ ] `jq` installed
- [ ] Cronjob or manual `runner.sh` scheduled

---

*Generated from hooks/hermes/*.sh source. Last updated: 2026-05-04*
