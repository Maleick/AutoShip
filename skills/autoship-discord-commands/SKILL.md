---
name: autoship-discord-commands
description: Read Discord for operator commands and execute against AutoShip state
platform: opencode
tools: ["Read", "Write", "Bash", "Skill"]
---

# AutoShip Discord Command Channel — OpenCode Port

Poll Discord for operator commands and execute against state.

---

## Step 1: Load Last-Seen Timestamp

```bash
if [ -f .autoship/discord-last-seen.json ]; then
  LAST_SEEN=$(jq -r '.last_seen_id // ""' .autoship/discord-last-seen.json)
else
  LAST_SEEN=""
fi
```

---

## Step 2: Fetch New Messages

```
mcp__discord__fetch_messages(since: "<LAST_SEEN>")
```

---

## Step 3: Authorize Messages

Check `.autoship/discord-auth.json`:

```json
{
  "allowed_user_ids": ["123456789012345678"],
  "allowed_role_ids": ["987654321098765432"]
}
```

Fail closed if auth file missing.

---

## Step 4: Parse Commands

| Pattern | Command |
|---------|---------|
| `work on #N` | Force-dispatch issue N |
| `skip #N` | Exclude issue N |
| `pause` | Halt dispatch loop |
| `resume` | Restart dispatch loop |
| `status` | Post status summary |

---

## Step 5: Execute Commands

### `work on #N` — Force Dispatch

```bash
EVENT='{"type":"force_dispatch","issue":"'"$ISSUE_NUM"'","priority":1,"source":"discord","queued_at":"'"$(date -u +%Y-%m-%dT%H:%M:%SZ)"'"}'
jq --argjson evt "$EVENT" '. += [$evt]' .autoship/event-queue.json > .autoship/event-queue.tmp && mv .autoship/event-queue.tmp .autoship/event-queue.json
```

Reply: "Dispatching #N — added to front of queue."

### `skip #N` — Exclude Issue

```bash
jq --arg n "$ISSUE_NUM" '.excluded_issues += [$n] | .excluded_issues |= unique' .autoship/state.json > .autoship/state.tmp && mv .autoship/state.tmp .autoship/state.json
```

Reply: "Skipped #N — excluded from future dispatch."

### `pause` — Halt Dispatch

```bash
jq '.paused = true' .autoship/state.json > .autoship/state.tmp && mv .autoship/state.tmp .autoship/state.json
```

Reply: "Paused. N agent(s) still running."

### `resume` — Restart

```bash
jq '.paused = false' .autoship/state.json > .autoship/state.tmp && mv .autoship/state.tmp .autoship/state.json
```

Reply: "Resumed. Dispatch restarting from phase N."

### `status` — Post Summary

```bash
Phase: $(jq -r '.plan.current_phase // "1"' .autoship/state.json)
Paused: $(jq -r '.paused // false' .autoship/state.json)
Agents: $(jq '[.issues[] | select(.state == "running")] | length' .autoship/state.json) running
```

Reply with formatted block.

---

## Step 6: Update Last-Seen

```bash
cat <<EOF > .autoship/discord-last-seen.json
{
  "last_seen_id": "$NEWEST_MSG_ID",
  "last_poll": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
```

---

## Output

```
discord-commands: processed N command(s) — [list]
```
