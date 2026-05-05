# Hermes Runtime Environment Variables

> Reference for all environment variables used by the Hermes runtime hooks in `hooks/hermes/`.
> Set these in your shell, `.env` file, or CI environment to configure Hermes behavior.

---

## Core Hermes Variables

| Variable | Default | Used In | Purpose |
|----------|---------|---------|---------|
| `HERMES_TARGET_REPO` | `Maleick/TextQuest` | `close-issue.sh`, `dispatch.sh`, `plan-issues.sh`, `post-merge-cleanup.sh` | Target GitHub repo for issue operations |
| `HERMES_TARGET_REPO_PATH` | `$HOME/Projects/TextQuest` | `auto-prune.sh`, `cleanup-worktrees.sh`, `post-merge-cleanup.sh`, `runner.sh` | Local filesystem path to the target repo |
| `HERMES_SESSION_ID` | *(none)* | `dispatch.sh`, `runner.sh`, `setup.sh`, `status.sh` | Hermes session detection — set automatically when running inside a Hermes agent session |
| `HERMES_CWD` | *(none)* | `setup.sh`, `status.sh` | Hermes current working directory — set automatically by the Hermes runtime |
| `HERMES_PROVIDER` | *(none)* | `setup.sh` | Hermes provider name (e.g. `nous`, `openrouter`, `kimi-coding`) — set automatically by the Hermes runtime |
| `HERMES_LABELS` | `autoship:ready-simple` | `plan-issues.sh` | Comma-separated GitHub issue labels to filter when planning issues for Hermes dispatch |

### Example

```bash
export HERMES_TARGET_REPO="Maleick/TextQuest"
export HERMES_TARGET_REPO_PATH="$HOME/Projects/TextQuest"
export HERMES_LABELS="autoship:ready-simple,autoship:hermes"
```

---

## AutoShip Directory & Pruning Variables

| Variable | Default | Used In | Purpose |
|----------|---------|---------|---------|
| `AUTOSHIP_DIR` | `/Users/maleick/Projects/AutoShip/.autoship` | `auto-prune.sh`, `cleanup-worktrees.sh`, `cronjob-dispatch.sh`, `dispatch.sh`, `plan-issues.sh`, `runner.sh`, `setup.sh`, `status.sh` | Root path for AutoShip runtime state (workspaces, logs, plans) |
| `AUTOSHIP_MAX_WORKTREE_SIZE_GB` | `2` | `auto-prune.sh` | Maximum size (GB) for a single worktree before auto-prune removes it |
| `AUTOSHIP_MAX_TOTAL_WORKTREES_GB` | `10` | `auto-prune.sh` | Maximum total size (GB) of all worktrees combined before oldest are pruned |
| `AUTOSHIP_MAX_WORKSPACE_COUNT` | `20` | `auto-prune.sh` | Maximum number of `.autoship/workspaces/issue-*` directories allowed |
| `AUTOSHIP_MAX_WORKSPACE_AGE_DAYS` | `7` | `auto-prune.sh` | Auto-remove workspaces older than this many days |

### Example

```bash
export AUTOSHIP_DIR="/Users/maleick/Projects/AutoShip/.autoship"
export AUTOSHIP_MAX_WORKTREE_SIZE_GB="3"
export AUTOSHIP_MAX_TOTAL_WORKTREES_GB="15"
export AUTOSHIP_MAX_WORKSPACE_COUNT="30"
export AUTOSHIP_MAX_WORKSPACE_AGE_DAYS="3"
```

---

## AutoShip Root (Model Routing)

| Variable | Default | Used In | Purpose |
|----------|---------|---------|---------|
| `AUTOSHIP_ROOT` | *(inferred from script)* | `model-router.sh` | Path to AutoShip repo root for locating `config/model-routing.json` and usage logs |

> `AUTOSHIP_ROOT` is computed automatically in `model-router.sh` via `git rev-parse --show-toplevel`. It is not typically set manually.

---

## How Variables Are Detected

### `setup.sh`

- `HERMES_AVAILABLE`: `true` if `hermes` CLI is on `$PATH`
- `HERMES_ACTIVE`: `true` if any of `HERMES_SESSION_ID`, `HERMES_CWD`, or `HERMES_PROVIDER` are set

### `status.sh`

- `HERMES_CLI`: `true` if `hermes` CLI is on `$PATH`
- `HERMES_SESSION`: `true` if `HERMES_SESSION_ID` or `HERMES_CWD` are set

### `runner.sh`

- `HERMES_SESSION_ID`: triggers `delegate_task` dispatch instead of `hermes chat` CLI

---

## File Location

Place this reference next to the hooks for discoverability:

```
hooks/hermes/
  setup.sh
  dispatch.sh
  runner.sh
  plan-issues.sh
  status.sh
  close-issue.sh
  post-merge-cleanup.sh
  cleanup-worktrees.sh
  auto-prune.sh
  cronjob-dispatch.sh
  model-router.sh
  ENVIRONMENT_VARIABLES.md   <-- this file
```

---

## See Also

- `hooks/opencode/` — OpenCode runtime hooks (separate variable set)
- `AGENTS.md` — AutoShip agent guide and runtime policy
- `config/model-routing.json` — Model tier configuration consumed by `model-router.sh`
