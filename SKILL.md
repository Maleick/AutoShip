---
name: autoship
description: "Delegate GitHub issue-to-PR orchestration to the AutoShip OpenCode plugin."
version: 1.0.0
author: Kira Vanguard
license: MIT
metadata:
  hermes:
    tags: [github, opencode, autoship, automation, pr]
    homepage: https://github.com/Maleick/AutoShip
    related_skills: [textquest-development, github-issues, github-pr-workflow]
---

# AutoShip

Lightweight Hermes skill wrapper for the [AutoShip](https://github.com/Maleick/AutoShip) OpenCode plugin.

## When to Use

- You need automated GitHub issue triage, planning, dispatch, and PR creation.
- You want cron-based burn-down with worker rotation and CI gating.
- The target repo uses `autoship:ready-simple` / `atomic:ready` labels.

## Runtime

- **Primary**: OpenCode plugin commands in `~/Projects/AutoShip`
- **Hermes**: Adapter hooks in `~/Projects/AutoShip/hooks/hermes/`

## Commands

```bash
# Plan issues
cd ~/Projects/AutoShip && bash hooks/opencode/plan-issues.sh

# Dispatch workers
cd ~/Projects/AutoShip && bash hooks/opencode/dispatch.sh

# Check status
cd ~/Projects/AutoShip && bash hooks/hermes/status.sh
```

## Hermes Integration

When called as a Hermes skill:

1. Read `~/Projects/AutoShip/AGENTS.md` for role model routing policy
2. Use `hooks/hermes/` adapter scripts for Hermes-specific dispatch
3. Respect free-first model routing from `config/model-routing.json`
4. Report PR URLs and CI status

### Hermes Hook Architecture

AutoShip provides Hermes-specific hooks in `hooks/hermes/`:

| Hook | Purpose | Entry Point |
|------|---------|-------------|
| `setup.sh` | Detect Hermes capabilities, write `hermes-model-routing.json` | `bash hooks/hermes/setup.sh` |
| `plan-issues.sh` | Fetch and filter issues for Hermes dispatch | `bash hooks/hermes/plan-issues.sh` |
| `dispatch.sh` | Create worktree, write prompt, queue issue, **auto-select model tier** | `bash hooks/hermes/dispatch.sh <issue_num> [task_type] [model]` |
| `runner.sh` | Execute queued workspace via `hermes chat` or delegate_task, **auto-cleanup after batch** | `bash hooks/hermes/runner.sh [issue_key]` |
| `cleanup-worktrees.sh` | Remove completed/abandoned worktrees (3-phase: workspaces → git worktrees → prune) | `bash hooks/hermes/cleanup-worktrees.sh [--dry-run] [--verbose]` |
| `model-router.sh` | Read `config/model-routing.json` and return next model in rotation | `bash hooks/hermes/model-router.sh dispatch_with_routing [code] [simple\|complex]` |
| `status.sh` | Show Hermes runtime state | `bash hooks/hermes/status.sh` |

**Critical**: `dispatch.sh` and `runner.sh` were historically empty stubs that only printed instructions. As of commit `90df8b9`, they properly:
- Read `max_concurrent_children` from `~/.hermes/config.yaml` (not hardcoded 3)
- Execute via `hermes chat` with 10-minute timeout for atomic work
- Support delegate_task mode when `HERMES_SESSION_ID` is set
- Auto-create PRs on COMPLETE status
- Handle timeouts (exit 124 = STUCK, other errors = BLOCKED)

**User Preference: AutoShip over Cron Burn-Down**
The user explicitly deleted the `textquest-issue-burn-down` cron and prefers AutoShip dispatch exclusively. Do not recreate cron-based burn-down. Use:
```bash
# Batch dispatch 10 issues
cd ~/Projects/AutoShip
for issue in $(gh issue list --label atomic:ready --limit 10 --json number --jq '[.[].number] | join(" ")'); do
  bash hooks/hermes/dispatch.sh "$issue"
done
bash hooks/hermes/runner.sh  # executes up to max_concurrent slots
```

**Phase-End Protocol**
At the end of each work phase, the user expects:
1. `git pull origin main` (or master) to sync latest
2. `git worktree prune` + remove stale worktrees for inactive issues
3. `git branch -D` merged local branches
4. `git push origin --delete` merged remote branches
5. `gh pr merge` any open PRs
6. Track improvements in commit log

This is now standard procedure for all repos under `~/Projects/`.

**Model Routing** (added commit `dc9e1f6`, rebuilt 2026-05-04):
- `dispatch.sh` calls `model-router.sh` to select tier based on `config/model-routing.json`
- **Intelligent routing**: Python router analyzes issue title + labels to determine complexity, domain, and task type
- Complex/parity tasks → go_paid (DeepSeek V4 Pro)
- Simple audits → zen_free fast (GPT 5 Nano)
- Medium implementations → zen_free balanced (Nemotron 3 Super)
- Free Zen models first → Go DeepSeek → Hermes fallback
- Logs model selection to `.autoship/logs/model-selection.log`
- Round-robin rotation across models in each tier

**Critical bug fixed**: `auto-prune.sh` used `ls -1` which counts files inside workspaces, not directories. This caused it to report 30+ workspaces when only 10 existed, leading to premature pruning of QUEUED issues. Fixed with `find -type d`.

**Worktree Cleanup** (added PR #317, commit `3e88b9`):
- `runner.sh` auto-calls `cleanup-worktrees.sh` after each batch dispatch
- Three-phase cleanup: AutoShip workspaces → git worktrees → prune metadata
- Safety: never removes active (QUEUED/RUNNING) or dirty (uncommitted) worktrees
- Supports `--dry-run` and `--verbose` flags
- See `references/worktree-cleanup-integration.md` for full details

**Post-Merge Cleanup** (added PR #319, commit `7d60487`):
- `post-merge-cleanup.sh` removes worktrees/branches after PR merge
- Updates issue label: `atomic:ready` → `atomic:complete`
- Prevents orphaned worktree accumulation after burn-down completion
- Call: `bash hooks/hermes/post-merge-cleanup.sh <issue_number>`

**Environment Variables**:
- `HERMES_TARGET_REPO_PATH`: Must be set to target repo path (default: `$HOME/Projects/TextQuest`)
- `HERMES_SESSION_ID`: Auto-detected; triggers delegate_task mode instead of `hermes chat`

If you encounter old stubs, update AutoShip: `cd ~/Projects/AutoShip && git pull origin main`

**User Preference**: All AutoShip improvements must be made in `~/Projects/AutoShip` repo and submitted as PRs (or direct to main if explicitly approved). Do not bake AutoShip logic into this Hermes skill — keep skills as thin wrappers.

**Skill sync**: This skill is symlinked to `~/Projects/AutoShip` for live updates. Do not copy SKILL.md manually.

## Model Routing

AutoShip uses a three-tier free-first routing strategy:

| Tier | Priority | Provider | Models | Cost |
|------|----------|----------|--------|------|
| zen_free | 1 | opencode-zen | Big Pickle, MiniMax M2.5, Ling 2.6 Flash, Hy3 Preview, Nemotron 3 Super, GPT 5 Nano | Free |
| go_paid | 2 | opencode-go | DeepSeek V4 Pro, DeepSeek V4 Flash | Paid |
| hermes_fallback | 3 | hermes | Kimi K2.6, GPT 5.5 | Subscription |

Rules:
- Always try zen_free first for every task
- Rotate across zen_free models in round-robin to distribute quota
- If all zen_free models hit rate limit or quota, escalate to go_paid tier
- If go_paid tier also unavailable, fall back to hermes_fallback
- Never use `openai/gpt-5.5-fast`
- Track usage per model in `.autoship/usage-log.json`

The routing config lives in `config/model-routing.json` (committed), not `.autoship/` (runtime state).

## Constraints

- Worker cap: 20 for both OpenCode and Hermes
- Plan `agent:ready` issues in ascending issue-number order
- Never use `openai/gpt-5.5-fast`
- Prefer free models first, then OpenCode Go roles
- All improvements to AutoShip must be made in `~/Projects/AutoShip` repo and submitted as PRs — do not bake AutoShip logic into this Hermes skill
