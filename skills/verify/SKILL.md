---
name: verify
description: OpenCode-only post-completion verification pipeline
tools: ["Bash", "Agent", "Read", "Write"]
---

# AutoShip Verification Pipeline

Invoked after an OpenCode worker reports `COMPLETE`.

## Guards

- `AUTOSHIP_RESULT.md` must exist inside the workspace.
- The workspace must have committed changes or uncommitted changes that are intentionally staged before PR creation.
- The diff must be non-empty.
- Test command discovery should use project files or `.autoship/config.json`.

## Review

Use the configured OpenCode reviewer model from `.autoship/model-routing.json` when a review worker is needed. Pass issue title, body, acceptance criteria, result path, diff command, and test command.

## PR Creation

Use conventional PR titles:

```bash
TITLE=$(bash hooks/opencode/pr-title.sh --issue <number>)
gh pr create --title "$TITLE" --body-file AUTOSHIP_PR_BODY.md --label autoship
```

## Failure Handling

- First failure: re-dispatch with failure context.
- Second failure: retry with a stronger configured OpenCode model if one is selected.
- Repeated or ambiguous failure: mark blocked for human review.

## Cleanup

After merge, remove the worktree and mark state merged with `hooks/update-state.sh`.
