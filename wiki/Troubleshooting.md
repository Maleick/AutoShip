# Troubleshooting

---

## Startup Issues

### `beacon-init.sh` fails

```
Error: not inside a git repository
```

Run `/beacon:start` from inside a git repository with a GitHub remote.

```
Error: jq is required but not found
```

Install: `brew install jq`

```
Error: gh CLI not found / not authenticated
```

Install and authenticate: `brew install gh && gh auth login`

---

## Agent Issues

### Agent dispatched but no status word emitted

The agent may have completed but forgotten to emit COMPLETE/BLOCKED/STUCK. Check the fallback:

```bash
# Check if pane died with BEACON_RESULT.md present
ls .beacon/workspaces/<issue-key>/BEACON_RESULT.md
tmux list-panes -t beacon -F '#{pane_id} #{pane_dead} #{pane_title}'
```

If `pane_dead=1` and `BEACON_RESULT.md` exists, manually queue a verify event:

```bash
jq '. += [{"type":"verify","issue":"issue-<N>","priority":2,"data":{},"queued_at":"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"}]' \
  .beacon/event-queue.json > /tmp/eq.json && mv /tmp/eq.json .beacon/event-queue.json
```

### Haiku fails twice on a simple task

Expected behavior — the escalation is automatic. Sonnet will pick up the task on the next dispatch cycle. Check `.beacon/state.json` to confirm `attempt: 2` and `agent: claude-sonnet` on the next run.

### Third-party agent crashes (no BEACON_RESULT.md)

The Monitor will emit `[AGENT_CRASH]` and queue a re-dispatch. If not automatic:

```bash
# Clean up the crashed worktree
bash hooks/cleanup-worktree.sh issue-<N>

# Re-dispatch via state reset
bash hooks/update-state.sh set-claimed <N>
```

### Worktree creation fails: "branch already exists"

Stale worktree from a previous attempt:

```bash
git worktree remove .beacon/workspaces/issue-<N> --force
git branch -D beacon/issue-<N>
# Retry dispatch
```

---

## Monitor Issues

### Monitor processes not running after context compaction

Context compaction kills Monitor processes. The orchestrator skill has a recovery step:

> After compaction, restart 3 Monitor processes (Step 6)

If Sonnet doesn't auto-restart them, manually trigger `/beacon:start` — it detects existing state and resumes without re-running UltraPlan.

### `monitor-prs.sh` emitting duplicate events

Check `.beacon/.pr-monitor-seen.json` — if it's grown large or corrupted:

```bash
echo '{}' > .beacon/.pr-monitor-seen.json
```

### Event queue growing without Sonnet consuming it

Sonnet may be stuck in a long pipeline step. Check the current event queue:

```bash
jq 'length, .[0]' .beacon/event-queue.json
```

If queue is > 10 items, something is wrong. Check tmux for the orchestrator pane and look for errors.

---

## State Recovery

### Session restart after tmux death

All agents died with the tmux session. On restart, Beacon reconciles from GitHub labels:

```bash
# Check what GitHub knows
gh issue list --repo <owner/repo> --label beacon:in-progress
gh issue list --repo <owner/repo> --label beacon:blocked

# Check what worktrees survived
ls .beacon/workspaces/
```

Issues with `beacon:in-progress` label and surviving worktrees will be re-dispatched. Issues without worktrees will start fresh.

### state.json corrupted or missing

GitHub labels are the durable source of truth. Beacon can rebuild from them:

1. Delete or rename the corrupted state: `mv .beacon/state.json .beacon/state.json.bak`
2. Run `/beacon:start` — it will re-initialize state and reconcile from GitHub labels

### PR has merge conflicts

Beacon does not auto-resolve conflicts — this is intentional. When `[PR_CONFLICT]` fires, Opus is consulted. If Opus recommends manual resolution, the issue will be marked `blocked`.

To manually resolve:

```bash
cd .beacon/workspaces/issue-<N>
git fetch origin
git rebase origin/main
# resolve conflicts
git push --force-with-lease
```

---

## Discord Issues

### Commands not being processed

Check that the Discord channel is connected (`--channels` flag on the Claude Code session). Check `.beacon/discord-last-seen.json` for the last processed timestamp.

### Webhook events not triggering dispatch

GitHub webhooks post as embeds. The beacon-discord-webhook skill looks for the embed pattern. Verify the webhook is configured on the GitHub repo to post to your Discord channel.

---

## Common `gh` Errors

```
GraphQL: Resource not accessible by integration
```

Your GitHub token lacks the required scopes. Re-authenticate: `gh auth refresh -s repo,read:org`

```
HTTP 422: Unprocessable Entity (label not found)
```

Run `hooks/beacon-init.sh` to recreate the GitHub labels.

---

## Checking Logs

```bash
# Poll log (GitHub issue sync)
tail -f .beacon/poll.log

# Agent pane output
tail -f .beacon/workspaces/issue-<N>/pane.log

# State snapshot
jq '.' .beacon/state.json

# Event queue
jq '.' .beacon/event-queue.json
```
