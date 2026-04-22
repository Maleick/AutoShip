# Epic #2128 — Merge Queue: 11 Branches (Audit 2026-04-19 Team C)

**Status updated:** 2026-04-21
**Source audit:** `audit/team-c-hygiene.md`

## Summary

Team C identified 11 mergeable branches during the 2026-04-19 hygiene audit. All had stale merge bases (200–253 commits behind master). 2 were merged via PRs shortly after the audit. 9 remain unmerged and have been decomposed into individual child issues.

## Branch Status Table

| Branch                                  | Commits Ahead | Behind (audit) | Status       | PR / Child Issue |
| --------------------------------------- | ------------- | -------------- | ------------ | ---------------- |
| `autoresearch/docs-20260418-0507`       | 8             | 225            | **Unmerged** | #2279            |
| `autoresearch/20260418-0419`            | 7             | 225            | **Unmerged** | #2283            |
| `feature/apr18-session-work`            | 10            | 225            | **Unmerged** | #2285            |
| `feature/tui-themed-tabs`               | 0             | —              | **Merged**   | PR #2112         |
| `claude/resolve-pr-issues-8uGZN`        | 0             | —              | **Merged**   | PR #2113         |
| `codex/issue-1708-wizard-rotation`      | 3             | 232            | **Unmerged** | #2293            |
| `codex/issue-1705-berserker-rotation`   | 3             | 253            | **Unmerged** | #2295            |
| `codex/issue-1702-monk-rotation`        | 10            | 233            | **Unmerged** | #2297            |
| `codex/issue-1706-ranger-rotation`      | 5             | 233            | **Unmerged** | #2299            |
| `codex/issue-1710-necromancer-rotation` | 5             | 231            | **Unmerged** | #2300            |
| `codex/issue-1277-vendor-cycle`         | 10            | 233            | **Unmerged** | #2301            |

**Merged:** 2 of 11
**Unmerged:** 9 of 11 (child issues filed, labeled `agent:ready`)

## Merge Order (recommended — oldest base first)

1. `codex/issue-1705-berserker-rotation` (253 behind — highest conflict risk, do first) → #2295
2. `codex/issue-1708-wizard-rotation` (232 behind) → #2293
3. `codex/issue-1702-monk-rotation` (233 behind) → #2297
4. `codex/issue-1706-ranger-rotation` (233 behind) → #2299
5. `codex/issue-1277-vendor-cycle` (233 behind) → #2301
6. `codex/issue-1710-necromancer-rotation` (231 behind) → #2300
7. `autoresearch/docs-20260418-0507` (225 behind) → #2279
8. `autoresearch/20260418-0419` (225 behind) → #2283
9. `feature/apr18-session-work` (225 behind) → #2285

## Risk Notes

- Codex rotation branches all touch `textquest-dll/src/combat/classes/` — rebase one at a time, pull master between each
- `feature/apr18-session-work` overlaps with already-merged TUI/session work from PR #2112; expect conflicts
- `autoresearch/docs-20260418-0507` and `autoresearch/20260418-0419` share commits — rebase the docs branch first, then cherry-pick any unique commits from 0419

## Epic Resolution

Epic #2128 is resolvable once all 9 child issues are closed (PRs merged to master).
