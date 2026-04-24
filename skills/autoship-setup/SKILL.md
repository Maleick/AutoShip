---
name: autoship-setup
description: Interactive setup wizard for AutoShip on OpenCode — model selection, tool detection, concurrency tuning
platform: opencode
tools: ["Bash", "question", "Write"]
---

# AutoShip Setup Wizard — OpenCode Port

Guide users through model selection and configuration on first run.

---

## Flow Overview

1. **Runtime Detection** — Check for OpenCode CLI
2. **Model Discovery** — List models from the current `opencode models` output
3. **Model Selection** — Default to free models, or let the operator choose explicit models
3. **Concurrency** — How many agents?
4. **Summary** — Ready to go

---

## Step 1: Runtime Detection

```bash
command -v opencode >/dev/null 2>&1 && opencode --version
```

## Step 2: Model Discovery

```bash
opencode models
```

## Step 3: Model Configuration

Ask the user:

```
Which model configuration?

◉ Free-first OpenCode
  └ Prefer configured free OpenCode models
  └ Do not include paid models by default

◯ Custom OpenCode models
  └ Operator chooses model IDs from the current `opencode models` output
  └ Writes `.autoship/model-routing.json`
  └ Explicitly selected non-free models are allowed
```

---

## Step 3: Concurrency

Ask:

```
How many concurrent agents?

◉ Conservative (5 agents)
  └ Lowest cost, good for testing

◯ Standard (10 agents)
  └ Balanced throughput/cost

◯ Aggressive (15 agents)
  └ Higher parallelism for trusted queues
```

---

## Step 4: Generate Config

```bash
# Create .autoship directory
mkdir -p .autoship

AUTOSHIP_MAX_AGENTS="$MAX_AGENTS" AUTOSHIP_MODELS="$SELECTED_MODELS" bash hooks/opencode/setup.sh
```

Setup preserves existing `.autoship/model-routing.json` by default so operators can edit it manually. Use `AUTOSHIP_REFRESH_MODELS=1` to regenerate free defaults from the current OpenCode model inventory.

---

## Step 5: Summary

```
✓ AutoShip configured!

Model configuration: <config>
Runtime: OpenCode
Models: <free discovered models or explicit operator selection>
Concurrency: <N> agents

Next: Run /autoship to start
```

---

## Error Recovery

| Error | Recovery |
|-------|----------|
| OpenCode not found | Install OpenCode before starting workers |
| Config write fails | Check file permissions |

---

## Reconfiguration

```bash
rm .autoship/.onboarded
# Then re-run /autoship-setup
```
