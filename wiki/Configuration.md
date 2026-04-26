# Configuration

AutoShip has two local configuration locations:

- OpenCode package assets live under the OpenCode config directory, usually `~/.config/opencode/.autoship/`.
- Project runtime state lives under the repository's local `.autoship/` directory and should not be committed.

For a long-term install, install the CLI globally and then register AutoShip with OpenCode:

```bash
npm install -g opencode-autoship
opencode-autoship install
opencode-autoship doctor
```

For a one-time install without keeping the CLI globally, use `bunx opencode-autoship install` instead.

When working from a source checkout, the repo-local installer is also supported and matches the Pages install guide:

```bash
bash hooks/opencode/install.sh
```

## Key Files

| File | Purpose |
| --- | --- |
| `.autoship/state.json` | Project-local issue lifecycle and active worker state |
| `.autoship/event-queue.json` | Project-local pending orchestration events |
| `.autoship/config.json` | Project-local runtime config, including concurrency and frontier role models |
| `.autoship/routing.json` | Project-local task type routing metadata |
| `.autoship/model-routing.json` | Project-local user-editable model routing and role config |
| `.autoship/model-history.json` | Optional learned model success/failure history |

## Model Routing Schema

The `model-routing.json` file defines role assignments and worker pools:

```json
{
  "roles": {
    "planner": "opencode-go/kimi-k2.6",
    "coordinator": "opencode-go/kimi-k2.6",
    "orchestrator": "opencode-go/kimi-k2.6",
    "reviewer": "opencode-go/kimi-k2.6",
    "lead": "opencode-go/kimi-k2.6"
  },
  "pools": {
    "default": {
      "description": "Default worker pool for general tasks",
      "models": ["provider/free-model", "provider/selected-model"]
    },
    "frontend": {
      "description": "Frontend development tasks",
      "models": ["provider/free-model"]
    },
    "backend": {
      "description": "Backend development tasks",
      "models": ["provider/selected-model"]
    },
    "docs": {
      "description": "Documentation tasks",
      "models": ["provider/free-model"]
    },
    "mechanical": {
      "description": "Mechanical/boilerplate tasks",
      "models": ["provider/free-model"]
    }
  },
  "defaultFallback": "provider/free-model",
  "models": [
    {"id": "provider/free-model", "cost": "free", "strength": 75, "max_task_types": ["docs", "simple_code", "medium_code"]},
    {"id": "provider/selected-model", "cost": "selected", "strength": 90, "max_task_types": ["medium_code", "complex"]}
  ]
}
```

### Frontier Roles

The frontier roles perform planning, coordination, orchestration, review, and lead decisions. They are selected from the live `opencode models` inventory instead of being pinned to one provider. Setup prefers capable free or OpenCode Go role models when available, especially Kimi/Kimmy/Ling 2.6-family models.

| Role | Purpose | Default |
| --- | --- | --- |
| `planner` | Plans issue work and acceptance criteria | Best available role model |
| `coordinator` | Coordinates multi-agent workflows | Planner model |
| `orchestrator` | Orchestrates issue-to-PR pipeline | Prompted on first run |
| `reviewer` | Reviews completed work before PR creation | Prompted on first run |
| `lead` | Makes dispatch, concurrency, and escalation decisions | Planner model |

Set all frontier roles together with `AUTOSHIP_PLANNER_MODEL` or `--planner-model`. Set orchestrator and reviewer separately with `AUTOSHIP_ORCHESTRATOR_MODEL` / `--orchestrator-model` and `AUTOSHIP_REVIEWER_MODEL` / `--reviewer-model`. Set only the lead role with `AUTOSHIP_LEAD_MODEL` or `--lead-model`.

### Worker Pools

Worker pools allow routing to specialized model groups. Setup defaults to live OpenCode models detected as free, including `:free`/`-free` IDs and bundled free Zen models such as `opencode/big-pickle` and `opencode/gpt-5-nano`. OpenCode Go models (`opencode-go/*`) are included as low-cost subscription fallback models, not free models. The selector scores compatible workers by cost class, configured strength, and `model-history.json` success/failure history. Dispatch uses deterministic issue-number rotation across the compatible sorted pool so concurrent workers do not all select the same strongest free model.

Complex tasks require a strong compatible worker. If the available pool is too weak for a complex task, AutoShip selects the configured orchestrator model as an advisor fallback.

- `default` - General task pool
- `frontend` - Frontend/UI work
- `backend` - Backend/complex work
- `docs` - Documentation tasks
- `mechanical` - Boilerplate tasks

### Model Entry Fields

| Field | Type | Description |
| --- | --- | --- |
| `id` | string | Model ID from `opencode models` |
| `cost` | string | `free` or `selected`; free models receive the highest cost score in default routing |
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

Refresh free defaults from the current OpenCode inventory:

```bash
AUTOSHIP_REFRESH_MODELS=1 bash hooks/opencode/setup.sh
```

Choose explicit models from the current inventory:

```bash
AUTOSHIP_MODELS="provider/model-a,provider/model-b" bash hooks/opencode/setup.sh
```

Equivalent setup flags are available for non-interactive runs:

```bash
bash hooks/opencode/setup.sh --no-tui --worker-models provider/model-a,provider/model-b
bash hooks/opencode/setup.sh --no-tui --refresh-models
```

Manual edits to `.autoship/model-routing.json` are preserved by default.
