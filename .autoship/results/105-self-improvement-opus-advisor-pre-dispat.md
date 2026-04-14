# Result: #105 — Opus pre-dispatch for risky complex issues

## Status: DONE

## Changes Made
- `skills/dispatch/SKILL.md`: Added Opus Advisor gate condition and prompt template to Step 3C. Updated Sonnet prompt template with an `## Architectural Guidance` placeholder.
- `hooks/quota-update.sh`: Added `advisor-call` subcommand, initialized `advisor_calls_today` and `last_advisor_reset` in `quota.json`, and ensured the counter is reset daily.

## Tests
- Command: `bash -n hooks/quota-update.sh`
- Result: PASS

## Notes
- `advisor_calls_today` is initialized to 0 and automatically reset to 0 whenever `quota-update.sh refresh` or `reset` (all tools) is called on a new day.
- The Opus Advisor gate checks for `complexity == complex` and relevant keywords (`unsafe`, `DLL`, `hook`, `injection`), high-risk labels, or crate spanning.
- The `advisor-call` subcommand increments the counter in `quota.json`.
