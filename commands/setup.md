---
name: setup
description: Configure AutoShip for OpenCode-first-party routing
---

# /autoship:setup

Configure AutoShip for OpenCode-only workers.

The setup flow verifies `opencode`, discovers models with `opencode models`, writes `.autoship/model-routing.json`, writes `.autoship/config.json`, and chooses a concurrency cap. The default cap is 15 active workers.

By default setup includes only currently available model IDs flagged free in the live OpenCode model list, across all providers. Set `AUTOSHIP_MODELS` to a comma-separated list to choose exact models from the discovered OpenCode list.
