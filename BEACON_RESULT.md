# Result: #80 — Fix BEACON_RESULT.md archival bug

## Status: DONE

## Changes Made
- hooks/cleanup-worktree.sh: Added fallback check for parent repo root BEACON_RESULT.md (warns to poll.log); added content validation that first line must match `^# Result: #[0-9]+`; skips archival with stderr warning if validation fails

## Tests
- Command: bash -n hooks/cleanup-worktree.sh
- Result: PASS

## Notes
- Root cause: Phase-1 agents without worktrees wrote to parent repo root; archival would fail to find worktree file. Fallback now handles this path.
- Content validation guards against stale/wrong-issue files being archived.
