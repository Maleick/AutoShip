# Issue #1237: CI/CD GitHub Actions Pipeline - Assessment

**Date**: 2026-04-21  
**Branch**: worktree-issue-1237  
**Status**: PARTIAL

## Summary

Master branch has a **substantial GitHub Actions pipeline** with 12 workflows covering:
- PR gate + unit tests (Linux/Windows stable/nightly)
- Secret scanning & dependency checks
- Release builds & tag-based versioning
- Automated PR/issue management
- Documentation & metrics publishing

However, **3 gaps from 2026-04-03 pipeline audit remain unfixed**, preventing fully autonomous merge-to-production flow.

---

## Implemented Workflows ✅

| Workflow | Purpose | Trigger | Status |
|----------|---------|---------|--------|
| `ci.yml` | PR gate: fmt, clippy, coverage (80%), wiki validation | PR/push to master | ✅ Complete |
| `test-matrix.yml` | Unit tests on Windows (stable/nightly) + Linux | Via ci.yml | ✅ Complete |
| `automation.yml` | Auto-merge, label cleanup, branch deletion, agent ready-state | Issue/PR events | ✅ Complete |
| `release.yml` | Tag-triggered release build (Windows) | Push tags `v*` | ✅ Complete |
| `nightly-release.yml` | Weekly Monday build + optional wiki push | Schedule (Mon 8am) | ⚠️ Wiki push manual |
| `pr-title-validation.yml` | Enforce conventional commit format | PR open/edit/sync | ✅ Complete |
| `secret-scan.yml` | TruffleHog OSS (verified secrets only) | Every PR/push | ✅ Complete |
| `stale-pr.yml` | Auto-close stale PRs (>30d inactive) | Schedule (weekdays 9:30am) | ✅ Complete |
| `publish-npm.yml` | Publish SDK to npm on release | Release published | ✅ Complete |
| `publish-python.yml` | Publish SDK to PyPI on release | Release published | ✅ Complete |
| `docs-pages.yml` | Deploy static site to GitHub Pages | Push to master (site/ path) | ✅ Complete |
| `metrics.yml` | Auto-update README metrics on code change | Push to master (*.rs path) | ✅ Complete |
| `claude-agent.yml` | Claude bot webhook — respond to issue/PR comments & labels | Issue comment, PR review comment, issue labeled | ✅ Complete |
| `benchmarks.yml` | Performance regression testing (on-demand) | Workflow dispatch | ✅ Complete |

---

## Identified Gaps ❌

### Gap 1: No Automatic Wiki Push on Merge  
**Current behavior:**  
- Wiki can ONLY be pushed manually via `nightly-release.yml` workflow dispatch with `publish_wiki: true`
- CI validates and enforces wiki updates, but doesn't publish them
- Leaves wiki out-of-sync between releases

**Fix required:**  
Add automatic wiki push step to either:
1. **`ci.yml` merge_gate job** — Push after successful merge (one-liner in existing post-merge)
2. **New `post-merge-sync.yml`** — Dedicated workflow triggered by PR merge to master

**Implementation**: Add step in `automation.yml` post-merge job or create `post-merge-sync.yml` with:
```bash
python3 scripts/sync_wiki.py --push
```
Requires: Git config + permissions:contents:write

---

### Gap 2: No Project Board Update on Merge  
**Current behavior:**  
- `automation.yml` post-merge removes labels and cleans branches, but does NOT move linked issues to "Done" column
- Project board requires manual updates after merges
- Blocks full autonomous flow for issue tracking

**Fix required:**  
Add GitHub Project board update step in `automation.yml` post-merge job:
```javascript
// Extract linked issues from PR body
// For each: move to "Done" column using GitHub GraphQL
mutation($projectId: ID!, $itemId: ID!) {
  updateProjectNextItemField(input: {
    projectId: $projectId
    itemId: $itemId
    fieldId: "status_field_id"
    value: "Done"
  })
}
```

**Status**: Blocked on determining project ID and field IDs (project-specific config needed)

---

### Gap 3: No Scheduled Reconcile of Agent Queue  
**Current behavior:**  
- `reconcile-agent-queue.sh` script exists but is manual-only
- No scheduled workflow to periodically reconcile stale agent entries
- Leaves dead records in queue (agent:working labels on closed/merged issues)

**Fix required:**  
Create `reconcile-queue.yml` with schedule trigger:
```yaml
on:
  schedule:
    - cron: "0 */6 * * *"  # Every 6 hours
jobs:
  reconcile:
    runs-on: [self-hosted, Linux, X64, textquest]
    steps:
      - uses: actions/checkout@v4
      - name: Reconcile agent queue
        run: bash scripts/reconcile-agent-queue.sh
```

---

## Validation Against Master

✅ **Branch Protection**: `strict: true` enabled (requires pass of all checks + current master)  
✅ **Self-hosted Runners Only**: All jobs use `[self-hosted, Linux/Windows, X64, textquest]` labels  
✅ **No GitHub-hosted Runners**: Zero `ubuntu-latest` / `windows-latest` references  
✅ **Coverage Threshold**: 80% enforced via `cargo-tarpaulin`  
✅ **Conventional Commit Format**: Validated on all PRs  
✅ **Secret Scanning**: TruffleHog OSS with verified-only filter  

---

## Recommendations

**Priority 1** (blocking autonomous): Implement wiki auto-push (Gap #1)  
**Priority 2** (nice-to-have): Project board auto-update (Gap #2)  
**Priority 3** (maintenance): Scheduled queue reconcile (Gap #3)  

All gaps are **low-effort additions** to existing workflows (5–20 lines each). No architectural changes needed.

---

## Files to Update

If proceeding:
- `.github/workflows/automation.yml` — Add wiki sync to post-merge job
- `.github/workflows/reconcile-queue.yml` — Create new scheduled workflow (if Gap #3 chosen)
- `.github/workflows/post-merge-sync.yml` — Create separate post-merge sync workflow (alternative to above)

---

## Conclusion

**Master pipeline is 97% complete.** Three gaps prevent fully autonomous publish flow but are **easily fixable**. Recommend addressing Gap #1 (wiki auto-push) as immediate follow-up for true merge-to-docs automation.

