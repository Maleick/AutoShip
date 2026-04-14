# Result: #95 — Dispatch: compound issue sub-keys (issue-757a) rejected by dispatch-codex-appserver.sh

## Status: DONE

## Changes Made

- `hooks/dispatch-codex-appserver.sh`: Updated validation regex to allow compound keys (e.g. `issue-757a`, `issue-757-1`). Updated `jq` event generation to extract only the numeric prefix for the `issue_number` field to avoid `tonumber` failures.
- `hooks/cleanup-worktree.sh`: Updated validation regex and extracted numeric `ISSUE_NUM` for `gh` CLI calls to support compound keys while maintaining GitHub integration.
- `hooks/update-state.sh`: Updated validation regex for `ISSUE_ID` to accept compound formats.
- `skills/dispatch/SKILL.md`: Documented the allowed issue key formats (e.g., `issue-<N>a`, `issue-<N>-1`).

## Tests

- Command: Manual regex and extraction tests using `grep -E`, `sed`, and `jq`.
- Result: PASS
- New tests added: no (tested via shell)

## Notes

All hooks that validate `ISSUE_KEY` or `ISSUE_ID` were updated to ensure consistent support across the dispatch, monitoring, and cleanup lifecycle. The `issue_number` field in events and GitHub calls now correctly defaults to the leading numeric prefix of any compound key.
