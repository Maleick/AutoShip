---
name: discord-commands
description: Read Discord for operator commands and execute them against AutoShip state
tools: ["Read", "Write", "Bash", "Skill"]
---

# AutoShip Discord Command Channel

Poll Discord for operator commands and execute them against AutoShip state. The orchestrator calls this skill on a timer (every 2 minutes). Each invocation fetches new messages, parses commands, executes them, and replies with acknowledgment.

## Step 1: Load Last-Seen Timestamp

Read the last-processed message timestamp to avoid reprocessing commands.

```bash
if [ -f .autoship/discord-last-seen.json ]; then
  LAST_SEEN=$(jq -r '.last_seen_id // ""' .autoship/discord-last-seen.json)
else
  LAST_SEEN=""
fi
```

If the file does not exist, treat all fetched messages as new (first run).

## Step 2: Fetch New Discord Messages

Use the Discord MCP tool to poll for messages:

```
mcp__plugin_discord_discord__fetch_messages(since: "<LAST_SEEN>")
```

- Pass `since: "<LAST_SEEN>"` when `LAST_SEEN` is non-empty so only messages newer than the last-seen ID are returned. If `LAST_SEEN` is empty (first run), omit the `since` parameter to fetch recent history.
- If no new messages exist, exit immediately without rewriting `.autoship/discord-last-seen.json`.
- Initialize `NEWEST_MSG_ID="$LAST_SEEN"` before iterating, then update it to each fetched message's ID as you process the batch.

## Step 3: Authorize Messages (Required)

Before parsing commands, discard any message that is not from an authorized operator.

Authorization must pass one of these checks:

- `message.author.id` is listed in `.autoship/discord-auth.json` under `allowed_user_ids`
- OR `message.member.roles` intersects `allowed_role_ids`

Example auth file:

```json
{
  "allowed_user_ids": ["123456789012345678"],
  "allowed_role_ids": ["987654321098765432"]
}
```

If `.autoship/discord-auth.json` is missing, invalid, or both allow-lists are empty, **fail closed**:

- Execute no commands
- Log an error to `.autoship/discord-commands.log`
- Reply once in channel: `Command channel disabled: no Discord authorization config.`

Only authorized messages continue to command parsing; all others are ignored silently.

## Step 4: Parse Commands

Iterate through new messages in chronological order (oldest first). For each message, attempt to match one of the following commands. Matching is **case-insensitive**. Issue numbers accept both `#N` and bare `N` formats.

| Pattern                     | Command                | Example       |
| --------------------------- | ---------------------- | ------------- |
| `work on #N` or `work on N` | Force-dispatch issue N | `work on #42` |
| `skip #N` or `skip N`       | Exclude issue N        | `Skip #15`    |
| `pause`                     | Halt dispatch loop     | `PAUSE`       |
| `resume`                    | Restart dispatch loop  | `Resume`      |
| `status`                    | Post status summary    | `status`      |

If a message does not match any command pattern, ignore it silently.

## Step 5: Execute Commands

Process each matched command in order:

### 4a. `work on #N` — Force Dispatch

Write a priority event to the event queue so the orchestrator picks it up immediately:

```bash
EVENT='{"type":"force_dispatch","issue":"'"$ISSUE_NUM"'","priority":1,"source":"discord","queued_at":"'"$(date -u +%Y-%m-%dT%H:%M:%SZ)"'"}'
jq --argjson evt "$EVENT" '. += [$evt]' .autoship/event-queue.json > /tmp/eq.json && mv /tmp/eq.json .autoship/event-queue.json
```

Reply to Discord:

> Dispatching #N — added to front of queue.

### 4b. `skip #N` — Exclude Issue

Add the issue number to the exclusion list in state.json. If the issue has a running agent, note that in the reply.

```bash
ISSUE_NUM="15"
jq --arg n "$ISSUE_NUM" '.excluded_issues += [$n] | .excluded_issues |= unique' .autoship/state.json > /tmp/st.json && mv /tmp/st.json .autoship/state.json
```

Check if the issue has a running agent:

```bash
RUNNING=$(jq -r --arg n "$ISSUE_NUM" '.issues[$n] | select(.state == "running") | .pane_id // empty' .autoship/state.json)
```

If a pane is found, the operator may want to cancel it — reply with context:

> Skipped #N. Agent running in pane `$RUNNING` — it will finish but no PR will merge.

If no running agent:

> Skipped #N — excluded from future dispatch.

### 4c. `pause` — Halt Dispatch Loop

Set the paused flag in state.json:

```bash
jq '.paused = true' .autoship/state.json > /tmp/st.json && mv /tmp/st.json .autoship/state.json
```

Count running agents to inform the operator:

```bash
RUNNING_COUNT=$(jq '[.issues[] | select(.state == "running")] | length' .autoship/state.json)
```

Reply:

> Paused. $RUNNING_COUNT agent(s) still running — they will complete but no new dispatches.

### 4d. `resume` — Restart Dispatch Loop

Clear the paused flag:

```bash
jq '.paused = false' .autoship/state.json > /tmp/st.json && mv /tmp/st.json .autoship/state.json
```

Read current phase to inform the operator:

```bash
PHASE=$(jq -r '.current_phase // "1"' .autoship/state.json)
```

Reply:

> Resumed. Dispatch restarting from phase $PHASE.

### 4e. `status` — Post Status Summary

Read state.json and build a compact summary. Include:

- **Phase**: current phase number and name
- **Paused**: yes/no
- **Agents**: count of in_progress, completed, failed
- **Issues**: total open, excluded count
- **Quota**: remaining tool budget if tracked

Format as a Discord-friendly block:

```
Phase 2 (medium) | Paused: no
Agents: 3 running, 7 done, 1 failed
Issues: 14 open, 2 excluded
```

Reply with this summary using `mcp__plugin_discord_discord__reply`.

## Step 6: Reply to Each Command

For every matched command, send an acknowledgment reply using:

```
mcp__plugin_discord_discord__reply
```

Always reply in the same channel/thread where the command was received. Keep replies concise — one or two lines max.

## Step 7: Handle Errors

If a command fails (e.g., invalid issue number, state.json write error):

- Reply with the error: `Failed to execute: <reason>`
- Do **not** halt processing of remaining commands — continue with the next message.
- Log the error to `.autoship/discord-commands.log`:

```bash
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) ERROR: $COMMAND - $REASON" >> .autoship/discord-commands.log
```

## Step 8: Update Last-Seen Timestamp

After processing all messages, write `NEWEST_MSG_ID` to the tracking file:

```bash
cat <<EOF > .autoship/discord-last-seen.json
{
  "last_seen_id": "$NEWEST_MSG_ID",
  "last_poll": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
```

This prevents reprocessing on the next poll cycle.

## Output

On completion, print a one-line summary:

```
discord-commands: processed N command(s) — [list of actions taken]
```

Examples:

```
discord-commands: processed 0 command(s) — no new messages
discord-commands: processed 2 command(s) — paused, skipped #15
discord-commands: processed 1 command(s) — force_dispatch #42
```
