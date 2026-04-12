---
name: beacon-discord-webhook
description: Parse GitHub webhook events from Discord channel and route to Beacon event queue
tools: ["Read", "Write", "Bash"]
---

# Beacon Discord Webhook Protocol

You are Beacon's Discord webhook consumer. You poll the connected Discord channel for GitHub webhook embeds, parse them into structured events, and write them to `.beacon/event-queue.json`. You run every 2 minutes via the orchestrator.

---

## Step 1: Load Last Poll Timestamp

```bash
last_ts=$(jq -r '.discord_last_poll // empty' .beacon/state.json)
```

If `discord_last_poll` is not set, default to 5 minutes ago:

```bash
last_ts=$(date -u -v-5M +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -d '5 minutes ago' +%Y-%m-%dT%H:%M:%SZ)
```

## Step 2: Fetch Messages from Discord

Use the Discord MCP tool to fetch messages since the last poll:

```
mcp__plugin_discord_discord__fetch_messages(since: "<last_ts>")
```

This returns an array of Discord messages. Filter to only messages that contain **embeds** — these are the GitHub webhook deliveries. Skip plain-text messages from humans or bots.

## Step 3: Identify GitHub Webhook Embeds

GitHub webhook embeds have a recognizable structure:

| Field             | Pattern                                                 |
| ----------------- | ------------------------------------------------------- |
| Embed title       | `[repo] Issue #N: <title>` or `[repo] Pull Request #N`  |
| Embed color       | Green (opened/reopened), purple (labeled), red (closed) |
| Embed description | Contains event details, label names, user who acted     |
| Embed author      | GitHub username with avatar                             |
| Embed footer/url  | Links to `github.com/<owner>/<repo>/issues/<N>`         |

For each embed, extract:

1. **Issue number** — parse `#N` from the embed title or URL
2. **Repository** — parse `[repo]` from the title or extract from URL
3. **Event type** — determine from embed content (see mapping below)
4. **Actor** — the GitHub user who triggered the event (from embed author)
5. **Additional data** — labels added/removed, body changes, etc.

## Step 4: Classify Event Type

Map embed content to event types using these signals:

| Signal                                                   | Event Type       |
| -------------------------------------------------------- | ---------------- |
| Title contains "opened" or description says "opened"     | `new_issue`      |
| Title contains "closed" or color is red                  | `closed_issue`   |
| Title contains "reopened" or description says "reopened" | `reopened_issue` |
| Description mentions label added/removed                 | `issue_labeled`  |
| Title contains "edited" or body content changed          | `issue_edited`   |

### Priority Assignment

| Event Type       | Priority | Rationale                               |
| ---------------- | -------- | --------------------------------------- |
| `new_issue`      | 3        | Low — enters triage, not urgent         |
| `closed_issue`   | 1        | Urgent — may need agent cancellation    |
| `reopened_issue` | 2        | Normal — needs re-evaluation            |
| `issue_labeled`  | 3        | Low — informational unless beacon label |
| `issue_edited`   | 2        | Normal — may need reclassification      |

**Exception:** If a `beacon:blocked` or `beacon:urgent` label is added, elevate `issue_labeled` to priority 1.

## Step 5: Deduplicate Against Event Queue

Before writing, check `.beacon/event-queue.json` for duplicate events:

```bash
jq --arg issue "<N>" --arg type "<event_type>" \
  '[.[] | select(.issue == $issue and .type == $type)] | length' \
  .beacon/event-queue.json
```

If an identical `(issue, type)` pair was queued in the last 5 minutes, skip it — this prevents duplicate processing when both the Discord webhook and the GitHub poll detect the same event.

## Step 6: Write Events to Queue

Read `.beacon/event-queue.json` (initialize as `[]` if missing), append entries, write back.

### Queue Entry Format

```json
{
  "type": "new_issue",
  "issue": "42",
  "priority": 3,
  "data": {
    "source": "discord-webhook",
    "title": "Add dark mode support",
    "actor": "username",
    "labels": ["enhancement"],
    "raw_embed_title": "[repo] Issue #42: Add dark mode support"
  },
  "queued_at": "2026-04-12T01:15:00Z"
}
```

The `data.source` field is always `"discord-webhook"` — this distinguishes webhook-sourced events from poll-sourced events and Monitor events.

## Step 7: Handle Edge Cases

### Edited Issues

When an issue body is edited:

1. Read the current complexity from `.beacon/state.json`
2. If the issue is `unclaimed`, queue an `issue_edited` event so the orchestrator can reclassify
3. If the issue is `running`, queue the event but with a note in `data.needs_reclassify: true` — the orchestrator decides whether to interrupt the agent

### Reopened Issues

When a closed issue is reopened:

1. Queue a `reopened_issue` event with priority 2
2. Include `data.previous_state` from `.beacon/state.json` if the issue exists there
3. The orchestrator will handle re-dispatching — do not modify state directly

### Label-Only Changes

When a label is added or removed:

1. If it's a `beacon:*` label, queue with elevated priority
2. If it's a standard GitHub label (bug, enhancement, etc.), queue as priority 3 for metadata sync
3. Ignore bot-generated label churn — if the actor matches the repo's bot account, skip

### Unrecognized Embeds

If an embed doesn't match any known GitHub webhook pattern:

1. Do not queue an event
2. Log it for debugging: append to `.beacon/discord-webhook.log`
3. Continue processing remaining messages

## Step 8: Update Poll Timestamp

After processing all messages, update the last poll timestamp:

```bash
jq --arg ts "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
   '.discord_last_poll = $ts' \
  .beacon/state.json > .beacon/state.tmp && mv .beacon/state.tmp .beacon/state.json
```

Write a summary to `.beacon/discord-webhook.log`:

```bash
cat >> .beacon/discord-webhook.log <<EOF
[$(date -u +%Y-%m-%dT%H:%M:%SZ)] Discord webhook poll
  Messages fetched: <count>
  Webhook embeds:   <count>
  Events queued:    <count> (<types>)
  Duplicates skipped: <count>
EOF
```

Keep the log bounded:

```bash
tail -200 .beacon/discord-webhook.log > .beacon/discord-webhook.log.tmp && mv .beacon/discord-webhook.log.tmp .beacon/discord-webhook.log
```

## Output

After completing all steps, print a brief summary:

```
Discord Webhook [<timestamp>]
  Fetched: <N> messages, <M> webhook embeds
  Queued:  <N> events (<types>)
  Skipped: <N> duplicates
```

If no webhook embeds were found:

```
Discord Webhook [<timestamp>] — no new webhook events
```
