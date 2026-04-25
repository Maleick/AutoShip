---
name: autoship-setup
description: Configure AutoShip for OpenCode model routing and first-run setup
platform: opencode
---

# /autoship-setup — Setup Wizard

Configure AutoShip for OpenCode-only workers.

## Interactive Mode

Invoke the `autoship-setup` skill for the interactive setup wizard:

```bash
/autoship-setup
```

The setup skill verifies OpenCode and GitHub authentication, discovers live OpenCode models, selects free-first worker routing, and writes `.autoship/model-routing.json` plus `.autoship/config.json`.

## Non-Interactive Mode

For scripted setup, run the setup hook directly:

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

- Setup includes only currently available model IDs flagged free in the live OpenCode model list.
- Set `AUTOSHIP_MODELS` or `--worker-models` to choose exact models.
- Use `--refresh-models` to regenerate from current model inventory.

## Next Step

After setup completes, run `/autoship` to start orchestration.
