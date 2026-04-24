# AutoShip Result — PR #2413

## Fixes applied
1. **PR title**: Renamed from "Rebase Ranger rotation branch" to "fix: rebase Ranger rotation branch" to satisfy `Validate PR title format` (conventional-commits). Now passing.
2. **PR gate failure (tarpaulin)**: Previous run errored with `could not parse/generate dep info at: .ci-target/.../time-*.d — No such file or directory`. Root cause = stale/corrupt tarpaulin build cache in the self-hosted runner's `.ci-target` directory — flaky infra, not a source-code defect. Mitigation: pushed empty commit `ci: retrigger checks after flaky tarpaulin dep-info failure` (56dc75cf) to force a clean rebuild.

## Branch state
- Branch `merge/codex-issue-1706-ranger-rotation-rebased` already on top of `origin/master` (c1446aa); no rebase needed, no conflicts.
- Pushed to origin.

## CI status at exit
- Validate PR title format: pass
- Secret scan: pass
- semgrep-cloud-platform/scan: pass
- PR gate (fmt + clippy + coverage) on Linux: pending (run 24877389503) — still compiling at tool-budget cut-off. Prior successful runs took ~15m; with a clean target cache, this should complete green.

## Next for operator
Verify PR gate goes green after current run completes. If tarpaulin `dep-info` error recurs, clear self-hosted runner cache at `.ci-target/` or pin `cargo-tarpaulin` version.
