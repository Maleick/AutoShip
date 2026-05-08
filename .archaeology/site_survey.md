# Code Archaeology Site Survey

Date: 2026-05-08
Mode: survey with targeted restore for confirmed CI/TextQuest failures

## Baseline

- Branch: main
- Head: 8e875011e0f2808c5ef2f5d83b9522bfca75dcba
- Inventory: 202 tracked/source files excluding `.git`, `node_modules`, `dist`, `.autoship`, `.archaeology`, and `graphify-out`
- Approximate lines: 22,745
- Dominant strata: 102 shell scripts, 56 markdown docs, 12 JSON files, 4 TypeScript files

## Survey Findings

- CI release failure was reproducible from GitHub Actions logs: `@semantic-release/git` attempted `git push --tags ... HEAD:main`, rejected by protected branch rule `GH006`.
- TextQuest policy test initially returned `default` because `.autoship/config.json` contains `policyProfile: "default"`, which suppressed AutoShip's TextQuest auto-detection.
- Policy asset lookup resolved against the target repo, so installed/source AutoShip policy JSON was unavailable when hooks ran inside TextQuest.
- Monitor liveness regression existed on macOS `/var` vs `/private/var` paths and strict `ps` pipeline matching.

## Tool Survey

- `knip --reporter compact`: reported generated `dist/*` files and `plugins/autoship.ts` as unused, plus `semantic-release` as an unlisted workflow binary. These are package/runtime artifacts, not safe removal candidates without packaging redesign.
- `madge src hooks --extensions ts,js,sh --circular`: no circular dependencies found.
- `jscpd --min-lines 12 --min-tokens 80 src hooks scripts`: 0 exact clones, 0 duplicated lines across 98 scanned files.
- `npm audit --audit-level=moderate`: 0 vulnerabilities.

## Preservation Notes

- No dead-code removals were performed. Reported `dist/*` outputs are publish artifacts and intentionally included in `package.json` files.
- Confirmed fixes were limited to CI release configuration, policy profile/asset resolution, and monitor liveness detection.
