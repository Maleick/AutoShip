---
name: setup
description: Interactive setup wizard for OpenCode model routing and agent concurrency tuning
tools: ["Bash", "AskUserQuestion", "Write"]
---

# AutoShip Setup Wizard

AutoShip is an OpenCode-first-party skill. It uses OpenCode models only; model routing is configured in `.autoship/model-routing.json`.

## Flow Overview

1. Verify `opencode` is installed.
2. Discover models with `opencode models`.
3. Default to free models, or use operator-selected model IDs.
4. Choose concurrency: 5, 10, or 15 agents.
5. Write `.autoship/config.json`, `.autoship/model-routing.json`, and `.autoship/.onboarded`.

## Runtime Detection

```bash
command -v opencode >/dev/null 2>&1 && echo "OpenCode available" || echo "OpenCode not found"
```

## Concurrency

Default to 15 active workers. Use a lower `AUTOSHIP_MAX_AGENTS` value for conservative runs.

## Model Routing

Run `bash hooks/opencode/setup.sh`. Without `AUTOSHIP_MODELS`, setup writes only live model IDs flagged free in the current `opencode models` output, across all OpenCode providers. With `AUTOSHIP_MODELS`, setup writes exactly the comma-separated models selected by the operator after verifying each ID exists in the current OpenCode model list.

## Error Recovery

| Error | Recovery |
| ----- | -------- |
| OpenCode not found | Install OpenCode before starting workers |
| Missing jq | Abort with install instructions: `brew install jq` |
| Config write fails | Check file permissions |
