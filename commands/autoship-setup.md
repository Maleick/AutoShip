---
name: autoship-setup
description: Configure AutoShip for OpenCode-first-party routing
platform: opencode
---

# /autoship-setup — Setup Wizard

Configure AutoShip for OpenCode-only workers.

## Interactive Mode

Runs a wizard that asks about:
1. OpenCode availability (verified automatically)
2. GitHub authentication (verified automatically)
3. Model inventory discovery (from `opencode models`)
4. Concurrency (default: 15 agents)
5. Labels to monitor (default: agent:ready)
6. Refresh behavior (auto-refresh models on startup?)

```bash
/autoship-setup
```

## Non-Interactive Mode (--no-tui)

Use `--no-tui` for scripted or CI setups:

```bash
bash hooks/opencode/setup.sh --no-tui \
  --max-agents 10 \
  --labels "agent:ready,autoship:in-progress" \
  --refresh-models \
  --planner-model openai/gpt-5.5
```

## Flags

| Flag | Description | Default |
|------|-------------|---------|
| `--no-tui` | Run in non-interactive mode | false |
| `--max-agents N` | Max concurrent agents | 15 |
| `--labels LABEL1,LABEL2` | Labels to monitor | agent:ready |
| `--refresh-models` | Force refresh model inventory | false |
| `--planner-model MODEL` | Planner/coordinator/orchestrator model | openai/gpt-5.5 |
| `--worker-models MODEL1,MODEL2` | Worker models (comma-separated) | auto-detect free |

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `AUTOSHIP_MAX_AGENTS` | Max concurrent agents | 15 |
| `AUTOSHIP_MODELS` | Worker models | auto-detect free |
| `AUTOSHIP_REFRESH_MODELS` | Set to 1 to force refresh | 0 |
| `AUTOSHIP_PLANNER_MODEL` | Planner model | openai/gpt-5.5 |
| `AUTOSHIP_LABELS` | Labels to monitor | agent:ready |
| `GH_TOKEN` | GitHub token | from gh config |

## Defaults

- Setup includes only currently available model IDs flagged free in the live OpenCode model list
- Set `AUTOSHIP_MODELS` or `--worker-models` to choose exact models
- Use `--refresh-models` to regenerate from current model inventory
