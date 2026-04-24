---
name: autoship-start
description: Launch AutoShip autonomous orchestration
platform: opencode
---

# /autoship — Start Orchestration

## Prerequisite Checks

```bash
git rev-parse --is-inside-work-tree 2>/dev/null
gh auth status 2>&1
gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null
```

Fail messages:
- Not in git repo: `"Error: Not inside a git repository."`
- gh not authed: `"Error: GitHub CLI not authenticated. Run 'gh auth login'."`
- No remote: `"Error: No GitHub remote detected."`

## Probe Available Tools

```bash
command -v opencode >/dev/null && echo "OpenCode available" || echo "OpenCode not found"
```

## Initialize State

```bash
./hooks/init.sh
```

## Invoke Orchestration

Run the `autoship-orchestrate` skill:

1. Fetch open issues
2. Classify each by complexity
3. Build dispatch plan
4. Start monitoring loop
5. Dispatch or queue agents up to the configured concurrency cap, default 15

## Monitoring

Agents write status to `.autoship/workspaces/<issue-key>/status`. The orchestrator polls for COMPLETE/BLOCKED/STUCK and runs the verification pipeline.

If dispatch queued workspaces instead of starting workers directly, run:

```bash
bash hooks/opencode/runner.sh
```
