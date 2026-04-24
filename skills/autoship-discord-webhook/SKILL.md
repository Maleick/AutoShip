---
name: autoship-discord-webhook
description: Parse GitHub webhook events from Discord channel and route to AutoShip event queue
platform: opencode
tools: ["Read", "Write", "Bash"]
---

# AutoShip Discord Webhook Protocol — OpenCode Port

Poll Discord for GitHub webhook embeds, parse into events, write to `.autoship/event-queue.json`.

---

## Step 1: Load Last Poll Timestamp

```bash
last_ts=$(jq -r '.discord_last_poll // empty' .autoship/state.json)
if [[ -z "$last_ts" ]]; then
  last_ts=$(date -u -d '5 minutes ago' +%Y-%m-%dT%H:%M:%SZ)
fi
```

---

## Step 2: Fetch Discord Messages

Use Discord MCP:
```
mcp__discord__fetch_messages(since: "<last_ts>")
```

Filter to messages with **embeds** (GitHub webhook deliveries).

---

## Step 3: Verify Webhook Identity

Accept messages only if:
1. `webhook_id` in `.autoship/discord-auth.json` under `allowed_webhook_ids`
2. Embed URL/footer links to `github.com`
3. Repository matches current repo

```json
{
  "allowed_webhook_ids": ["123456789012345678"]
}
```

Fail closed if auth file missing.

---

## Step 4: Identify GitHub Webhook Embeds

GitHub webhook embeds:
- Title: `[repo] Issue #N: <title>`
- Color: Green (opened), red (closed), purple (labeled)
- URL/footer: `github.com/<owner>/<repo>/issues/<N>`

Extract:
- Issue number from `#N`
- Repository from title or URL
- Event type from content

---

## Step 5: Classify Event Type

| Signal | Event Type |
|--------|------------|
| Title contains "opened" | `new_issue` |
| Title contains "closed" | `closed_issue` |
| Title contains "reopened" | `reopened_issue` |
| Label mentioned | `issue_labeled` |
| Body changed | `issue_edited` |

Priority: 1=urgent, 2=normal, 3=low

---

## Step 6: Deduplicate

```bash
jq --arg issue "<N>" --arg type "<event_type>" \
  '[.[] | select(.issue == $issue and .type == $type)] | length' \
  .autoship/event-queue.json
```

Skip if identical event queued in last 5 minutes.

---

## Step 7: Write Events to Queue

```json
{
  "type": "new_issue",
  "issue": "42",
  "priority": 3,
  "data": {
    "source": "discord-webhook",
    "title": "<title>",
    "actor": "<username>",
    "labels": ["enhancement"]
  },
  "queued_at": "2026-04-15T12:00:00Z"
}
```

---

## Step 8: Update Poll Timestamp

```bash
jq --arg ts "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
   '.discord_last_poll = $ts' \
  .autoship/state.json > .autoship/state.tmp && mv .autoship/state.tmp .autoship/state.json
```

---

## Output

```
Discord Webhook [<timestamp>]
  Fetched: N messages, M webhook embeds
  Queued:  N events
  Skipped: N duplicates
```
