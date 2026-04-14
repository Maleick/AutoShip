Implement the following AutoShip self-improvement issue.

## Issue: #102 — init.sh should auto-populate config.json test_command from CLAUDE.md build commands

## Problem

config.json is empty ({}) on new projects. Dispatch prompts fall back to the literal placeholder `<test-command>`. Agents skip testing or guess wrong commands, leading to unverified commits.

## Acceptance Criteria

1. In `hooks/init.sh`, after creating config.json, scan `CLAUDE.md` for test/build command patterns and write the best candidate to `config.json` as `test_command`
   - Patterns to detect: `cargo test`, `pytest`, `npm test`, `make test`, `./gradlew test`, `python3 scripts/dev-preflight.py`
2. Add a `verify_command` field for CI-equivalent checks (e.g. `python3 scripts/dev-preflight.py`)
3. If AGENTS.md exists, check it for explicit test commands too
4. If no test command detected, write `test_command: ""` and print a warning: "Warning: no test_command detected — set it in .autoship/config.json"
5. Must be idempotent — running init.sh twice does not overwrite an already-set test_command

## Instructions

- Read `hooks/init.sh` — understand the config.json creation block (search for "config.json")
- Keep the detection logic simple: grep/awk patterns, no external deps beyond bash + jq
- The config.json write must use jq (already a required dep) for valid JSON output
- Before committing: `bash -n hooks/init.sh` must pass

## ⚠️ Merge Note

Issue #94 also modifies `hooks/init.sh`. Coordinate with the reviewer to ensure ordered merge: #94 first, then #102.

## When Finished

Write `AUTOSHIP_RESULT.md` to the worktree root.

```
# Result: #102 — init.sh test_command auto-detection

## Status: DONE | PARTIAL | STUCK

## Changes Made
- hooks/init.sh: <what changed>

## Tests
- Command: bash -n hooks/init.sh
- Result: PASS | FAIL

## Notes
<anything for reviewer>
```

Print exactly one of: COMPLETE / BLOCKED / STUCK
