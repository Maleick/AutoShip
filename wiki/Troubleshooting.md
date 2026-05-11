# Troubleshooting

## Package install looks stale

Run the package installer, then diagnostics:

```bash
npm install -g opencode-autoship
opencode-autoship install
opencode-autoship doctor
```

The installer refreshes AutoShip assets under your OpenCode config directory and registers `opencode-autoship` in `opencode.json`. If you are developing from a source checkout, this repo-local path is also valid:

```bash
bash hooks/opencode/install.sh
```

## Doctor reports failures

Run:

```bash
opencode-autoship doctor
```

Failures usually mean the OpenCode package registration, installed assets, `.autoship/config.json`, or `.autoship/model-routing.json` are missing or stale. Run `opencode-autoship install`, then `/autoship-setup` or `bash hooks/opencode/setup.sh` to regenerate project-local runtime config.

Warnings are not always blockers. For example, missing onboarding means setup has not run yet, and model inventory warnings can occur when `opencode models` is unavailable in the current shell.

## Setup finds no models

Run:

```bash
opencode models
```

If the expected provider models are missing, reconnect the provider in OpenCode and rerun setup. AutoShip defaults to ranked live models flagged free; if no free models are available, choose exact worker models explicitly:

```bash
AUTOSHIP_MODELS="provider/model-a,provider/model-b" bash hooks/opencode/setup.sh
```

## Manual model edits disappeared

Setup preserves `.autoship/model-routing.json` by default. It regenerates only when you use `AUTOSHIP_REFRESH_MODELS=1` or provide `AUTOSHIP_MODELS=...`.

Equivalent flags are `--refresh-models` and `--worker-models`.

## Role model is not what I expected

Planner, coordinator, orchestrator, reviewer, and lead are frontier roles selected from the live `opencode models` inventory. AutoShip prefers capable free models first, then OpenCode Go models when available; `openai/gpt-5.5-fast` is not allowed.

Use `AUTOSHIP_PLANNER_MODEL` or `--planner-model` to set all frontier roles together. Use `AUTOSHIP_ORCHESTRATOR_MODEL` / `--orchestrator-model` or `AUTOSHIP_REVIEWER_MODEL` / `--reviewer-model` for first-run role choices, and `AUTOSHIP_LEAD_MODEL` or `--lead-model` to override only the lead role.

## Local state should not be committed

The repository `.autoship/` directory is project-local runtime state. It contains files such as `state.json`, `event-queue.json`, `config.json`, `routing.json`, `model-routing.json`, workspaces, results, and logs.

Do not commit `.autoship/`. If state looks corrupt, prefer targeted setup or reconciliation first:

```bash
bash hooks/opencode/setup.sh
bash hooks/opencode/reconcile-state.sh
```

## Workers are queued but not running

Run:

```bash
bash hooks/opencode/status.sh
bash hooks/opencode/runner.sh
```

The runner starts queued work up to the configured active cap.

### opencode run exits with code 128 (signal kill)

If you see workers exiting immediately with code 128, `opencode run` is hanging or being killed by the system. This happens when:

- The OpenCode CLI session is stale or cannot start nested runs
- The selected model is not available or quota is exhausted
- Resource limits (memory/CPU) are being hit

**Fix:** Use `hermes session create --agent` instead of `opencode run`. The runner auto-detects Hermes and uses it when available. To force Hermes mode:

```bash
export AUTOSHIP_RUNTIME=hermes
bash hooks/opencode/runner.sh
```

### Model routing returns empty (no free models found)

If `select-model.sh` returns empty and workers fail to start, check `config/model-routing.json`:

```bash
cat config/model-routing.json | jq '.models[] | select(.cost == "free")'
```

The `select-model.sh` script filters for models with `"cost": "free"`. If your primary model (e.g., `kimi-k2.6`) has `"cost": "selected"` or another value, it will not be picked.

**Fix:** Edit `config/model-routing.json` and set the cost field to `"free"` for your desired models, then copy to `.autoship/`:

```bash
# Edit config/model-routing.json, then:
cp config/model-routing.json .autoship/model-routing.json
```

### Subagent timeout (900s) on complex issues

Hermes `delegate_task` has a 900-second (15-minute) timeout. Complex Rust builds or multi-file refactors may exceed this.

**Fix:** The subagent will commit whatever progress it has made. Check the worktree, finish compilation fixes manually if needed, then commit and push:

```bash
cd .worktrees/issue-NNNN
cargo test --no-run  # check if it compiles
git add -A
git commit -m "feat: partial implementation (issue #NNNN)"
git push origin autoship/issue-NNNN
gh pr create --title "..." --body "Closes #NNNN"
```

### Pre-existing compilation errors block all PRs

If `cargo test --no-run` fails on `main`/`master`, all dispatched workers will also fail because they branch from the broken base.

**Fix:** Fix compilation on `main` first, then dispatch workers:

```bash
cd ~/Projects/TextQuest
cargo test --no-run  # identify errors
# Fix errors...
git add -A
git commit -m "fix: resolve compilation errors"
git push origin main
```

## Status looks stale

Run:

```bash
bash hooks/opencode/reconcile-state.sh
bash hooks/opencode/status.sh
```
