# AutoShip OpenCode Installation Guide

AutoShip uses OpenCode as the only supported runtime. The package installer registers AutoShip with OpenCode and installs the bundled hooks, commands, and skills under the OpenCode config directory.

## Requirements

- [OpenCode](https://opencode.ai) installed
- `jq` — `brew install jq`
- `gh` — `brew install gh && gh auth login`
- Git repo with GitHub remote
- Open GitHub issues to work on

## Installation

### OpenCode Agent Instructions

Tell OpenCode:

```text
Fetch and follow instructions from https://raw.githubusercontent.com/Maleick/AutoShip/refs/tags/v2.2.1/INSTALL.md
```

### Option 1: Global Installation (Recommended)

```bash
npm install -g opencode-autoship
opencode-autoship install
opencode-autoship doctor
```

### Option 2: One-Time Package Installation

```bash
bunx opencode-autoship install
bunx opencode-autoship doctor
```

### Option 3: Source Checkout Installation

```bash
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
# 1. Install AutoShip for OpenCode
npm install -g opencode-autoship
opencode-autoship install
opencode-autoship doctor

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

## OpenCode Runtime

| Aspect | OpenCode behavior |
|--------|-------------------|
| Worker execution | `opencode run --model <selected-model>` |
| Status detection | Workspace `status` files |
| Monitor interval | Hook-driven polling |
| Process management | Queued workspaces and runner cap |

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

By default, setup chooses the best available role model from `opencode models`, preferring capable free models first and OpenCode Go models second. First-run setup asks which models to use for orchestrator and reviewer; they can be the same model or different models. Worker models are selected per task by the model selector using task compatibility, configured strength, cost class, previous success/failure history, and deterministic issue-number rotation across compatible workers. Free models, OpenCode Go models, and explicitly selected provider models can be configured. Paid Zen/OpenRouter Kimi models require explicit selection. `openai/gpt-5.5-fast` is rejected.

## Troubleshooting

### Skills not loading

```bash
# Check installed AutoShip package assets
ls ~/.config/opencode/.autoship/

# Verify OpenCode registers the package plugin
jq '.plugin' ~/.config/opencode/opencode.json
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
# Remove installed package assets
rm -rf ~/.config/opencode/.autoship

# Remove state (optional)
rm -rf .autoship/
```
