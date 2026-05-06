# Architecture

AutoShip is OpenCode-only.

## Pipeline

1. `plan-issues.sh` builds an ascending issue plan.
2. `setup.sh` discovers live models with `opencode models` and writes `.autoship/model-routing.json`.
3. `dispatch.sh` creates worktrees, prompts, and queued status files.
4. `select-model.sh` chooses a compatible worker model for each task using task fit, cost class, configured strength, history, and deterministic issue-number rotation.
5. `runner.sh` starts queued workers up to the configured cap.
6. `reviewer.sh` invokes the configured reviewer role before PR creation.
7. `reconcile-state.sh` syncs workspace status files back into `.autoship/state.json`.

## Model Roles

| Role | Default |
| --- | --- |
| Planner | Best available role model from `opencode models` |
| Coordinator | Planner model by default |
| Orchestrator | Prompted on first-run setup |
| Reviewer | Prompted on first-run setup |
| Lead | Planner model by default |
| Workers | Free-first compatible model per task, rotated by issue number |

Complex tasks fall back to the configured orchestrator model as an advisor when no sufficiently strong compatible worker is available.

`openai/gpt-5.5-fast` is not allowed.

AutoShip does not require `openai/gpt-5.5`. Prefer capable free or OpenCode Go role models when available, especially Kimi/Kimmy/Ling 2.6-family models.

## Concurrency

Default active worker cap: 20. Dispatch may queue more work, but the runner starts only available capacity.
