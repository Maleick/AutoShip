# HERMES Runtime Environment Variables

This document catalogs every `HERMES_*` environment variable referenced by the AutoShip Hermes runtime hooks in `hooks/hermes/*.sh`.  Each entry includes the variable name, the script(s) that consume it, the default value (if any), and a description of its purpose.

---

## HERMES_ACTIVE

| | |
|---|---|
| **Scripts** | `setup.sh` |
| **Default** | `false` |
| **Description** | Set to `true` when any of `HERMES_SESSION_ID`, `HERMES_CWD`, or `HERMES_PROVIDER` is present in the environment.  Indicates that the current shell is running inside an active Hermes session (messaging gateway, cron job, etc.).  Used to populate the `active_session` field in `hermes-model-routing.json`. |

---

## HERMES_AVAILABLE

| | |
|---|---|
| **Scripts** | `setup.sh`, `dispatch.sh` |
| **Default** | `false` |
| **Description** | Set to `true` when the `hermes` CLI binary is found on `$PATH`.  `setup.sh` writes this into `hermes-model-routing.json`; `dispatch.sh` blocks dispatch with a `BLOCKED` status when the CLI is missing. |

---

## HERMES_CWD

| | |
|---|---|
| **Scripts** | `setup.sh`, `status.sh` |
| **Default** | *(none — checked for presence only)* |
| **Description** | One of the three sentinel variables used to detect an active Hermes session.  If set (along with `HERMES_SESSION_ID` or `HERMES_PROVIDER`), the runtime considers itself inside a Hermes-managed environment.  Never read for its value — only tested with `[[ -n ... ]]`. |

---

## HERMES_LABELS

| | |
|---|---|
| **Scripts** | `plan-issues.sh` |
| **Default** | `autoship:ready-simple` |
| **Description** | Comma-separated list of GitHub issue labels to filter on when fetching the Hermes work queue.  Passed directly to the GitHub REST API `labels` query parameter. |

---

## HERMES_PROVIDER

| | |
|---|---|
| **Scripts** | `setup.sh`, `status.sh` |
| **Default** | *(none — checked for presence only)* |
| **Description** | Same sentinel semantics as `HERMES_CWD`.  Presence implies an active Hermes session.  Not used for provider selection logic inside the hooks. |

---

## HERMES_PROMPT.md

| | |
|---|---|
| **Scripts** | `dispatch.sh`, `cronjob-dispatch.sh`, `runner.sh`, `status.sh` |
| **Default** | *(file path, not an env var)* |
| **Description** | Not an environment variable, but a file path convention.  `dispatch.sh` writes the per-issue prompt to `$WORKSPACE_PATH/HERMES_PROMPT.md`.  `runner.sh` and `cronjob-dispatch.sh` read it back to execute the task.  `status.sh` checks for its presence to confirm a workspace is a Hermes dispatch. |

---

## HERMES_RESULT.md

| | |
|---|---|
| **Scripts** | `dispatch.sh` (referenced in generated prompt) |
| **Default** | *(file path, not an env var)* |
| **Description** | Referenced inside the generated `HERMES_PROMPT.md` as the file the Hermes agent must write after finishing work.  Expected contents: `status` (`COMPLETE`/`BLOCKED`/`STUCK`), files changed, and validation results. |

---

## HERMES_SESSION_ID

| | |
|---|---|
| **Scripts** | `setup.sh`, `dispatch.sh`, `runner.sh`, `status.sh` |
| **Default** | *(none — checked for presence only)* |
| **Description** | Primary sentinel for detecting an active Hermes session.  When present, `dispatch.sh` immediately triggers `runner.sh` via `delegate_task` instead of queuing for later cron execution.  `runner.sh` uses it to choose the `delegate_task` code path (parallel sub-agent dispatch) versus the `hermes chat` CLI path. |

---

## HERMES_TARGET_REPO

| | |
|---|---|
| **Scripts** | `plan-issues.sh`, `dispatch.sh`, `close-issue.sh`, `post-merge-cleanup.sh` |
| **Default** | `Maleick/TextQuest` |
| **Description** | GitHub repository slug (`owner/repo`) that the Hermes runtime targets for issue fetch, PR creation, issue closure, and label updates.  All `gh` CLI calls that need a `--repo` argument fall back to this value. |

---

## HERMES_TARGET_REPO_PATH

| | |
|---|---|
| **Scripts** | `runner.sh`, `cleanup-worktrees.sh`, `auto-prune.sh`, `post-merge-cleanup.sh` |
| **Default** | `$HOME/Projects/TextQuest` |
| **Description** | Absolute path to the *local* clone of `HERMES_TARGET_REPO`.  Used to resolve git worktree locations, run `git worktree list`, and perform post-merge cleanup.  `runner.sh` searches this path first when looking for the worktree belonging to a dispatched issue. |

---

## Summary Table

| Variable | Scripts | Default | Type |
|---|---|---|---|
| `HERMES_ACTIVE` | `setup.sh` | `false` | runtime flag |
| `HERMES_AVAILABLE` | `setup.sh`, `dispatch.sh` | `false` | runtime flag |
| `HERMES_CWD` | `setup.sh`, `status.sh` | — | sentinel |
| `HERMES_LABELS` | `plan-issues.sh` | `autoship:ready-simple` | config |
| `HERMES_PROVIDER` | `setup.sh`, `status.sh` | — | sentinel |
| `HERMES_SESSION_ID` | `setup.sh`, `dispatch.sh`, `runner.sh`, `status.sh` | — | sentinel |
| `HERMES_TARGET_REPO` | `plan-issues.sh`, `dispatch.sh`, `close-issue.sh`, `post-merge-cleanup.sh` | `Maleick/TextQuest` | config |
| `HERMES_TARGET_REPO_PATH` | `runner.sh`, `cleanup-worktrees.sh`, `auto-prune.sh`, `post-merge-cleanup.sh` | `$HOME/Projects/TextQuest` | config |

---

## Notes

- **Sentinel variables** (`HERMES_SESSION_ID`, `HERMES_CWD`, `HERMES_PROVIDER`) are never read for their values; the hooks only test whether they are *set* (`[[ -n "${VAR:-}" ]]`).  This is the canonical way the Hermes runtime signals to child shells that they are executing inside a managed session.
- **Config variables** (`HERMES_TARGET_REPO`, `HERMES_TARGET_REPO_PATH`, `HERMES_LABELS`) use Bash parameter expansion (`${VAR:-default}`) so they can be overridden per-invocation without editing hook source.
- **Max concurrent** is *not* controlled by an environment variable; it is read from `~/.hermes/config.yaml` (`max_concurrent_children`) by `dispatch.sh` and `runner.sh`.
- The generated `HERMES_PROMPT.md` instructs the Hermes agent to use `git push`, `gh pr create`, and `gh issue close` commands; these instructions are literal text in the prompt, not environment-driven.

---

## Quick Reference

For a compact table of **all** Hermes and AutoShip environment variables (including pruning thresholds and directory paths), see:

**`hooks/hermes/ENVIRONMENT_VARIABLES.md`**
