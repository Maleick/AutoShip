# Fix: monitor-issues.sh baseline state-key format (issue #2227)

## Problem

`hooks/monitor-issues.sh` looked up issue tracking state using a bare number key:

```bash
tracked=$(jq -r --arg n "$num" '.issues[$n] // empty' "$STATE_FILE" 2>/dev/null)
```

However, `.autoship/state.json` stores issues under `"issue-N"` keys (e.g. `"issue-2227"`), not bare numbers (e.g. `"2227"`). This caused the lookup to always return empty, making the monitor re-emit `[ISSUE_NEW]` events for already-tracked issues on every poll cycle, and never emit `[ISSUE_CLOSED]` events.

## Root Cause

Mismatch between the key format written by `init.sh`/`reconcile-agent-queue.sh` (`"issue-N"`) and the key format used for the `jq` lookup (`"$num"` bare integer string).

## Fix

Prefix the jq argument with `"issue-"` so the lookup matches the actual state key format. Applied to both the `[ISSUE_NEW]` check (line 72) and the `[ISSUE_CLOSED]` check (line 88):

```bash
# Before
tracked=$(jq -r --arg n "$num" '.issues[$n] // empty' "$STATE_FILE" 2>/dev/null)

# After
tracked=$(jq -r --arg n "issue-$num" '.issues[$n] // empty' "$STATE_FILE" 2>/dev/null)
```

## Patch

See `.autoship/patches/monitor-issues-key-fix.patch` for the unified diff.

## Impact

- Prevents duplicate `[ISSUE_NEW]` events for already-tracked issues
- Enables correct `[ISSUE_CLOSED]` event emission when issues are closed
- No behavior change for issues not yet in state (correctly still emits `[ISSUE_NEW]`)
