# Result: #43 — Implement Codex app-server JSON-RPC dispatch (replace tmux pipe-pane)

## Status: DONE

## Changes Made

- `hooks/dispatch-codex-appserver.sh` (new, executable): Spawns `codex app-server` via mkfifo IPC, drives JSON-RPC protocol (initialize → thread/start → turn/start), reads turn/completed|turn/failed|thread/tokenUsage/updated events, writes COMPLETE/STUCK to pane.log, emits verify/agent_stuck event to event-queue.json via atomic flock write. 300s stall watchdog kills subprocess on timeout.
- `skills/beacon-dispatch/SKILL.md`: Removed Step 0 (Verify Tmux Layout) entirely. Replaced Step 3A Codex tmux block with `bash hooks/dispatch-codex-appserver.sh "$ISSUE_KEY" "$PROMPT_FILE" &`. Gemini tmux dispatch preserved. Updated Step 2 and "20+ Pane Handling" sections to note Codex exclusion. Removed `pane_id=$PANE_ID` from Codex state update.

## Tests

- Command: `test -x hooks/dispatch-codex-appserver.sh && grep -c 'app-server' hooks/dispatch-codex-appserver.sh`
- Result: PASS
- New tests added: no

## Notes

- `monitor-agents.sh` unchanged — already tails pane.log; dispatch-codex-appserver.sh writes status words there directly
- Gemini retains tmux pane dispatch; only Codex migrated to app-server
- `pane_id` field deprecated for Codex issues in state.json
