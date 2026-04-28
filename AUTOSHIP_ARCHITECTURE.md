# AutoShip Architecture

AutoShip is OpenCode-only.

## Roles

- Planner: best available role model from the live `opencode models` inventory
- Coordinator: planner model by default
- Orchestrator: prompted during first-run setup
- Reviewer: prompted during first-run setup
- Workers: free-first compatible model per task from the live `opencode models` inventory

## Flow

1. `plan-issues.sh` builds an ascending issue queue.
2. `setup.sh` writes user-editable `.autoship/model-routing.json` from live OpenCode models.
3. `dispatch.sh` creates an isolated worktree and prompt, then marks the workspace queued.
4. `select-model.sh` chooses a compatible worker using task fit, cost class, configured strength, historical outcomes, and deterministic issue-number rotation.
5. `runner.sh` starts queued workers up to the active cap.
6. `reviewer.sh` verifies completed work before PR creation.
7. `reconcile-state.sh` synchronizes status files back into `.autoship/state.json`.

## Defaults

- Active worker cap: 15
- Queue ordering: lowest issue number first
- Default worker pool: ranked currently available free models
- Explicit worker models: allowed if present in the current OpenCode model list
- Disallowed model: `openai/gpt-5.5-fast`
