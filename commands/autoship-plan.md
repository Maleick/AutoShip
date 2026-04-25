---
name: autoship-plan
description: Dry-run AutoShip — analyze issues and show dispatch plan without executing
platform: opencode
---

# /autoship-plan — Dry Run

Analyze open GitHub issues and show the dispatch plan without executing.

## Fetch Open Issues

```bash
gh issue list --state open --json number,title,body,labels --limit 200 | jq 'sort_by(.number)'
```

## Classify Each Issue

```bash
bash hooks/opencode/plan-issues.sh --limit 10
```

## Build Plan

For each issue:
1. Complexity (simple/medium/complex)
2. Assigned tool based on routing matrix
3. Dependencies (blocks: #N, depends-on: #N)

## Display Plan

```
═══════════════════════════════════════════
           AUTOSHIP DISPATCH PLAN
═══════════════════════════════════════════

Phase 1 (3 issues)
───────────────────────────────────────────
  #42  simple_code    → OpenCode free-first
  #45  medium_code     → OpenCode free-first
  #48  docs           → OpenCode free-first

Phase 2 (2 issues)
───────────────────────────────────────────
  #51  complex         → OpenCode capable model
  #53  ci_fix          → OpenCode free-first

Blocked (1 issue)
───────────────────────────────────────────
  #39  blocked by #42

═══════════════════════════════════════════
Quota Estimate
───────────────────────────────────────────
  OpenCode: provider-managed

Total: 6 issues across 2 phases
═══════════════════════════════════════════
```

## No Execution

This command only reads and analyzes. No agents are dispatched, no changes made.
Eligible issues are ordered by ascending issue number. Issues already marked running, blocked, or human-required are excluded from the dispatch plan.
