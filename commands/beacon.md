---
description: "Beacon help — show available commands"
allowed-tools: ["Bash"]
---

<beacon-help>

Display this help text:

```
Beacon — Autonomous Multi-Agent Orchestrator

Usage: /beacon:<command>

Commands:
  /beacon:start    Launch the orchestration loop. Fetches open issues, classifies
                   complexity, dispatches agents (Claude/Codex/Gemini), reviews
                   results, and opens PRs.
  /beacon:status   Show running agents, issues in progress, tool quotas, and
                   completed count.
  /beacon:stop     Gracefully stop all running agents and save state to
                   .beacon/state.json.
  /beacon:plan     Analyze open issues (UltraPlan) and display the dispatch plan
                   without executing. Use this for dry-run previews.
  /beacon:beacon   Show this help text.

Requirements: gh (authenticated), tmux, git repo
```

</beacon-help>
