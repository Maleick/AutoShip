# Result: #94 — Hooks: init.sh must create .pr-monitor-seen.json on first run

## Status: DONE

## Changes Made

- `hooks/init.sh`: Added initialization for `.autoship/.pr-monitor-seen.json` and `.autoship/event-queue.json` after creating the `.autoship/workspaces` directory. This ensures all necessary state files for monitors are present on first run.
- `hooks/monitor-prs.sh`: Removed redundant and potentially failing initialization of `.pr-monitor-seen.json`. Added an explicit check to verify the file exists before attempting to read it, with a clear error message directing the user to run `init.sh` first.

## Tests

- Command: `bash hooks/init.sh && bash hooks/init.sh`
- Result: PASS (verified idempotency: second run correctly skipped initialization of existing files)
- Command: `bash hooks/monitor-prs.sh`
- Result: PASS (verified it starts correctly when files are present; fails with clear error when missing)
- New tests added: no (shell scripts verified manually)

## Notes

- The fix ensures that `monitor-prs.sh` (which runs with `set -euo pipefail`) will not fail due to missing directory/file when started in a fresh workspace, provided `init.sh` has been executed as part of the startup sequence.
- Added `event-queue.json` initialization to `init.sh` as well, as it is also used by monitors and `emit-event.sh`.
