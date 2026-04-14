Implement the following AutoShip self-improvement issue.

## Issue: #101 — routing.json should route unsafe/Windows Rust issues away from Gemini/Codex to Claude

## Problem

AutoShip routes `simple_code`/`medium_code` to Gemini/Codex first, but Rust/Windows projects with `unsafe`, `#[cfg(windows)]`, `retour`, DLL injection, and nightly-only crates consistently STUCK on those agents (100% STUCK rate on TextQuest).

## Acceptance Criteria

1. Add a `rust_unsafe` task type to the routing table in `hooks/init.sh` (where routing.json defaults are set) that always routes to `claude-haiku` (simple) or `claude-sonnet` (medium/complex)
2. In `skills/dispatch/SKILL.md`, document a routing override rule: if issue body/title contains `unsafe`, `#[cfg(windows)]`, `retour`, `DLL`, `cdylib`, or `winapi` → promote to Claude regardless of complexity
3. In `hooks/detect-tools.sh` or `hooks/init.sh`, add `project_language` detection: check for `Cargo.toml` + `#[cfg(windows)]` in `src/` to set `rust_windows` project profile. When detected, override routing to prefer Claude
4. Update `AUTOSHIP.md` front matter routing section to document `rust_unsafe` task type

## Instructions

- Read `hooks/init.sh` to understand how routing.json defaults are written
- Read `skills/dispatch/SKILL.md` to understand the routing matrix and task types
- Read `hooks/detect-tools.sh` to understand project detection hooks
- Make the changes. Keep them surgical — don't rewrite unrelated logic
- Before committing, verify `hooks/init.sh` is valid bash (no syntax errors): `bash -n hooks/init.sh`
- Commit to the current branch
- Do NOT push, merge, or close the issue

## Merge Note

This PR does not conflict with any currently running issue.

## When Finished

Write `AUTOSHIP_RESULT.md` to the worktree root.

```
# Result: #101 — rust_unsafe routing override

## Status: DONE | PARTIAL | STUCK

## Changes Made
- <file>: <what changed>

## Tests
- Command: bash -n hooks/init.sh
- Result: PASS | FAIL

## Notes
<anything for reviewer>
```

Print exactly one of: COMPLETE / BLOCKED / STUCK
