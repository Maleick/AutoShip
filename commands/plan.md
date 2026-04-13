---
description: "Analyze open GitHub issues and display dispatch plan without executing"
allowed-tools: ["Bash", "Read", "Write", "Skill", "ToolSearch", "WebFetch"]
---

<autoship-plan>

You are AutoShip's **Sonnet executor**. Run UltraPlan analysis (dry-run — no agents dispatched, no worktrees created).

## Step 1: Prerequisite Checks

```bash
git rev-parse --is-inside-work-tree 2>/dev/null
gh auth status 2>&1
gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null
```

## Step 2: Fetch Open Issues

```bash
gh issue list --state open --json number,title,labels,body --limit 100
```

## Step 3: UltraPlan Analysis

For each issue, classify:

- **Simple** — isolated bug, single file, clear fix, no deps → Codex Spark
- **Medium** — multi-file change, refactor, feature addition → Codex GPT or Sonnet
- **Complex** — architecture change, cross-cutting, unknown scope → Sonnet (with Opus advisor)

Build dependency graph: identify which issues block others (look for "blocks #N", "depends on #N" in body/comments).

## Step 4: Display Plan

```
AUTOSHIP PLAN (dry-run — no agents dispatched)
─────────────────────────────────────────────
Phase 1 (N issues):
  [Simple/Codex]   #12 — Fix login validation
  [Medium/Sonnet]  #18 — Refactor query builder
Phase 2 (N issues):
  [Complex/Sonnet] #22 — Migrate auth subsystem
Dependencies: #18 blocks #22
Estimated quota: ~X% Codex, ~Y% Claude
─────────────────────────────────────────────
```

Do not write state, create worktrees, or dispatch agents.

</autoship-plan>
