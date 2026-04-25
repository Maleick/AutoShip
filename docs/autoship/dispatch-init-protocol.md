# AutoShip Dispatch Initialization Protocol

## Issue

- **#2468** — autoship: init.sh/dispatch must fetch + base worktrees on origin/master, not stale local master

## Problem

When dispatch creates worker worktrees, using local `master` as the base can inherit stale code if the local branch hasn't been updated. This causes:

- Workers to implement against outdated code
- Stale files (e.g., `AUTOSHIP_RESULT.md`) to contaminate verify output
- Merge conflicts at PR time

## Solution: init.sh

The `init.sh` script provides the correct initialization pattern. Dispatch **must** call it instead of directly running `git worktree add ... master`.

### Usage

```bash
./init.sh <WORKER_KEY>
```

### What init.sh Does (Step 1 of Dispatch)

1. **Fetch fresh state**: `git fetch origin master`
   - Ensures local repo is aware of latest upstream changes
2. **Warn if stale**: Compares local master vs origin/master
   - Alerts operators if local master is behind
3. **Create on origin/master**: `git worktree add ... -b autoship/$KEY origin/master`
   - **NOT** `master` (local ref, may be stale)
   - **NOT** `HEAD` (current detached state)
   - **YES** `origin/master` (guaranteed fresh, authoritative)

### Integration

Dispatch protocol **Step 1** must invoke:

```bash
./init.sh <worker-key>
```

Before any other worker initialization or assignment steps.

### Example

```bash
# Create worker for codex-appserver task
./init.sh codex-appserver-001

# Worktree created at .autoship/workspaces/codex-appserver-001
# Branch: autoship/codex-appserver-001
# Base: origin/master (fresh)
```

## Root Cause (Fixed)

**Before:**

```bash
git worktree add .autoship/workspaces/$KEY -b autoship/$KEY master   # ← local ref, may be stale
```

**After (init.sh):**

```bash
git fetch origin master                                             # ← ensure fresh state
git worktree add .autoship/workspaces/$KEY -b autoship/$KEY origin/master  # ← remote ref, authoritative
```

## Testing

Verify stale inheritance is prevented:

1. Create a file on master
2. Commit and push to origin/master
3. Delete file from master locally (simulate stale local state)
4. Run `./init.sh test-worker`
5. Check that worker worktree has the file (from origin/master, not local)

```bash
# Setup
echo "test-content" > test-file.txt
git add test-file.txt
git commit -m "test: add file"
git push origin master

# Simulate stale local state
rm test-file.txt
git reset --hard HEAD~1

# Create worker
./init.sh test-worker

# Verify
ls .autoship/workspaces/test-worker/test-file.txt
# Should NOT exist (correct), since HEAD~1 doesn't have it
# But if using local master (stale), would pull from commit that had it (wrong)
```

## Related Issues

- #2466 — codex-appserver worker dispatch bugs
- #2464 — stale AUTOSHIP_RESULT.md (symptom, now fixed by proper dispatch init)
