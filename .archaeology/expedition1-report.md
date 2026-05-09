# Expedition 1: Dead Code Excavation

Mode: survey

## Commands

- `npx --yes knip --reporter json`

## Findings

- `knip` exited with findings, not with a tool failure.
- Raw issue count: 262.
- Runtime/generated false-positive strata dominated the output: 254 `.autoship/workspaces/` files and 6 `dist/` package build artifacts.
- After excluding `.autoship/`, `.worktrees/`, and `dist/`, two reviewable findings remain:
- `plugins/autoship.ts` is reported as unused by static imports, but it is a packaged OpenCode plugin asset and should not be removed without install/runtime validation.
- `.github/workflows/release.yml` reports the `semantic-release` binary because the workflow invokes it through `npx --package`, not through a direct package dependency.

## Confidence

- Removal confidence: low.
- The remaining findings are runtime/package entry assets where static import analysis is insufficient.

## Action

- No code removed.
- Future restore mode should add a `knip` configuration that ignores `.autoship/`, `.worktrees/`, `dist/`, package assets, and intentional `npx --package` binaries before treating findings as actionable.
