# AutoShip OpenCode Installation Guide

AutoShip v1.5.0+ uses OpenCode as the only supported runtime. The installer checks GitHub releases and refreshes the local plugin copy when a newer release is available.

## Requirements

- [OpenCode](https://opencode.ai) installed
- `jq` — `brew install jq`
- `gh` — `brew install gh && gh auth login`
- Git repo with GitHub remote
- Open GitHub issues to work on

## Installation

### Option 1: Global Installation (Recommended)

```bash
bash hooks/opencode/install.sh
```

### Option 2: Project-Local Installation

```bash
# Use the repo-local installer from inside the checkout
bash hooks/opencode/install.sh
```

## Skills Overview

| Skill | Description |
|-------|-------------|
| `autoship-orchestrate` | Core orchestration — issue classification, dispatch, event handling |
| `autoship-dispatch` | Agent dispatch — worktree creation, prompt generation, tool routing |
| `autoship-verify` | Verification pipeline — review, simplify, PR creation, CI monitoring |
| `autoship-status` | Status dashboard — quota bars, progress, token usage |
| `autoship-poll` | GitHub issue sync — 10-minute polling safety net |
| `autoship-setup` | First-run configuration wizard |
| `autoship-discord-webhook` | Discord webhook integration |
| `autoship-discord-commands` | Discord command channel |

## Commands

| Command | Description |
|---------|-------------|
| `/autoship` | Start orchestration |
| `/autoship-status` | Show current status |
| `/autoship-plan` | Dry-run (show plan) |
| `/autoship-setup` | First-run wizard |
| `/autoship-stop` | Stop orchestration |

## Quick Start

```bash
# 1. Install AutoShip skills
bash hooks/opencode/install.sh

# 2. Navigate to your project
cd ~/Projects/my-project

# 3. Start AutoShip
/autoship
```

## How It Works on OpenCode

```
GitHub Issue → orchestrate skill
    ↓
classify-issue.sh → task type
    ↓
dispatch skill → Agent subagent
    ↓
Agent writes status file (.autoship/workspaces/<key>/status)
    ↓
Monitor detects COMPLETE/BLOCKED/STUCK
    ↓
verify skill → reviewer agent
    ↓
PR creation → GitHub API
    ↓
Monitor CI → merge
```

## Key Differences from the Old Runtime

| Aspect | Old runtime | OpenCode |
|--------|-------------|----------|
| Agent execution | tmux panes | Agent subagents |
| Status detection | `pane.log` | `status` file |
| Monitor interval | 5s/30s/60s | 10s polling |
| Process management | tmux commands | Agent task state |

## State Files

AutoShip maintains state in `.autoship/`:

- `state.json` — Issue lifecycle, plan phases
- `quota.json` — OpenCode provider availability
- `token-ledger.json` — Token usage tracking
- `event-queue.json` — Pending events
- `config.json` — Project configuration
- `routing.json` — Task type routing metadata
- `model-routing.json` — Live OpenCode model selections

## Routing Matrix

Configure available models with setup:

```bash
bash hooks/opencode/setup.sh

# Or choose exact models from the current opencode models output:
AUTOSHIP_MODELS="provider/model-a,provider/model-b" bash hooks/opencode/setup.sh

# Regenerate free defaults from the current opencode models output:
AUTOSHIP_REFRESH_MODELS=1 bash hooks/opencode/setup.sh
```

`setup.sh` writes `.autoship/model-routing.json`. This file is intentionally user-editable and is preserved on later setup runs unless `AUTOSHIP_REFRESH_MODELS=1` or `AUTOSHIP_MODELS=...` is provided.

By default, `openai/gpt-5.5` is used for planner/coordinator/orchestrator/reviewer roles. Worker models are selected per task by the model selector using task compatibility, configured strength, cost class, and previous success/failure history. Go-provider and Spark models can be selected manually and can win when they are the best configured fit. `openai/gpt-5.5-fast` is rejected.

## Troubleshooting

### Skills not loading

```bash
# Check OpenCode plugin directory
ls ~/.config/opencode/plugins/

# Verify plugin file exists
ls ~/.config/opencode/plugins/autoship.ts
```

### State file issues

```bash
# Reset state
rm .autoship/state.json
./hooks/opencode/init.sh
```

### Quota tracking

```bash
# Check quotas
cat .autoship/quota.json

# Reset quotas
bash hooks/quota-update.sh reset
```

## Uninstall

```bash
# Remove plugin files
rm -rf ~/.config/opencode/plugins/autoship.ts

# Remove state (optional)
rm -rf .autoship/
```
