# AUTOSHIP_RESULT — PR #2422 rebase + auto-merge

## Outcome
- Rebased `autoship/issue-762` onto `origin/master` (1 conflict in AUTOSHIP_RESULT.md, took PR #2422 side).
- `cargo check -p textquest-common` passes clean.
- Force-pushed; auto-merge (squash) enabled.

## Actions Taken
- `git fetch origin master && git rebase origin/master`
- Resolved conflict with `git checkout --theirs AUTOSHIP_RESULT.md`.
- `git push --force-with-lease`
- `gh pr merge 2422 --auto --squash`

## Earlier Work (preserved)
- Addressed Copilot inline comment on `textquest-common/src/offset_db.rs:42` documenting module-scoped offset maps.
- Appended **Module-Scoped Offset Parity (issue #762 / PR #2422)** section to `docs/wiki/Offsets-EQ-Internals-and-MacroQuest-References.md`.
- Covered eqmain.dll + eqgraphicsdx9.dll globals/functions, per-module rebase rule, scan-engine scope resolution, validator usage.

## Notes
- Merges automatically once required checks pass.
