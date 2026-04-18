# Branch Protection Rules

This document describes the recommended GitHub branch protection configuration for the TextQuest repository.

## Overview

Branch protection rules enforce code quality standards and maintain repository integrity by requiring reviews, status checks, and administrative approval before code can be merged to protected branches.

## Protected Branches

The following branches should be protected with the rules described below:

- `master` — Main production branch
- `develop` — Integration branch (if used)

## Recommended Configuration for `master`

### Basic Settings

| Setting | Value | Reason |
|---------|-------|--------|
| Require a pull request before merging | ✅ Enabled | Enforces code review workflow |
| Require approvals | ✅ Enabled (1 approval) | At least one code review required |
| Require review from code owners | ✅ Enabled | Designated maintainers must approve |
| Require conversation resolution | ✅ Enabled | All review comments must be resolved |
| Require status checks to pass | ✅ Enabled | Must pass CI/CD pipeline |

### Required Status Checks

The following GitHub Actions workflows must pass before merge:

- `CI / PR gate (fmt + clippy + test + python)` — Rust formatting, linting, tests, Python tests
- `CI / Secret scan` — Verify no secrets in commit history
- `PR Title Validation / Validate PR title format` — Enforce conventional commit format

Enable the following options:

| Setting | Value | Reason |
|---------|-------|--------|
| Require branches to be up to date before merging | ✅ Enabled | Prevents merge conflicts and stale code |
| Require status checks to pass before merging | ✅ Enabled | CI must pass |
| Require code scanning results to pass | ⚠️ Optional | Recommended if CodeQL is enabled |

### Dismiss and Push Permissions

| Setting | Value | Reason |
|---------|-------|--------|
| Allow auto-merge | ✅ Enabled | Automates cleanup when PR meets all requirements |
| Auto-delete head branches | ✅ Enabled | Reduces branch clutter post-merge |
| Dismiss stale PR approvals when new commits are pushed | ✅ Enabled | Ensures reviews are fresh |
| Allow force pushes | ❌ Disabled | Prevents rewriting history |
| Allow deletions | ❌ Disabled | Prevents accidental branch deletion |

### Permit Exceptions

- **Restrict who can push to matching branches** — Optional, set to repository admins only
- **Require dismissal of stale reviews** — ✅ Enabled

## Setup Instructions

### Via GitHub Web Interface

1. Navigate to **Settings** → **Branches**
2. Click **Add rule** under "Branch protection rules"
3. Enter `master` as the pattern name
4. Enable all settings as shown in tables above
5. Click **Create**

### Via GitHub CLI

```bash
# This requires GitHub CLI (gh) and appropriate permissions

# Set branch protection for master
gh api repos/{owner}/{repo}/branches/master/protection \
  --method PUT \
  -f required_pull_request_reviews='{"require_code_owner_reviews":true,"required_approving_review_count":1}' \
  -f required_status_checks='{"strict":true,"contexts":["CI / PR gate (fmt + clippy + test + python)","CI / Secret scan","PR Title Validation / Validate PR title format"]}' \
  -f enforce_admins=true \
  -f allow_force_pushes=false \
  -f allow_deletions=false \
  -f auto_merge_enabled=true \
  -f delete_branch_on_merge=true
```

## PR Title Format

All pull request titles must follow the **Conventional Commits** specification:

```
<type>(<scope>): <description>
```

### Valid Types

- `feat` — A new feature
- `fix` — A bug fix
- `docs` — Documentation updates
- `style` — Code style changes (formatting, semicolons, etc.)
- `refactor` — Code refactoring without feature changes
- `test` — Adding or updating tests
- `chore` — Build system, dependencies, tooling
- `ci` — CI/CD workflow changes

### Examples

```
feat(combat): add ability cooldown tracking
fix: resolve race condition in rotation evaluation
docs: update branch protection guidelines
ci: upgrade GitHub Actions checkout to v4
refactor(protocol): simplify IPC message handling
test: add unit tests for cooldown tracker
```

## Automatic Workflows

The following workflows support the branch protection strategy:

### PR Title Validation (`pr-title-validation.yml`)

- **Trigger**: PR opened, edited, or synchronized
- **Checks**: PR title must match conventional commit format
- **Failure**: PR cannot be merged until title is corrected

### Auto Branch Cleanup (`auto-branch-cleanup.yml`)

- **Trigger**: PR merged to master
- **Action**: Automatically deletes the merged branch
- **Benefit**: Reduces branch clutter, encourages short-lived feature branches

## Troubleshooting

### PR blocked by required status checks

**Problem**: A status check is failing and blocking merge.

**Solution**:
1. Check the failing workflow logs in the **Checks** tab
2. Address the root cause (test failure, lint error, etc.)
3. Push a fix to the PR branch
4. Wait for checks to re-run and pass

### Dismiss stale reviews

**Problem**: An approval is marked stale after new commits are pushed.

**Solution**: This is expected behavior. Request a fresh review from the original reviewer or another maintainer.

### Force push protection

**Problem**: Cannot force push to master.

**Solution**: Force pushes are intentionally blocked to prevent history rewriting. Use a new branch and PR instead.

## References

- [GitHub Branch Protection Rules Documentation](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)
- [Conventional Commits Specification](https://www.conventionalcommits.org/)
- [GitHub Actions Syntax Reference](https://docs.github.com/en/actions/using-workflows/workflow-syntax-for-github-actions)
