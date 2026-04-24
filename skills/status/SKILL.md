---
name: status
description: Display current AutoShip OpenCode orchestration status
tools: ["Bash", "Read"]
---

# AutoShip Status

Use the real OpenCode status hook:

```bash
bash hooks/opencode/status.sh
```

The hook summarizes configured concurrency, queued/running/completed/blocked/stuck issues, workspace status files, and queue depth.

If `.autoship/state.json` is missing, report that no active AutoShip session exists and tell the operator to run `/autoship`.
