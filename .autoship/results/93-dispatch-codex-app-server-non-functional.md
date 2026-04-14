# Result: #93 — Dispatch: codex app-server non-functional — add Claude/Gemini auto-fallback

## Status: DONE

## Changes Made

- `hooks/dispatch-codex-appserver.sh`: Added `mark_exhausted` function to write `exhausted: true` for `codex-spark` to `.autoship/quota.json`. Added logic to detect app-server start failure and initialization timeout/failure, marking the tool as exhausted and exiting with `STUCK`.
- `hooks/detect-tools.sh`: Updated `detect_codex` to probe app-server availability using `codex app-server help`. If the probe fails, it marks `codex-spark` as exhausted in `quota.json`.
- `skills/dispatch/SKILL.md`: Documented the fallback protocol (gemini > claude-haiku > claude-sonnet) and the requirement for exec fallback for `codex-spark`.

## Tests

- Command: `bash hooks/detect-tools.sh` with a mock `codex` script simulating failure.
- Result: PASS
- New tests added: no (tested via manual execution and mock)

## Notes

- The `mark_exhausted` function ensures `.autoship/quota.json` is initialized with `{}` if it doesn't exist.
- Probe uses `codex app-server help` as it is a safe, non-hanging command that confirms the app-server's availability.
