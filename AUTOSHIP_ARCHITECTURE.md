# AutoShip Architecture

AutoShip is OpenCode-only.

## Roles

- Planner: `openai/gpt-5.5`
- Coordinator: `openai/gpt-5.5`
- Orchestrator: `openai/gpt-5.5`
- Reviewer: `openai/gpt-5.5`
- Workers: best configured model per task from the live `opencode models` inventory

## Flow

1. `plan-issues.sh` builds an ascending, safety-filtered issue queue.
2. `setup.sh` writes user-editable `.autoship/model-routing.json` from live OpenCode models.
3. `dispatch.sh` creates an isolated worktree and prompt, then marks the workspace queued.
4. `select-model.sh` chooses the best worker using task fit, cost class, configured strength, and historical outcomes.
5. `runner.sh` starts queued workers up to the active cap.
6. `reviewer.sh` verifies completed work before PR creation.
7. `reconcile-state.sh` synchronizes status files back into `.autoship/state.json`.

## Defaults

- Active worker cap: 15
- Queue ordering: lowest issue number first
- Default worker pool: currently available free models
- Explicit worker models: allowed if present in the current OpenCode model list
- Disallowed model: `openai/gpt-5.5-fast`

## Safety

Anti-cheat evasion, stealth, VM/fingerprint evasion, shellcode, hook signature evasion, detour hiding, and similar abuse-prone work is blocked for human review.
