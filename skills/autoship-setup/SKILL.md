---
name: autoship-setup
description: Interactive setup wizard for AutoShip on OpenCode — model selection, tool detection, concurrency tuning
platform: opencode
tools: ["Bash", "question", "Write"]
---

# AutoShip Setup Wizard — OpenCode Port

Guide users through model selection and configuration on first run.

---

## Usage

### Interactive Mode (default)

```bash
/autoship-setup
```

### Non-Interactive Mode (--no-tui)

```bash
/autoship-setup --no-tui \
  --max-agents 10 \
  --labels "agent:ready,autoship:in-progress" \
  --refresh-models \
  --planner-model openai/gpt-5.5
```

---

## Flow Overview

1. **Runtime Detection** — Check for OpenCode CLI
2. **GitHub Auth** — Verify `gh` auth status
3. **Model Discovery** — List models from `opencode models`
4. **Model Selection** — Free-first or custom
5. **Concurrency** — How many agents?
6. **Labels** — Which labels to monitor?
7. **Refresh Behavior** — Auto-refresh model inventory?
8. **Summary** — Ready to go

---

## Step 1: Runtime Detection

```bash
command -v opencode >/dev/null 2>&1 && opencode --version
```

Ask user: "OpenCode not found. Install it first."

---

## Step 2: GitHub Auth

```bash
gh auth status
```

Ask user if not authenticated:
```
GitHub authentication required.

Run: gh auth login

Or set GH_TOKEN environment variable.
```

---

## Step 3: Model Discovery

```bash
opencode models
```

---

## Step 4: Model Configuration

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

## Step 5: Concurrency

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

## Step 6: Labels

Ask:

```
Which labels should AutoShip monitor for issues?

◉ agent:ready (default)
  └ Standard AutoShip queue label

◯ Custom labels (comma-separated)
  └ e.g., "agent:ready,needs-triage"
```

---

## Step 7: Refresh Behavior

Ask:

```
How often should AutoShip refresh model inventory?

◉ Auto-refresh on startup
  └ Checks for new models each time AutoShip starts

◯ Manual only
  └ Only refreshes when AUTOSHIP_REFRESH_MODELS=1 is set
```

---

## Step 8: Generate Config

```bash
# Interactive mode
bash hooks/opencode/setup.sh

# Non-interactive mode (--no-tui)
bash hooks/opencode/setup.sh --no-tui \
  --max-agents 10 \
  --labels "agent:ready" \
  --refresh-models \
  --planner-model openai/gpt-5.5
```

Setup preserves existing `.autoship/model-routing.json` by default so operators can edit it manually. Use `AUTOSHIP_REFRESH_MODELS=1` or `--refresh-models` to regenerate free defaults from the current OpenCode model inventory.

---

## Step 9: Summary

```
✓ AutoShip configured!

Runtime: OpenCode
GitHub: Authenticated
Models: <free discovered models or explicit operator selection>
Concurrency: <N> agents
Labels: <configured labels>
Refresh: <auto/manual>

Next: Run /autoship to start
```

---

## Non-Interactive Flags (--no-tui)

| Flag | Description | Default |
|------|-------------|---------|
| `--max-agents N` | Set max concurrent agents | 15 |
| `--labels LABEL1,LABEL2` | Comma-separated labels to monitor | agent:ready |
| `--refresh-models` | Force refresh model inventory | false |
| `--planner-model MODEL` | Set planner/coordinator/orchestrator model | openai/gpt-5.5 |
| `--worker-models MODEL1,MODEL2` | Comma-separated worker models | auto-detect free models |

---

## Error Recovery

| Error | Recovery |
|-------|----------|
| OpenCode not found | Install OpenCode before starting workers |
| GitHub not authenticated | Run `gh auth login` or set GH_TOKEN |
| Config write fails | Check file permissions |
| No free models found | Use `--worker-models` to specify explicitly |

---

## Reconfiguration

```bash
# Remove onboarding marker to re-run wizard
rm .autoship/.onboarded

# Or re-run with --no-tui flags
/autoship-setup --no-tui --max-agents 10 --refresh-models
```
