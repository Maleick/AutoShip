# CI Gate Split: Fast PR Gate + Async Coverage

## Summary

The CI workflow's monolithic `merge_gate` job has been split into two independent
jobs so that PRs can merge quickly without waiting on a 15-90 minute coverage run.

| Before                                                                 | After                                                                   |
| ---------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `merge_gate` (fmt + clippy + coverage) — 15-90 min, required for merge | `pr_gate` (fmt + clippy + wiki enforcement) — ~3-5 min, **required**    |
|                                                                        | `coverage` (tarpaulin + threshold check) — 15-90 min, **advisory only** |

## Jobs

### `pr_gate` — "PR gate (fmt + clippy) on Linux"

Fast path that must pass before merge. Includes:

- Docs-only change detection
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `scripts/sync_wiki.py --check`
- `scripts/validate_offsets_sync.py`
- Wiki enforcement (source changes require matching `docs/wiki/` updates)
- Python tests (advisory)

Runs on `[self-hosted, Linux, X64, textquest]`, 15-minute timeout.

### `coverage` — "Coverage (tarpaulin) on Linux"

Async job that runs `cargo-tarpaulin` and enforces the 68% coverage threshold.
Uses `continue-on-error: true` at the job level so failures post results but do
**not** block merge. Runs on the same self-hosted Linux runner, 90-minute
timeout.

## Operator Action Required

Branch protection rules on `master` must be updated:

1. Navigate to **Settings → Branches → master**.
2. Remove required check: `PR gate (fmt + clippy + coverage) on Linux`
3. Add required check: `PR gate (fmt + clippy) on Linux`
4. Do **not** require: `Coverage (tarpaulin) on Linux` (intentionally advisory).

Other required checks (`test-matrix`, `Secret scan`) are unchanged.

## Rationale

Coverage runs were dominating PR merge latency. Decoupling lets clippy/fmt
regressions block merges in minutes while coverage regressions post as advisory
signal developers can triage asynchronously.
