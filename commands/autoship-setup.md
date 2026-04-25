---
name: autoship-setup
description: Configure AutoShip for OpenCode model routing and first-run setup
platform: opencode
---

# /autoship-setup — Setup Wizard

Configure AutoShip for OpenCode-only workers.

## Run Setup Skill

Invoke the `autoship-setup` skill for the interactive setup wizard.

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

## Next Step

After setup completes, run `/autoship` to start orchestration.
