---
description: "Gracefully stop all Beacon agents and save state"
allowed-tools: ["Bash", "Read", "Write", "Skill", "ToolSearch"]
---

<beacon-stop>

You are Beacon's **Sonnet executor**. Run the stop protocol.

## Phase 0: Kill Monitors + Drain Event Queue

```bash
for pid_file in .beacon/.monitor-agents.pid .beacon/.monitor-prs.pid .beacon/.monitor-issues.pid; do
  [[ -f "$pid_file" ]] && kill "$(cat "$pid_file")" 2>/dev/null && rm "$pid_file"
done
echo '[]' > .beacon/event-queue.json
```

## Phase 1: Signal Agents (Graceful)

```bash
tmux list-panes -t beacon -F '#{pane_id} #{pane_title} #{pane_dead}' 2>/dev/null
```

For each active pane, send `C-c`. Wait up to 15 seconds.

## Phase 2: Save State

Update `.beacon/state.json`: mark in-progress issues with their current worktree paths and timestamps.

Add `beacon:paused` GitHub label to all in-progress issues:

```bash
gh label create "beacon:paused" --color "FFA500" --description "Beacon agent paused" 2>/dev/null || true
# For each in-progress issue:
gh issue edit <number> --add-label "beacon:paused"
```

## Phase 3: Force Kill (if needed)

After grace period, kill any remaining panes:

```bash
tmux list-panes -t beacon -F '#{pane_id} #{pane_title} #{pane_dead}' 2>/dev/null
# tmux kill-pane -t <pane_id> for any survivors
```

Report: `Beacon stopped. Completed: N, Paused: N, Killed: N`

</beacon-stop>
