---
description: "AutoShip help — show available commands"
allowed-tools: ["Bash"]
---

<autoship-help>

Display this help text:

```
AutoShip — Autonomous Multi-Agent Orchestrator

Usage: /autoship:<command>

Commands:
  /autoship:start    Launch the orchestration loop. Fetches open issues, classifies
                     complexity, dispatches agents (Claude/Codex/Gemini), reviews
                     results, and opens PRs.
  /autoship:status   Show running agents, issues in progress, tool quotas, and
                     completed count.
  /autoship:stop     Gracefully stop all running agents and save state to
                     .beacon/state.json.
  /autoship:plan     Analyze open issues (UltraPlan) and display the dispatch plan
                     without executing. Use this for dry-run previews.
  /autoship:autoship Show this help text.

Requirements: gh (authenticated), tmux, git repo
```

</autoship-help>
