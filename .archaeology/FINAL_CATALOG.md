# Final Catalog

Mode: survey
Date: 2026-05-08

## Verified Results

- TypeScript typecheck passed.
- Bash syntax checks passed.
- OpenCode syntax umbrella check passed.
- Package dry-run verification passed with 139 files.
- npm audit found 0 vulnerabilities.
- `madge` found no circular dependencies.
- `jscpd` found no exact duplication at the configured threshold.

## High-Value Findings

- Static dead-code tooling needs project-specific ignores before restore mode: `.autoship/`, `.worktrees/`, `dist/`, package plugin assets, and `npx --package` workflow binaries all produce false positives.
- Shell quality is the largest debt stratum: `shellcheck` found 45 finding blocks across 22 files, and guarded-failure patterns are widespread.
- Type hardening opportunities remain in `src/cli.ts` and `src/types.ts`, but the current strict compiler setup is healthy and changes may affect public API surfaces.
- No dependency cycles, exact clones, TypeScript suppressions, empty JS/TS catches, or npm vulnerabilities were found.

## Restore Candidates

- Create a checked-in static-analysis config for `knip` that excludes generated/runtime/package-asset strata.
- Triage shellcheck findings starting with `hooks/opencode/monitor-agents.sh`, `hooks/opencode/create-pr.sh`, `hooks/opencode/dispatch.sh`, and `hooks/opencode/runner.sh`.
- Review `eval` usage in `hooks/lib/test-fixtures.sh` and `hooks/opencode/anti-flake.sh` for safer command execution boundaries.
- Review weak source type positions in `src/cli.ts` and `src/types.ts`; do not edit generated `dist/*.d.ts` directly.

## Preservation

- This pass made archaeology report changes only.
- No source code, runtime state, generated package output, or user-modified shell hook files were reverted or modified.
