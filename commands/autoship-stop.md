---
name: autoship-stop
description: Gracefully stop AutoShip orchestration
platform: opencode
---

# /autoship-stop — Stop Orchestration

## Graceful Shutdown

1. Stop dispatching new agents
2. Let running agents complete (or kill if requested)
3. Save state
4. Add `autoship:paused` labels to in-progress issues

## Process

```bash
# Check for running agents
RUNNING=$(jq '[.issues | to_entries[] | select(.value.state == "running")] | length' .autoship/state.json)

if (( RUNNING > 0 )); then
  echo "Stopping dispatch loop..."
fi

# Mark as paused
jq '.paused = true' .autoship/state.json > .autoship/state.tmp && mv .autoship/state.tmp .autoship/state.json

# Add paused labels to GitHub
for issue in $(jq -r '.issues | to_entries[] | select(.value.state == "running") | .key' .autoship/state.json); do
  num="${issue#issue-}"
  gh issue edit "$num" --add-label "autoship:paused" 2>/dev/null || true
done

echo "AutoShip paused. $RUNNING agent(s) completed or running."
echo "Run /autoship to resume."
```

## Options

- **Soft stop**: Stop new dispatches, let running agents finish
- **Hard stop**: Kill all running agents immediately

## Resume

```bash
jq '.paused = false' .autoship/state.json > .autoship/state.tmp && mv .autoship/state.tmp .autoship/state.json
# Then run /autoship
```
