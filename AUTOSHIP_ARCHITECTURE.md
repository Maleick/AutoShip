# AutoShip v3 Architecture — Advisor + Monitor Pattern

Status: **Locked** — Design decisions finalized
Updated: 2026-04-12

---

## 1. Design Philosophy

AutoShip v3 inverts the orchestration model. Instead of Opus running a constant loop and spawning workers, **Sonnet is the executor** — it reacts to events, handles dispatch, and runs the pipeline. **Opus is the advisor** — called only at strategic decision points where higher intelligence changes the outcome.

This mirrors Anthropic's Advisor tool pattern (API beta `advisor-tool-2026-03-01`), adapted for the Claude Code plugin runtime using native Agent spawning and the Monitor tool.

### Core Principles

1. **Event-driven, not poll-driven** — Monitor tools stream events into the conversation. No CronCreate polling loops.
2. **Sonnet-first execution** — The bulk of orchestration runs at Sonnet-level. Mechanical work doesn't need Opus.
3. **Opus for strategy only** — UltraPlan, re-dispatch decisions, ambiguous verdicts, phase checkpoints.
4. **Fresh context per advisor call** — Opus gets spawned with focused context, not accumulated noise.

### Four-Tier Model

Each tier operates in its optimal domain:

| Tier       | Role         | Responsibility                                      |
| ---------- | ------------ | --------------------------------------------------- |
| **Bash**   | Watcher      | Raw event polling, tmux pane tailing, file watching |
| **Haiku**  | Thinker      | Event interpretation, triage, simple task execution |
| **Sonnet** | Orchestrator | Pipeline execution, dispatch, complex work          |
| **Opus**   | Strategist   | UltraPlan, phase checkpoints, escalation decisions  |

---

## 2. Model Roles

| Model      | Role                    | When Active                                                                                |
| ---------- | ----------------------- | ------------------------------------------------------------------------------------------ |
| **Opus**   | Advisor                 | Spawned at decision points: UltraPlan, re-dispatch, checkpoints, LOW_CONFIDENCE reviews    |
| **Sonnet** | Executor / Orchestrator | Main loop — reacts to Monitor events, dispatches agents, runs verify pipeline, creates PRs |
| **Sonnet** | Worker (medium/complex) | Dispatched to worktrees for medium and complex issues                                      |
| **Haiku**  | Worker (simple)         | Dispatched to worktrees for simple issues (2-3 files, clear acceptance criteria)           |
| **Haiku**  | Event Triage            | Interprets Monitor events, categorizes PR comments, queues actions for Sonnet              |
| **Sonnet** | Reviewer                | Verification agent — structured I/O, test discovery, verdicts                              |

### 2.1 Task Routing: Haiku vs Sonnet

**Decision: Haiku handles simple tasks with clear acceptance criteria** (locked)

- Haiku assigned to tasks classified as "simple" — 2-3 files with straightforward logic
- Prompt quality matters more than model size for well-scoped tasks
- Dispatch already generates detailed prompts with AUTOSHIP_RESULT.md templates and acceptance criteria
- Repetitive medium-complexity cross-file work excluded from Haiku to avoid subtle mistakes

### 2.2 Haiku Failure Escalation

**Decision: One retry, then escalate to Sonnet** (locked)

```
Haiku attempt 1 → FAIL → retry with failure context appended to prompt
Haiku attempt 2 → FAIL → promote task to Sonnet (no 3rd Haiku attempt)
```

- First failure often a prompt clarity issue, not capability limit
- Second failure indicates task exceeds model capability — stop burning cycles
- Escalation is automatic, no Opus consultation needed

### 2.3 Dispatch Priority: Third-Party Tools First

**Decision: Third-party tools for simple/medium, Claude for complex** (locked)

| Complexity | Primary Tool                     | Fallback             |
| ---------- | -------------------------------- | -------------------- |
| Simple     | Codex/Gemini/Grok (if available) | Haiku                |
| Medium     | Codex/Gemini/Grok (if available) | Sonnet               |
| Complex    | Sonnet (with autoresearch)       | Sonnet + Opus review |

- Third-party tools burn their own quota — maximize usage on volume work
- Third-party tools lack autoresearch, plugins, and native verify pipeline
- Complex tasks need full Claude ecosystem — retry costs outweigh savings from cheaper tools
- Dispatch respects `quota_pct` from detect-tools.sh; exhausted tools are skipped

### Advisor Call Points

**Decision: Hybrid triggers — hardcoded + Sonnet-initiated escalation** (locked)

Opus is spawned (via `Agent` with `model: "opus"`) at these hardcoded moments:

1. **UltraPlan** — After issues are fetched, Opus classifies complexity, builds dependency graph, assigns tools, plans phases
2. **Re-dispatch decision** — Agent failed verification 2+ times on same issue
3. **Phase checkpoint** — A dispatch phase completed. Opus reviews results and adjusts the plan
4. **LOW_CONFIDENCE verdict** — Sonnet reviewer returned LOW_CONFIDENCE. Opus makes the final call
5. **Stuck detection** — Same issue failed 2+ times. Opus decides: block it, re-approach, or escalate to human

Additionally, Sonnet can escalate to Opus at any time for:

- Conflicting acceptance criteria
- Ambiguous issue scope
- Unexpected tool behavior or edge cases
- PR conflicts requiring architectural judgment

### Advisor Call Format

Each Opus advisor call gets a focused prompt:

```
You are AutoShip's strategic advisor. Review the current state and provide a decision.

## Context
<current state summary from .autoship/state.json>
<specific decision needed>

## Options
<enumerated choices with tradeoffs>

## Constraints
<quota status, time pressure, dependency state>

Respond with: your decision, reasoning (1-2 sentences), and any plan adjustments.
Keep response under 200 words.
```

---

## 3. Event Architecture

### Three-Monitor Design

**Decision: Three separate monitors with tuned poll intervals** (locked)

| Monitor          | Poll Interval | Domain                    | Rationale                                    |
| ---------------- | ------------- | ------------------------- | -------------------------------------------- |
| Agent Completion | 5 seconds     | tmux pane output          | Fast verification start after agent finishes |
| PR Status        | 30 seconds    | CI checks, merge status   | CI takes minutes; faster polling wastes API  |
| GitHub Issues    | 60 seconds    | New/closed/changed issues | External events; lowest urgency              |

A unified monitor would either waste GitHub API calls (polling everything at 5s) or delay agent detection (polling at 60s). Separate monitors allow independent tuning and debugging.

### Monitor Integration: Bash Watches, Haiku Thinks

**Decision: Bash scripts poll, Haiku interprets events** (locked)

```
Bash (Monitor tool) → streams raw events → Haiku interprets meaning → queues action for Sonnet
```

- Bash excels at fast, reliable polling without token consumption
- Haiku interprets what events mean and decides next steps
- Sonnet pulls from queue after completing each pipeline step

### Agent Completion Detection

**Decision: Real-time pane output monitoring with status words** (locked)

Instead of polling `pane_dead`, agents emit a final status word that Monitor detects instantly:

```bash
# tmux pipe-pane captures agent output to log file
tmux pipe-pane -t "$PANE_ID" "cat >> .autoship/workspaces/$KEY/pane.log"

# Monitor tails the log for status keywords
tail -f .autoship/workspaces/$KEY/pane.log | grep --line-buffered -E "^(COMPLETE|BLOCKED|STUCK)$"
```

**Status vocabulary** (three words):

| Status     | Meaning                                   | Next Action                   |
| ---------- | ----------------------------------------- | ----------------------------- |
| `COMPLETE` | Agent finished, AUTOSHIP_RESULT.md written  | Run verify pipeline           |
| `BLOCKED`  | External dependency or permission issue   | Mark blocked, notify operator |
| `STUCK`    | Agent attempted but cannot solve the task | Re-dispatch or escalate       |

The reviewer handles pass/fail granularity — agents only signal their own outcome assessment.

### Third-Party Agent Completion

**Decision: pane_dead + AUTOSHIP_RESULT.md existence check** (locked)

Claude agents use TeamCreate with native completion signaling. Third-party tools (Codex/Gemini/Grok) run in tmux panes:

- `pane_dead=1` + AUTOSHIP_RESULT.md exists → agent completed, run verify
- `pane_dead=1` + no AUTOSHIP_RESULT.md → crash, flag for re-dispatch
- Exit codes from third-party CLIs are unreliable (may exit 0 on failure)

### Event Queue: Haiku Queues, Sonnet Pulls

**Decision: Producer-consumer pattern** (locked)

```
Haiku (producer) → interprets raw events → writes to .autoship/event-queue.json
Sonnet (consumer) → pulls next event after completing current pipeline step
```

- Haiku controls event generation and queuing
- Sonnet controls its own processing rate
- Pull-based model prevents pipeline overload during event storms

### Monitor Scripts

#### Monitor 1: Agent Completion Watcher (5s)

```bash
# Watch for agent status words via tmux pipe-pane logs
for logfile in .autoship/workspaces/*/pane.log; do
  tail -f "$logfile" | grep --line-buffered -E "^(COMPLETE|BLOCKED|STUCK)$" | \
    while read -r status; do
      key=$(basename "$(dirname "$logfile")")
      echo "[AGENT_STATUS] key=$key status=$status"
    done &
done
wait
```

#### Monitor 2: GitHub Issue Watcher (60s)

```bash
last_check=$(date -u +%Y-%m-%dT%H:%M:%SZ)
while true; do
  now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  new=$(gh api "repos/OWNER/REPO/issues?state=open&since=$last_check&sort=created" --jq '.[].number' 2>/dev/null)
  for num in $new; do
    echo "[ISSUE_NEW] number=$num"
  done
  closed=$(gh api "repos/OWNER/REPO/issues?state=closed&since=$last_check&sort=updated" --jq '.[].number' 2>/dev/null)
  for num in $closed; do
    echo "[ISSUE_CLOSED] number=$num"
  done
  last_check=$now
  sleep 60
done
```

#### Monitor 3: PR Status Watcher (30s)

```bash
while true; do
  gh pr list --label autoship --state open --json number,mergeable,statusCheckRollup 2>/dev/null | \
    jq -r '.[] | "\(.number) \(.mergeable) \(.statusCheckRollup // [] | map(.conclusion) | join(","))"' | \
    while read -r num mergeable checks; do
      if echo "$checks" | grep -q "SUCCESS"; then
        echo "[PR_CI_PASS] number=$num"
      elif echo "$checks" | grep -q "FAILURE"; then
        echo "[PR_CI_FAIL] number=$num"
      fi
      if [[ "$mergeable" == "CONFLICTING" ]]; then
        echo "[PR_CONFLICT] number=$num"
      fi
    done
  gh pr list --label autoship --state merged --json number,mergedAt 2>/dev/null | \
    jq -r '.[] | "[PR_MERGED] number=\(.number)"'
  sleep 30
done
```

### Event Processing Flow

Events arrive as Monitor notifications. Sonnet processes them sequentially:

```
1. Bash Monitor fires: [AGENT_STATUS] key=issue-25 status=COMPLETE
2. Haiku interprets: "Agent finished issue-25 successfully"
3. Haiku queues: {type: "verify", issue: "issue-25", priority: 1}
4. Sonnet pulls from queue → reads AUTOSHIP_RESULT.md → spawns reviewer
5. Reviewer returns PASS → Sonnet creates PR
6. Sonnet pulls next event from queue
```

For strategic decisions, Sonnet spawns Opus:

```
1. [AGENT_STATUS] key=issue-25 status=STUCK (2nd attempt)
2. Sonnet spawns Opus advisor: "Issue #25 failed twice. Codex and Haiku both failed."
3. Opus returns: "Re-slice. Split into #25a (validation) and #25b (error handling)."
4. Sonnet creates sub-issues and dispatches
```

---

## 4. PR Review Comment Triage

**Decision: Haiku categorizes, tiered resolution** (locked)

| Comment Type | Handler | Examples                                 |
| ------------ | ------- | ---------------------------------------- |
| **Nit**      | Haiku   | Naming, formatting, unused imports       |
| **Bug**      | Sonnet  | Missing edge cases, wrong algorithms     |
| **Design**   | Opus    | Refactoring decisions, reviewer pushback |

- Haiku reads all PR review comments and categorizes as nit/bug/design
- Haiku auto-fixes nits in seconds (most automated reviewer comments from Copilot/CodeRabbit are nits)
- Sonnet handles bug/logic comments requiring code understanding
- Opus reviews design issues requiring architectural judgment

---

## 5. CI Autofix Loop

**Decision: Tiered agent selection by error type** (locked)

```
PR Monitor detects CI failure
  → Classify error type
  → Route to appropriate agent tier
  → Agent fixes → pushes → CI re-triggers
  → If fails 2+ times on same PR → Opus advisor
```

| Error Type          | Agent  | Examples                                |
| ------------------- | ------ | --------------------------------------- |
| Mechanical          | Haiku  | Lint errors, format errors, type errors |
| Logic               | Sonnet | Test failures, build errors             |
| Repeated (2+ fails) | Opus   | Re-approach decision or block PR        |

---

## 6. Startup Sequence

```
1. Sonnet validates environment (gh, tmux, git repo)
2. Sonnet detects available tools + checks quota (hooks/detect-tools.sh)
3. Sonnet loads .autoship/state.json or runs hooks/init.sh
4. Sonnet fetches open issues via gh
5. → ADVISOR CALL: Opus runs UltraPlan
   - Classifies complexity per issue
   - Builds dependency graph
   - Assigns tools based on complexity + quota
   - Plans dispatch phases
   - Returns: structured plan JSON
6. Sonnet stores plan in .autoship/state.json
7. Sonnet starts 3 persistent Monitors (agent, issue, PR watchers)
8. Sonnet dispatches Phase 1 agents (third-party tools first)
9. Sonnet enters reactive mode — pulls from event queue
```

---

## 7. Dispatch Protocol

Sonnet handles all dispatch. Third-party tools dispatched first when available.

### Simple Issues → Third-Party or Haiku

```
# If Codex/Gemini available with quota:
tmux send-keys -t autoship:<pane> "codex --prompt-file .autoship/workspaces/$KEY/prompt.md" Enter

# Fallback to Haiku:
Agent({
  model: "haiku",
  prompt: "<autoship worker prompt>",
  mode: "auto"
})
```

### Medium Issues → Third-Party or Sonnet

```
# If Codex/Gemini available with quota:
tmux send-keys -t autoship:<pane> "codex --prompt-file .autoship/workspaces/$KEY/prompt.md" Enter

# Fallback to Sonnet:
Agent({
  model: "sonnet",
  prompt: "<autoship worker prompt with autoresearch>",
  mode: "auto"
})
```

### Complex Issues → Sonnet Worker (always Claude)

```
Agent({
  model: "sonnet",
  prompt: "<autoship worker prompt with autoresearch + note: this is complex>",
  mode: "auto"
})
// After completion, Opus reviews the work before proceeding to PR
```

---

## 8. Comparison: v2 vs v3

| Aspect              | v2 (Current)                  | v3 (Advisor + Monitor)                          |
| ------------------- | ----------------------------- | ----------------------------------------------- |
| Orchestrator        | Opus (always on)              | Sonnet (event-driven)                           |
| Strategic decisions | Opus makes every decision     | Opus called at hardcoded triggers + escalation  |
| Event detection     | CronCreate poll (10 min)      | 3 Monitors (5s/30s/60s, tuned per domain)       |
| Worker dispatch     | Opus → Sonnet/Haiku           | Sonnet → Third-party/Haiku/Sonnet               |
| Tool priority       | Claude first                  | Third-party first (burn external quota)         |
| Event processing    | Sequential in Opus context    | Haiku queues → Sonnet pulls                     |
| Agent detection     | pane_dead polling             | Real-time status words via pipe-pane            |
| PR comments         | Sonnet handles all            | Haiku triages → tiered resolution               |
| CI failures         | Manual                        | Auto-fix loop (Haiku/Sonnet/Opus by error type) |
| Context usage       | Opus accumulates full session | Opus gets fresh, focused context per call       |
| Cost model (API)    | Opus rates for everything     | Sonnet rates + Opus for ~5 strategic calls      |
| Cost model (Max)    | Flat rate                     | Flat rate (but better context efficiency)       |

---

## 9. Locked Design Decisions

Summary of all finalized architecture decisions:

| #   | Decision                    | Choice                                                   |
| --- | --------------------------- | -------------------------------------------------------- |
| 1   | Haiku task scope            | Simple tasks only (2-3 files, clear acceptance criteria) |
| 2   | Haiku failure escalation    | 1 retry with context, then promote to Sonnet             |
| 3   | Monitor architecture        | 3 separate monitors (5s/30s/60s intervals)               |
| 4   | Dispatch priority           | Third-party first for simple/medium, Claude for complex  |
| 5   | Agent completion detection  | Real-time status words (COMPLETE/BLOCKED/STUCK)          |
| 6   | Third-party completion      | pane_dead + AUTOSHIP_RESULT.md existence check             |
| 7   | Haiku + Monitor integration | Bash watches, Haiku interprets, Sonnet orchestrates      |
| 8   | PR comment triage           | Haiku categorizes → nits/bugs/design → tiered resolution |
| 9   | Event queue pattern         | Haiku queues events, Sonnet pulls after pipeline step    |
| 10  | Opus trigger strategy       | Hybrid: hardcoded triggers + Sonnet-initiated escalation |
| 11  | CI autofix loop             | Tiered: Haiku (mechanical) → Sonnet (logic) → Opus (2+)  |

---

## 10. Open Design Questions

- [ ] Should the main executor skill be a rewrite or a wrapper around v2?
- [ ] Opus advisor call budget: hard cap per session? Per phase?
- [ ] Event queue persistence format (JSON file vs in-memory)
