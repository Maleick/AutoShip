# Result: #79 — Replace Grok CLI with Copilot CLI in detect-tools.sh and dispatch routing

## Status: DONE

## Changes Made

- `hooks/detect-tools.sh`: Removed `detect_grok()` function and its call in the JSON output section. Added `detect_copilot()` function that detects `gh copilot` (extension) or standalone `copilot` binary, outputs quota_pct: -1 (unknown/unlimited) and exhausted: false.
- `hooks/shims/grok-appserver.sh`: Added deprecation notice at top of file (early exit with error message). File preserved for git history.
- `skills/beacon-dispatch/SKILL.md`: Updated intro line and Dispatch Priority Matrix table to reference Copilot instead of Grok. Removed TODO comment about Grok support.
- `CLAUDE.md`: Updated Workers line to reference Codex/Gemini/Copilot instead of Codex/Gemini/Grok.

## Tests

- Command: N/A (no automated tests in this repo)
- Result: N/A
- New tests added: no

## Notes

- `detect_copilot()` supports two variants: `gh copilot` (GitHub CLI extension) and standalone `copilot` binary. Both report `quota_pct: -1` since Copilot quota is not queryable via CLI.
- The grok-appserver.sh shim exits 1 immediately with an error to stderr, preventing accidental use.
