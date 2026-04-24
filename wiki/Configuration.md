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

## Model Routing Schema

The `model-routing.json` file defines role assignments and worker pools:

```json
{
  "roles": {
    "planner": "openai/gpt-5.5",
    "coordinator": "openai/gpt-5.5",
    "orchestrator": "openai/gpt-5.5",
    "reviewer": "openai/gpt-5.5",
    "lead": "openai/gpt-5.5"
  },
  "pools": {
    "default": {
      "description": "Default worker pool for general tasks",
      "models": ["model-a", "model-b"]
    },
    "frontend": {
      "description": "Frontend development tasks",
      "models": ["model-a"]
    },
    "backend": {
      "description": "Backend development tasks",
      "models": ["model-b"]
    },
    "docs": {
      "description": "Documentation tasks",
      "models": ["model-a"]
    },
    "mechanical": {
      "description": "Mechanical/boilerplate tasks",
      "models": ["model-a"]
    }
  },
  "defaultFallback": "model-a",
  "models": [
    {"id": "model-a", "cost": "free", "strength": 90, "max_task_types": ["docs", "simple_code"]},
    {"id": "model-b", "cost": "selected", "strength": 110, "max_task_types": ["medium_code", "complex"]}
  ]
}
```

### Roles

| Role | Purpose | Default |
| --- | --- | --- |
| `planner` | Plans issue work and acceptance criteria | openai/gpt-5.5 |
| `coordinator` | Coordinates multi-agent workflows | openai/gpt-5.5 |
| `orchestrator` | Orchestrates issue→PR pipeline | openai/gpt-5.5 |
| `reviewer` | Reviews PRs before merge | openai/gpt-5.5 |
| `lead` | Leads individual agent work | openai/gpt-5.5 |

### Worker Pools

Worker pools allow routing to specialized model groups:

- `default` - General task pool
- `frontend` - Frontend/UI work
- `backend` - Backend/complex work
- `docs` - Documentation tasks
- `mechanical` - Boilerplate tasks

### Model Entry Fields

| Field | Type | Description |
| --- | --- | --- |
| `id` | string | Model ID from `opencode models` |
| `cost` | string | "free" or "selected" |
| `strength` | int | Capability score (0-150) |
| `max_task_types` | array | Compatible task types |

### Model Selection

Use the select-model.sh hook to query models:

```bash
# Get model for a task
bash hooks/opencode/select-model.sh medium_code 170

# Get model for a specific role
bash hooks/opencode/select-model.sh --role lead

# Get models in a specific pool
bash hooks/opencode/select-model.sh --pool frontend
```

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
