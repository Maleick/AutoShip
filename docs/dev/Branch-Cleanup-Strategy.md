# TextQuest Branch Cleanup Strategy

**Status**: Automated + Manual cleanup enabled  
**Last updated**: 2026-04-12

---

## Overview

Branch cleanup is critical for repository hygiene. TextQuest uses a multi-layered approach:

1. **Automatic cleanup on PR merge** (GitHub settings)
2. **Scheduled stale branch cleanup** (daily, via GitHub Actions)
3. **Manual commands** for developer use
4. **Protected branches** (master, main, develop, staging, production) are never auto-deleted

---

## Automatic Cleanup Mechanisms

### 1. GitHub Repository Settings

**Enable "Delete head branch on merge"**:
- When a PR is merged, the source branch is automatically deleted
- Prevents accumulation of merged branches
- Developers don't need to manually delete branches after merge

**Setting location**: Repository Settings → General → "Automatically delete head branches"

**Status**: ⏳ **RECOMMENDED** — Should be enabled in repository settings

### 2. GitHub Actions Workflow

**File**: `.github/workflows/branch-cleanup.yml`

**Triggers**:
- On PR close (merged) — immediately delete source branch
- Daily schedule (2 AM UTC) — clean stale branches (>30 days old)
- Manual dispatch — on-demand cleanup

**Protected branches** (never deleted):
- `master`
- `main`
- `develop`
- `staging`
- `production`

**Features**:
- Deletes branches with last commit >30 days old
- Skips fork PR head branches and stale branches that still back open PRs
- Prunes remote-tracking refs for deleted remotes
- Reports cleanup status and remaining branches
- Safe: skips protected branches, handles errors gracefully

---

## Branch Naming Conventions

To work well with cleanup automation:

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

## Manual Branch Cleanup

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

## Branch Cleanup Workflow for Teams

### When you finish a feature:

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

4. **Automatic Cleanup** (GitHub Actions)
    - PR closed → branch automatically deleted
    - No manual action needed

5. **Local Cleanup**
   - `git fetch origin --prune`
   - Your local branch is now gone
   - You can start the next feature

### For long-running branches:

If your branch is active for >30 days:
- Keep it on origin (daily updates)
- Open PR branches are skipped by stale cleanup
- No action needed from you

If your branch is abandoned:
- Scheduled cleanup will delete it after 30 days
- No action needed from you

---

## Branch Lifecycle

```
Created
  ↓
  Active (< 30 days)
  ↓
  Stale (> 30 days)
  ↓
  Scheduled cleanup (daily check)
  ↓
  Deleted (if unpushed changes, git saves in reflog)
```

**Important**: If you have unpushed work, git keeps it in the reflog for ~30 days. You can recover it with:
```bash
git reflog
git checkout <commit-hash>
```

---

## Exception: Protected Branches

These branches are **never** automatically deleted:

- `master` — production-ready code
- `main` — primary development branch
- `develop` — integration branch
- `staging` — pre-production branch
- `production` — live code

Branch protection rules should also be set:
- Require PR reviews before merge
- Require status checks to pass
- Require branch to be up to date
- Restrict who can dismiss reviews

---

## GitHub Actions Cleanup Job

### `.github/workflows/branch-cleanup.yml`

**On PR merge**:
1. Detects merged PR
2. Gets source branch name
3. Verifies the branch belongs to this repository and is not protected
4. Deletes remote branch
5. Reports success

**On schedule (daily)**:
1. Fetches all remote branches
2. Fetches open PR head branches
3. Calculates branch age
4. Identifies stale branches (>30 days) that are not protected and do not back open PRs
5. Deletes eligible stale branches
6. Generates cleanup report

**Status report includes**:
- Total remote branches
- Total local branches
- Protected branches
- Recently active branches
- Cleanup summary

---

## Monitoring & Reporting

### Check branch status
```bash
# See what the cleanup job reports
# Look at: Actions → Branch Cleanup → Latest run
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

## Troubleshooting

### "Branch is protected"
- Branch is in protected list (master, main, etc.)
- Contact repo admin to unprotect if needed
- Cannot auto-delete protected branches (intentional)

### "Force push rejected"
- Branch protection requires PR review
- Create PR instead of pushing directly
- Don't bypass branch protection

### "Can't delete branch with unpushed changes"
- Push changes first: `git push origin <branch>`
- Then delete: `git push origin --delete <branch>`
- Or delete locally only: `git branch -d <branch>`

### "Local branch gone but still tracking"
- Run: `git fetch origin --prune`
- Local tracking branch will be removed
- Your branch will be deleted next day

---

## Best Practices

✅ **DO**:
- Delete branches after PR merge
- Use consistent naming conventions
- Update branch daily if active
- Keep feature branches focused

❌ **DON'T**:
- Keep merged branches around
- Push to master directly
- Ignore branch protection
- Use vague branch names

---

## Repository Settings Checklist

- [ ] "Delete head branch on merge" enabled
- [ ] Branch protection on master/main
- [ ] Require PR reviews (1+)
- [ ] Require status checks
- [ ] Require up-to-date branch
- [ ] Restrict dismissal of reviews
- [ ] Allow force pushes: NO (for protected branches)
- [ ] Allow deletions: NO (for protected branches)

---

## Summary

**TextQuest branch cleanup is:**
- ✅ Automatic on PR merge (GitHub setting)
- ✅ Scheduled daily for stale branches (GitHub Actions)
- ✅ Safe (protected branches never deleted)
- ✅ Transparent (status reports generated)
- ✅ Reversible (git reflog keeps history 30 days)

**No manual action needed** unless you have special requirements.

For questions, check the automation job logs: **Actions → Branch Cleanup**.
