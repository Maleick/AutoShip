# OpenCode Runtime Spec

AutoShip supports OpenCode as its only worker runtime.

## Runtime Model

- Model inventory comes from `opencode models` at setup time.
- Default routing includes currently available free model IDs, including `:free`/`-free` IDs and bundled free Zen models such as `opencode/big-pickle` and `opencode/gpt-5-nano`.
- OpenCode Go models (`opencode-go/*`) are included as low-cost subscription fallback models, not classified as free.
- Operators may explicitly select model IDs with `AUTOSHIP_MODELS`.
- Selected models must exist in the current OpenCode model list.
- Selected routing is saved in `.autoship/model-routing.json`, which is user-editable and preserved by setup unless refresh is requested.
- Role models are selected from live `opencode models` inventory. Setup prefers capable free models first, then OpenCode Go models, instead of requiring `openai/gpt-5.5`. Paid Zen/OpenRouter Kimi models require explicit selection.
- First-run setup prompts for orchestrator and reviewer models; they may be the same model or different models.
- Worker models are selected per task by `select-model.sh`, which scores task compatibility, cost class, configured strength, prior success/failure history, and rotates deterministically by issue number across compatible workers.
- Complex tasks require a sufficiently strong compatible worker; otherwise `select-model.sh` falls back to the configured orchestrator model as an advisor.
- Go-provider and Spark models are allowed when selected and can win if they are the best configured fit for the task.
- `openai/gpt-5.5-fast` is not allowed.
- Worker concurrency defaults to 15 active workspaces.

## Pipeline

1. `plan-issues.sh` fetches or receives issue JSON and sorts eligible issues by ascending issue number.
2. `dispatch.sh` creates an isolated worktree and prompt, assigns a configured model, and writes `QUEUED`.
3. `select-model.sh` chooses the best worker model for the task from current config and history.
4. `runner.sh` starts queued workspaces up to the configured cap.
5. Workers write `COMPLETE`, `BLOCKED`, or `STUCK` to their workspace status file.
6. `reviewer.sh` runs the configured reviewer role model before PR creation.
7. `reconcile-state.sh` updates `.autoship/state.json` from workspace status files.
8. Verification creates conventional-title PRs only after completed work is checked.

## Setup

```bash
bash hooks/opencode/setup.sh
```

Default setup uses ranked free models from the live OpenCode model list. To select models explicitly:

```bash
AUTOSHIP_MODELS="provider/model-a,provider/model-b" bash hooks/opencode/setup.sh
```

## Verification

```bash
bash hooks/opencode/test-policy.sh
bash hooks/opencode/smoke-test.sh
```
