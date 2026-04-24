# CI Pipeline Parallelization

## Change

The `test-matrix` job (Windows + Linux unit tests) no longer depends on `merge_gate` (fmt + clippy + coverage). Both run in parallel on push to PRs.

## Rationale

Previously `test-matrix` had `needs: merge_gate`, forcing sequential execution: Linux gate (~15–90 min) had to complete before Windows tests could start. With 2 self-hosted Linux runners and 2 self-hosted Windows runners, the serialization pinned Windows runners idle while Linux coverage ran.

Running in parallel:

- Unit tests (Windows matrix) begin immediately on push, providing fast failure signal on the most common target.
- `merge_gate` still runs independently and still gates merge (required status check).
- Overall wall-clock per PR drops roughly 40–60% on success paths.

## Operator Impact

- No behavior change for merging — `merge_gate` (PR gate) remains the required status check.
- Unit test failures surface sooner, shortening feedback loop.
- If unit tests pass but `merge_gate` fails (clippy/fmt regression), the PR is still blocked as before.

## Reverting

Restore the line `    needs: merge_gate` above the `if:` in the `test-matrix` job in `.github/workflows/ci.yml`.
