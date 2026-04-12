---
name: beacon-poll
description: 10-minute GitHub issue sync — diffs live issue state against .beacon/state.json, integrates new issues into the plan, cancels agents for closed issues, and updates changed issue metadata
tools: ["Bash", "Read", "Write"]
---

# Beacon Poll Protocol

You are the Beacon polling safety net. You run every 10 minutes via CronCreate. Your job is to diff live GitHub issue state against `.beacon/state.json` and reconcile any drift.

## Step 1: Load Known State

```bash
cat .beacon/state.json
```

Extract the `issues` map: each key is an issue number (as string), with `state`, `complexity`, `agent`, `pane_id`, and `worktree` fields.

Also note the `repo` field — you'll need it for `gh` calls.

## Step 2: Fetch Live Issues from GitHub

```bash
gh issue list --state open --json number,title,body,labels,updatedAt --limit 200
```

This returns all currently **open** issues. Store the result as `live_issues`.

Also fetch recently closed issues (closed in the past 30 minutes) to catch external closures:

```bash
gh issue list --state closed --json number,title,labels,updatedAt --limit 50 \
  | jq '[.[] | select(.updatedAt > (now - 1800 | todate))]'
```

## Step 3: Diff Against Known State

Build three lists by comparing `live_issues` against the `issues` map in `.beacon/state.json`:

### New Issues

Issues in `live_issues` that are **not** present in `.beacon/state.json`.

```bash
# Pseudocode — implement with jq
known_numbers=$(jq -r '.issues | keys[]' .beacon/state.json)
live_numbers=$(echo "$live_issues" | jq -r '.[].number')
new_issues=$(comm -13 <(echo "$known_numbers" | sort) <(echo "$live_numbers" | sort))
```

### Closed Issues

Issues present in `.beacon/state.json` with `state` != `merged` or `blocked`, but **absent** from `live_issues` (i.e., closed externally).

```bash
jq -r '.issues | to_entries[] | select(.value.state | test("running|claimed|unclaimed|verifying|approved")) | .key' \
  .beacon/state.json
# Cross-reference against live open issue numbers
```

### Changed Issues

Issues in both `live_issues` and `.beacon/state.json` where `updatedAt` in GitHub is **newer** than the timestamp stored in local state (if tracked), or where labels have changed.

Compare label sets — look for additions or removals of `beacon:*` labels that would indicate out-of-band state changes.

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

Append the new issue to `.beacon/state.json`:

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
   }' .beacon/state.json > .beacon/state.tmp && mv .beacon/state.tmp .beacon/state.json
```

Determine which phase the issue belongs to (Phase 1 if no unresolved dependencies, otherwise after its blockers). Append to the appropriate phase in `plan.phases`.

## Step 5: Handle Closed Issues

For each issue that was closed externally:

### 5a. Check for Running Agent

Read `.beacon/state.json` to get `pane_id` and `worktree` for the issue.

```bash
pane_id=$(jq -r --arg num "<number>" '.issues[$num].pane_id // empty' .beacon/state.json)
worktree=$(jq -r --arg num "<number>" '.issues[$num].worktree // empty' .beacon/state.json)
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
  .beacon/state.json > .beacon/state.tmp && mv .beacon/state.tmp .beacon/state.json
```

Remove the `beacon:in-progress` label from the issue if present:

```bash
gh issue edit <number> --remove-label "beacon:in-progress" 2>/dev/null || true
```

## Step 6: Handle Changed Issues

For each changed issue (label changes or body updates):

### Label Reconciliation

If a `beacon:*` label was **added** externally:
- `beacon:blocked` → update local state to `blocked`
- `beacon:in-progress` → log discrepancy (we should already know about it)

If a `beacon:*` label was **removed** externally:
- `beacon:in-progress` removed → treat as cancellation signal, proceed as Step 5

### Metadata Update

Update title, body, and labels in `.beacon/state.json`:

```bash
jq --arg num "<number>" \
   --arg title "<new_title>" \
   --argjson labels '<labels_array>' \
   '.issues[$num].title = $title |
    .issues[$num].labels = $labels |
    .issues[$num].last_synced = now | todate' \
  .beacon/state.json > .beacon/state.tmp && mv .beacon/state.tmp .beacon/state.json
```

## Step 7: Update Poll Timestamp and Write Log

Update `updated_at` and `last_poll` in state:

```bash
jq --arg ts "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
   '.updated_at = $ts | .last_poll = $ts' \
  .beacon/state.json > .beacon/state.tmp && mv .beacon/state.tmp .beacon/state.json
```

Write a summary to `.beacon/poll.log`:

```bash
cat >> .beacon/poll.log <<EOF
[$(date -u +%Y-%m-%dT%H:%M:%SZ)] Poll complete
  New issues:    <count> (<numbers>)
  Closed issues: <count> (<numbers>)
  Changed issues: <count> (<numbers>)
  Active agents:  <count>
  State file:     .beacon/state.json
EOF
```

Keep the log bounded — trim to the last 500 lines:

```bash
tail -500 .beacon/poll.log > .beacon/poll.log.tmp && mv .beacon/poll.log.tmp .beacon/poll.log
```

## Output

After completing all steps, print a brief summary:

```
Beacon Poll [<timestamp>]
  New:     <N> issues added to plan
  Closed:  <N> agents cancelled
  Changed: <N> state updates
  No action needed for <N> issues
```

If nothing changed, print:

```
Beacon Poll [<timestamp>] — no changes detected
```
