# Code Archaeology Site Survey

Date: 2026-05-08
Mode: survey
Repository: `/Users/maleick/Projects/AutoShip`

## Baseline

- Branch: `main`
- Head: `b16ae0ee7548e40124f2e6819a36977bd95b09a6`
- Working tree at survey start was already dirty: `.archaeology/site_survey.md`, five shell hook files, and untracked `.tmp/` were present.
- Inventory, excluding `.git`, `node_modules`, `.autoship`, `.worktrees`, `graphify-out`, and prior `.archaeology` reports: 190 text-like files and approximately 28,640 lines.
- Dominant strata: 103 shell scripts, 57 markdown docs, 13 JSON files, 7 TypeScript files, 6 workflow files, 3 JavaScript files, and 1 module script.

## Baseline Verification

- `npm run typecheck -- --pretty false`: passed.
- `bash -n hooks/opencode/*.sh hooks/*.sh hooks/hermes/*.sh scripts/**/*.sh`: passed.
- `bash hooks/opencode/check.sh --syntax`: passed.
- `npm run verify:pack`: passed, package dry-run verified 139 files.
- `npm audit --audit-level=moderate`: passed, found 0 vulnerabilities.

## Tooling

- Available: `npx`, `npm`, `git`, `bash`, `jq`, `gh`, `shellcheck`, `rg`.
- Not installed as local/global commands: `madge`, `knip`, `jscpd`.
- Survey used `npx --yes` for `madge`, `knip`, and `jscpd` checks without adding dependencies.

## Stratum Summary

- Dead code: `knip` produced publish/runtime false positives in `dist/`, `.autoship/workspaces/`, `plugins/autoship.ts`, and `.github/workflows/release.yml`; only two non-runtime, non-generated findings remain for human review.
- Legacy/shim code: no deprecated package references were detected; shell compatibility and retry patterns remain intentionally broad.
- Dependencies: `madge` processed 106 files and found no circular dependency.
- Type catalog: TypeScript strict mode is enabled; weak type tokens remain in generated declarations and a few source positions.
- Type hardening: no TypeScript suppression comments were found.
- DRY: `jscpd` found 0 exact clones and 0 duplicated lines across 99 scanned files.
- Error handling: no empty JS/TS `catch {}` blocks were found; shell scripts contain many intentional `|| true` / `|| :` guards that need semantic review before any cleanup.
- Security/dependency health: npm audit found 0 vulnerabilities.

## Preservation Notes

- No source code changes were made in this survey pass.
- `.autoship/` runtime state and `.worktrees/` workspace copies were excluded from archaeology conclusions.
- Existing unrelated shell hook modifications were preserved and not reverted.
