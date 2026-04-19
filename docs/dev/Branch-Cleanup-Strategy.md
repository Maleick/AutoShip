# TextQuest Branch Cleanup Strategy

**Status**: Reference only
**Last updated**: 2026-04-12

> Historical note: `.github/workflows/branch-cleanup.yml` was removed during CI workflow rationalization on 2026-04-13. This document is reference-only and no longer describes active branch-cleanup automation.

---

## Overview

Branch cleanup was historically handled through a mix of repository settings, a GitHub Actions workflow, manual commands, and branch protection rules. That automation was removed on 2026-04-13 and this page now serves as historical reference only.

1. **Automatic cleanup on PR merge** (GitHub settings)
2. **Scheduled stale branch cleanup** (GitHub Actions workflow, removed on 2026-04-13)
3. **Manual commands** for developer use
4. **Protected branches** (master, main, develop, staging, production) were never auto-deleted

---

## Historical Cleanup Mechanisms

### 1. GitHub Repository Settings

**Enable "Delete head branch on merge"**:
- When a PR was merged, the source branch was automatically deleted if the setting was enabled
- Prevented accumulation of merged branches
- Reduced the need for manual branch deletion after merge

**Setting location**: Repository Settings → General → "Automatically delete head branches"

**Status**: Historical note — this was the recommended repository setting before the workflow was removed

### 2. GitHub Actions Workflow

**File**: `.github/workflows/branch-cleanup.yml` (removed on 2026-04-13)

**Triggers**:
- Weekly schedule (Sunday 3:00 AM UTC) — cleaned stale branches (>30 days old)
- Manual dispatch — on-demand cleanup

The workflow no longer exists, so there is no active schedule, no dispatch target, and no job log location to inspect.

**Protected branches** (never deleted):
- `master`
- `main`
- `develop`
- `staging`
- `production`

**Features**:
- Deleted branches with last commit >30 days old
- Skipped fork PR head branches and stale branches that still backed open PRs
- Pruned remote-tracking refs for deleted remotes
- Reported cleanup status and remaining branches
- Was safe: skipped protected branches and handled errors gracefully

---

## Branch Naming Conventions

These branch names were used to work well with the historical cleanup automation:

### Feature Branches
Format: `feature/<description>` or `feat/<description>`
- Example: `feature/soul-engine-sentiment`
- Example: `feat/combat-reactions`

### Fix Branches
Format: `fix/<description>` or `bugfix/<description>`
- Example: `fix/memory-decay-bug`
- Example: `bugfix/sentiment-npe`

### Chore Branches
Format: `chore/<description>`
- Example: `chore/update-dependencies`

### Research/Experiment
Format: `research/<description>` or `experiment/<description>`
- Example: `research/llm-integration`
- Example: `experiment/gossip-system`

### Agent/Copilot Work
Format: `claude/<description>` or `copilot/<description>`
- Example: `claude/define-soul-system-W8Gbb`
- Example: `copilot/fix-memory-leak`

---

## Historical Manual Cleanup

### List branches to delete
```bash
# Show local branches that are fully merged
git branch --merged

# Show remote branches older than 30 days
git branch -r --sort=-committerdate | head -20

# Show stale local branches (no upstream)
git branch -vv | grep '\[gone\]'
```

### Delete local branch
```bash
# Delete merged local branch
git branch -d <branch-name>

# Force delete local branch (even if not merged)
git branch -D <branch-name>
```

### Delete remote branch
```bash
# Delete remote branch
git push origin --delete <branch-name>

# Shorthand
git push origin :<branch-name>
```

### Clean up tracking branches
```bash
# Remove local branches with deleted remotes
git fetch origin --prune

# Clean up all stale remote tracking branches
git branch -r | grep ': gone]' | awk '{print $1}' | xargs -r git branch -rd
```

---

## Historical Workflow for Teams

### When a feature was finished:

1. **Create Pull Request**
   - Push your feature branch to origin
   - Create PR against master/main
   - Request review

2. **Get Approval**
   - Reviewer approves changes
   - All checks pass

3. **Merge PR**
   - Use "Squash and merge" or "Create merge commit"
   - Check: "Delete head branch" option is checked
   - Click "Merge"

4. **Automatic Cleanup** (GitHub Actions, removed on 2026-04-13)
    - PR closed → branch was automatically deleted
    - No manual action was needed

5. **Local Cleanup**
   - `git fetch origin --prune`
   - Your local branch is now gone
   - You can start the next feature

### For long-running branches:

If a branch was active for >30 days:
- It was kept on origin with regular updates
- Open PR branches were skipped by stale cleanup
- No action was needed

If a branch was abandoned:
- Scheduled cleanup would delete it after 30 days
- No action was needed

---

## Historical Branch Lifecycle

```
Created
  ↓
  Active (< 30 days)
  ↓
  Stale (> 30 days)
  ↓
  Scheduled cleanup (historical daily check)
  ↓
  Deleted (if unpushed changes, git saves in reflog)
```

**Important**: If you had unpushed work, git kept it in the reflog for ~30 days. You could recover it with:
```bash
git reflog
git checkout <commit-hash>
```

---

## Historical Protected Branches

These branches were **never** automatically deleted:

- `master` — production-ready code
- `main` — primary development branch
- `develop` — integration branch
- `staging` — pre-production branch
- `production` — live code

Branch protection rules were typically set to:
- Require PR reviews before merge
- Require status checks to pass
- Require branch to be up to date
- Restrict who can dismiss reviews

---

## Removed GitHub Actions Cleanup Job

### `.github/workflows/branch-cleanup.yml`

This workflow was removed on 2026-04-13. It used to:

**On PR merge**:
1. Detected the merged PR
2. Got the source branch name
3. Verified the branch belonged to this repository and was not protected
4. Deleted the remote branch
5. Reported success

**On schedule (daily)**:
1. Fetched all remote branches
2. Fetched open PR head branches
3. Calculated branch age
4. Identified stale branches (>30 days) that were not protected and did not back open PRs
5. Deleted eligible stale branches
6. Generated a cleanup report

**Status report included**:
- Total remote branches
- Total local branches
- Protected branches
- Recently active branches
- Cleanup summary

---

## Historical Monitoring & Reporting

### Check branch status
```bash
# Historical note: the workflow run used to appear under Actions → Branch Cleanup
```

### Manual inspection
```bash
# List all remote branches with age
git branch -r --sort=-committerdate --format='%(refname:short) | %(committerdate:relative)'

# Count branches
git branch -r | wc -l

# Find branches older than 30 days
git branch -r --sort=-committerdate | while read branch; do
  AGE=$(git log -1 --format=%ai "$branch" | cut -d' ' -f1)
  DAYS=$((( $(date +%s) - $(date -d "$AGE" +%s) ) / 86400))
  if [ "$DAYS" -gt 30 ]; then
    echo "$DAYS days: $branch"
  fi
done
```

---

## Historical Troubleshooting

### "Branch is protected"
- Branch was in the protected list (master, main, etc.)
- Repo admins controlled protection settings
- Protected branches were not auto-deleted by design

### "Force push rejected"
- Branch protection required PR review
- Contributors used PRs instead of direct pushes
- Branch protection was not meant to be bypassed

### "Can't delete branch with unpushed changes"
- Push changes first: `git push origin <branch>`
- Then delete: `git push origin --delete <branch>`
- Or delete locally only: `git branch -d <branch>`

### "Local branch gone but still tracking"
- Run: `git fetch origin --prune`
- Local tracking branch was removed
- Your local tracking ref will be pruned on the next fetch

---

## Historical Best Practices

✅ **DO**:
- Delete branches after PR merge
- Use consistent naming conventions
- Update branches regularly while they were active
- Keep feature branches focused

❌ **DON'T**:
- Keep merged branches around
- Push to master directly
- Ignore branch protection
- Use vague branch names

---

## Historical Repository Settings Checklist

- [ ] "Delete head branch on merge" was enabled
- [ ] Branch protection existed on master/main
- [ ] Require PR reviews (1+)
- [ ] Require status checks
- [ ] Require up-to-date branch
- [ ] Restrict dismissal of reviews
- [ ] Allow force pushes: NO (for protected branches)
- [ ] Allow deletions: NO (for protected branches)

---

## Summary

**TextQuest branch cleanup was:**
- ✅ Automatic on PR merge when the GitHub setting was enabled
- ✅ Scheduled for stale branches via GitHub Actions before removal
- ✅ Safe (protected branches were never deleted)
- ✅ Transparent (status reports were generated)
- ✅ Reversible (git reflog kept history for about 30 days)

**No active workflow remains.** This page is reference-only, and there is no Branch Cleanup job log to inspect after 2026-04-13.
