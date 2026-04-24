# Configuration

AutoShip configuration lives under `.autoship/` and should not be committed.

## Key Files

| File | Purpose |
| --- | --- |
| `.autoship/state.json` | Issue lifecycle and active worker state |
| `.autoship/event-queue.json` | Pending orchestration events |
| `.autoship/config.json` | Runtime config, including concurrency and role models |
| `.autoship/model-routing.json` | User-editable model routing and role config |
| `.autoship/model-history.json` | Optional learned model success/failure history |

## Model Routing

Run setup to detect current models:

```bash
bash hooks/opencode/setup.sh
```

Refresh free defaults from current OpenCode inventory:

```bash
AUTOSHIP_REFRESH_MODELS=1 bash hooks/opencode/setup.sh
```

Choose explicit models from the current inventory:

```bash
AUTOSHIP_MODELS="provider/model-a,provider/model-b" bash hooks/opencode/setup.sh
```

Manual edits to `.autoship/model-routing.json` are preserved by default.
