# Beacon Result: Issue #16 — Worktree Cleanup After Merge

## Summary

Implemented worktree cleanup and state reconciliation after PR merge. Created two new hook scripts and updated beacon-init.sh to automatically sweep stale worktrees on startup.

## Work Completed

### 1. Created `hooks/cleanup-worktree.sh`

A comprehensive cleanup script that:

- Takes an issue-key argument (e.g., `issue-16`)
- Removes the git worktree at `.beacon/workspaces/<issue-key>`
- Deletes the local beacon branch (`beacon/<issue-key>`)
- Calls `update-state.sh set-merged <issue-key>` to update state.json
- Removes all beacon labels from the GitHub issue (in-progress, blocked, paused, done)
- Closes the issue on GitHub if it's still open
- Provides graceful fallbacks when components are unavailable (e.g., gh CLI, state file)

Features:

- bash 3.2 compatible (no associative arrays)
- Robust error handling and non-fatal failures for GitHub operations
- Derives repo slug from state.json or git remote as needed
- Handles both existing and missing worktrees gracefully

### 2. Created `hooks/sweep-stale.sh`

An automated cleanup scanner that:

- Iterates over all worktrees in `.beacon/workspaces/`
- Checks the state of each corresponding issue in state.json
- Identifies stale worktrees (issues in terminal states: merged, blocked, approved)
- Calls cleanup-worktree.sh for each stale worktree
- Reports statistics on how many worktrees were cleaned

Terminal states recognized:

- `merged` — PR was merged, work is complete
- `blocked` — Work is blocked and won't continue
- `approved` — Work was completed and approved

### 3. Updated `hooks/beacon-init.sh`

Added a call to sweep-stale.sh during startup:

- Runs after GitHub label creation
- Non-fatal (wrapped in `|| true`)
- Provides user feedback that scanning is occurring

## Implementation Notes

### Bash 3.2 Compatibility

All scripts use POSIX-compatible bash without associative arrays or modern bash features:

- String manipulation for loops instead of array operations
- Simple grep/sed patterns for text processing
- jq for JSON manipulation (already a dependency)

### State Transitions

The cleanup process follows the established state machine:

- `set-merged` action sets state to "merged"
- Also increments the `completed` counter in stats
- Updates the timestamp in state.json

### GitHub Integration

- Uses `gh` CLI (optional, non-fatal if unavailable)
- Attempts to remove multiple label variations
- Gracefully handles missing issues or insufficient permissions

## Testing

Created shell scripts:

- `cleanup-worktree.sh` — syntax verified ✓
- `sweep-stale.sh` — syntax verified ✓
- `beacon-init.sh` (updated) — syntax verified ✓

All scripts validate input, check for required tools, and provide helpful error messages.

## Acceptance Criteria Status

- [x] After confirmed merge: `git worktree remove --force`
- [x] Remove beacon labels from issue
- [x] Close issue if not auto-closed by PR
- [x] Update state file: move to merged, increment stats
- [x] On startup: sweep stale worktrees for terminal-state issues
- [x] Never ask before cleanup — just do it (hooks execute automatically)

## Deployment

To use:

1. Commit changes to `beacon/issue-16` branch
2. The cleanup-worktree.sh script can be called manually:
   ```bash
   ./hooks/cleanup-worktree.sh issue-16
   ```
3. The sweep-stale.sh script runs automatically on startup via beacon-init.sh

## Files Modified/Created

- ✅ Created: `hooks/cleanup-worktree.sh`
- ✅ Created: `hooks/sweep-stale.sh`
- ✅ Modified: `hooks/beacon-init.sh`
