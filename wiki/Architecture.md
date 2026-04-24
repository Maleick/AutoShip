# Architecture

AutoShip is OpenCode-only.

## Pipeline

1. `plan-issues.sh` builds an ascending, safety-filtered issue plan.
2. `setup.sh` discovers live models with `opencode models` and writes `.autoship/model-routing.json`.
3. `dispatch.sh` creates worktrees, prompts, and queued status files.
4. `select-model.sh` chooses the best worker model for each task using task fit, cost class, configured strength, and history.
5. `runner.sh` starts queued workers up to the configured cap.
6. `reviewer.sh` invokes the configured reviewer role before PR creation.
7. `reconcile-state.sh` syncs workspace status files back into `.autoship/state.json`.

## Model Roles

| Role | Default |
| --- | --- |
| Planner | `openai/gpt-5.5` |
| Coordinator | `openai/gpt-5.5` |
| Orchestrator | `openai/gpt-5.5` |
| Reviewer | `openai/gpt-5.5` |
| Workers | Best configured model per task |

`openai/gpt-5.5-fast` is not allowed.

## Concurrency

Default active worker cap: 15. Dispatch may queue more work, but the runner starts only available capacity.
