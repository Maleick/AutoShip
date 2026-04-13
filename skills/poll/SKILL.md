---
name: poll
description: 10-minute GitHub issue sync — diffs live issue state against .autoship/state.json, integrates new issues into the plan, cancels agents for closed issues, and updates changed issue metadata
tools: ["Bash", "Read", "Write"]
---

# AutoShip Poll Protocol

You are the AutoShip polling safety net. You run every 10 minutes via CronCreate. Your job is to diff live GitHub issue state against `.autoship/state.json` and reconcile any drift.

## Step 1: Load Known State

```bash
cat .autoship/state.json
```

Extract the `issues` map: each key is an issue number (as string), with `state`, `complexity`, `agent`, `pane_id`, and `worktree` fields.

Also note the `repo` field — you'll need it for `gh` calls.

## Step 2: Fetch Live Issues from GitHub

**Error Recovery #5: GitHub API errors** — all `gh` calls in this step must be wrapped with failure handling. Read the consecutive failure counter before fetching:

```bash
# Read current consecutive failure count (default 0 if not set)
api_failures=$(jq -r '.consecutive_api_failures // 0' .autoship/state.json 2>/dev/null) || api_failures=0
```

Fetch open issues:

```bash
POLL_ERROR_TMP=$(mktemp .autoship/poll-error.XXXXXX.tmp)
if live_issues=$(gh issue list --state open --json number,title,body,labels,updatedAt --limit 200 2>"$POLL_ERROR_TMP"); then
  # Reset failure counter on success
  jq '.consecutive_api_failures = 0' .autoship/state.json > .autoship/state.tmp && mv .autoship/state.tmp .autoship/state.json
else
  api_failures=$((api_failures + 1))
  echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] GitHub API error (attempt $api_failures): $(cat "$POLL_ERROR_TMP")" >> .autoship/poll.log
  echo "GitHub API error, will retry on next poll" >&2

  # Update consecutive failure counter in state
  jq --argjson n "$api_failures" '.consecutive_api_failures = $n' .autoship/state.json \
    > .autoship/state.tmp && mv .autoship/state.tmp .autoship/state.json

  # After 3 consecutive failures, write urgent event to queue
  if [[ "$api_failures" -ge 3 ]]; then
    EVENT_QUEUE=".autoship/event-queue.json"
    [[ ! -f "$EVENT_QUEUE" ]] && echo '[]' > "$EVENT_QUEUE"
    jq '. + [{"type": "github_api_down", "consecutive_failures": '"$api_failures"', "priority": 1}]' \
      "$EVENT_QUEUE" > "${EVENT_QUEUE}.tmp" && mv "${EVENT_QUEUE}.tmp" "$EVENT_QUEUE"
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] github_api_down event queued after $api_failures consecutive failures" >> .autoship/poll.log
  fi

  rm -f "$POLL_ERROR_TMP"
  # Skip the rest of this poll cycle — do not crash
  exit 0
fi
rm -f "$POLL_ERROR_TMP"
```

This returns all currently **open** issues. Store the result as `live_issues`.

Also fetch recently closed issues (closed in the past 30 minutes) to catch external closures:

```bash
POLL_CLOSED_TMP=$(mktemp .autoship/poll-error.XXXXXX.tmp)
if ! closed_issues=$(gh issue list --state closed --json number,title,labels,updatedAt,closedAt --limit 50 \
    | jq '[.[] | select(.closedAt > (now - 1800 | todate))]' 2>"$POLL_CLOSED_TMP"); then
  echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] GitHub API warning (closed issues fetch): $(cat "$POLL_CLOSED_TMP")" >> .autoship/poll.log
  closed_issues='[]'  # Non-fatal — continue with empty closed list
fi
rm -f "$POLL_CLOSED_TMP"
```

## Step 3: Diff Against Known State

Build three lists by comparing `live_issues` against the `issues` map in `.autoship/state.json`:

### New Issues

Issues in `live_issues` that are **not** present in `.autoship/state.json`.

```bash
# Use jq for numeric-aware comparison — avoids lexicographic sort bugs (e.g. #100 before #9)
new_issues=$(jq --argjson known "$(jq '[.issues | keys[] | ltrimstr("issue-") | tonumber]' .autoship/state.json)" \
  '[.[] | .number | select(. as $n | $known | index($n) == null)]' <<< "$live_issues")
```

### Closed Issues

Issues present in `.autoship/state.json` with `state` != `merged` or `blocked`, but **absent** from `live_issues` (i.e., closed externally).

```bash
jq -r '.issues | to_entries[] | select(.value.state | test("running|claimed|unclaimed|verifying|approved")) | .key' \
  .autoship/state.json
# Cross-reference against live open issue numbers
```

### Changed Issues

Issues in both `live_issues` and `.autoship/state.json` where `updatedAt` in GitHub is **newer** than the timestamp stored in local state (if tracked), or where labels have changed.

Compare label sets — look for additions or removals of `autoship:*` labels that would indicate out-of-band state changes.

## Step 4: Handle New Issues

For each new issue:

### 4a. Classify Complexity

Use the same classification table as the main orchestrator:

| Signal           | Simple                          | Medium                        | Complex                              |
| ---------------- | ------------------------------- | ----------------------------- | ------------------------------------ |
| Scope            | Single file, < 50 lines changed | 2–5 files, < 200 lines        | 5+ files, architectural changes      |
| Test coverage    | Existing tests cover the change | Needs new test cases          | Needs new test infrastructure        |
| Dependencies     | None                            | Touches shared utilities      | Cross-cutting concerns               |
| Domain knowledge | Obvious fix/feature             | Requires reading related code | Requires understanding system design |
| Description      | Clear steps to implement        | Partial spec, needs inference | Ambiguous, needs decomposition       |

When ambiguous, classify **up** (prefer medium over simple, complex over medium).

### 4b. Check Dependencies

Scan the issue body for:

- Explicit: `blocks: #N`, `depends-on: #N`, `blocked by #N`, `after #N`
- Implicit: references to the same files or components as in-flight issues

### 4c. Add to Plan

Append the new issue to `.autoship/state.json`:

```bash
jq --arg num "<number>" \
   --arg title "<title>" \
   --arg complexity "<simple|medium|complex>" \
   '.issues[$num] = {
     state: "unclaimed",
     complexity: $complexity,
     title: $title,
     agent: null,
     attempt: 0,
     worktree: null,
     pane_id: null,
     discovered_by: "poll",
     started_at: null,
     attempts_history: []
   }' .autoship/state.json > .autoship/state.tmp && mv .autoship/state.tmp .autoship/state.json
```

Determine which phase the issue belongs to (Phase 1 if no unresolved dependencies, otherwise after its blockers). Append to the appropriate phase in `plan.phases`.

## Step 5: Handle Closed Issues

For each issue that was closed externally:

### 5a. Check for Running Agent

Read `.autoship/state.json` to get `pane_id` and `worktree` for the issue.

```bash
pane_id=$(jq -r --arg num "<number>" '.issues[$num].pane_id // empty' .autoship/state.json)
worktree=$(jq -r --arg num "<number>" '.issues[$num].worktree // empty' .autoship/state.json)
```

### 5b. Kill Running Pane (if any)

If `pane_id` is set and the tmux pane is still alive:

```bash
if tmux list-panes -a -F '#{pane_id}' | grep -q "^${pane_id}$"; then
  tmux kill-pane -t "$pane_id"
  echo "Killed pane $pane_id for closed issue #<number>"
fi
```

### 5c. Remove Worktree (if any)

If `worktree` path exists:

```bash
if [ -d "$worktree" ]; then
  git worktree remove "$worktree" --force
  echo "Removed worktree $worktree for closed issue #<number>"
fi
```

### 5d. Update Local State

```bash
jq --arg num "<number>" \
   '.issues[$num].state = "cancelled" |
    .issues[$num].cancelled_reason = "closed-externally"' \
  .autoship/state.json > .autoship/state.tmp && mv .autoship/state.tmp .autoship/state.json
```

Remove the `autoship:in-progress` label from the issue if present:

```bash
gh issue edit <number> --remove-label "autoship:in-progress" 2>/dev/null || true
```

## Step 6: Handle Changed Issues

For each changed issue (label changes or body updates):

### Label Reconciliation

If a `autoship:*` label was **added** externally:

- `autoship:blocked` → update local state to `blocked`
- `autoship:in-progress` → log discrepancy (we should already know about it)

If a `autoship:*` label was **removed** externally:

- `autoship:in-progress` removed → treat as cancellation signal, proceed as Step 5

### Metadata Update

Update title, body, and labels in `.autoship/state.json`:

```bash
jq --arg num "<number>" \
   --arg title "<new_title>" \
   --argjson labels '<labels_array>' \
   '.issues[$num].title = $title |
    .issues[$num].labels = $labels |
    .issues[$num].last_synced = now | todate' \
  .autoship/state.json > .autoship/state.tmp && mv .autoship/state.tmp .autoship/state.json
```

## Step 7: Refresh Quota Estimates

Run the daily refresh check — this auto-resets any tool whose quota crossed midnight (subscription tools renew daily):

```bash
bash "$(cat .autoship/hooks_dir)/quota-update.sh" refresh
```

This is a no-op if all tools were already reset today. No API calls made.

## Step 8: Update Poll Timestamp and Write Log

Update `updated_at` and `last_poll` in state:

```bash
jq --arg ts "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
   '.updated_at = $ts | .last_poll = $ts' \
  .autoship/state.json > .autoship/state.tmp && mv .autoship/state.tmp .autoship/state.json
```

Write a summary to `.autoship/poll.log`:

```bash
cat >> .autoship/poll.log <<EOF
[$(date -u +%Y-%m-%dT%H:%M:%SZ)] Poll complete
  New issues:    <count> (<numbers>)
  Closed issues: <count> (<numbers>)
  Changed issues: <count> (<numbers>)
  Active agents:  <count>
  State file:     .autoship/state.json
EOF
```

Keep the log bounded — trim to the last 500 lines:

```bash
tail -500 .autoship/poll.log > .autoship/poll.log.tmp && mv .autoship/poll.log.tmp .autoship/poll.log
```

## Output

After completing all steps, print a brief summary:

```
AutoShip Poll [<timestamp>]
  New:     <N> issues added to plan
  Closed:  <N> agents cancelled
  Changed: <N> state updates
  No action needed for <N> issues
```

If nothing changed, print:

```
AutoShip Poll [<timestamp>] — no changes detected
```
