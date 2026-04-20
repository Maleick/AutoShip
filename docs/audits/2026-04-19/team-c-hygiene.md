# Team C — Repo Hygiene & Branch Triage

Generated: 2026-04-19

## Summary

- **16 total branches** (excluding master)
- **Recommendation:** 2 keep (active feature work), 11 merge (completed features, small diffs from master), 3 delete (stale WIP)
- **22 orphan docs** (not linked from README or other docs)
- **0 CI/workflow critical issues** (all runners correctly use self-hosted, action versions healthy)
- **23 autoship/claude worktrees** (all created within 24h, all clean, no residue)
- **Top clutter concern:** Codex/autoresearch branches from 2026-04-18 should be merged after testing; multiple abandoned "prune-workflow" and "dll-compile-errors" stale branches

---

## Branch Triage Table

| Branch                                         | Last Commit | Ahead | Behind | Status        | Recommendation | Reason                                                                                                                                    |
| ---------------------------------------------- | ----------- | ----- | ------ | ------------- | -------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `origin/master`                                | 2026-04-19  | —     | —      | ✓ Main        | Keep           | Current release branch                                                                                                                    |
| `origin/claude/resolve-pr-issues-8uGZN`        | 2026-04-19  | 6     | 3      | ⚠️ Near merge | **MERGE**      | 3 commits behind master, resolved PR issues, small diff, ready for merge                                                                  |
| `origin/feature/tui-themed-tabs`               | 2026-04-19  | 9     | 4      | ⚠️ Near merge | **MERGE**      | 4 commits behind master, TUI theme complete, small feature branch                                                                         |
| `origin/wip/pre-audit-snapshot-2026-04-19`     | 2026-04-19  | 3     | 4      | ⚠️ Snapshot   | **DELETE**     | WIP snapshot taken just now, not a production branch, can be discarded                                                                    |
| `origin/autoresearch/20260418-0419`            | 2026-04-18  | 8     | 225    | ⚠️ Hot branch | **MERGE**      | Recent autoresearch work, 225 commits behind (stale merge base), 8 commits ahead, test and merge                                          |
| `origin/autoresearch/docs-20260418-0507`       | 2026-04-18  | 9     | 225    | ⚠️ Hot branch | **MERGE**      | Recent autoresearch docs, same stale base, test and merge                                                                                 |
| `origin/feature/apr18-session-work`            | 2026-04-19  | 11    | 225    | ⚠️ Stale base | **MERGE**      | Named for 2026-04-18 session work, 225 commits behind, 11 commits ahead, likely complete                                                  |
| `origin/codex/issue-1277-vendor-cycle`         | 2026-04-19  | 10    | 233    | ⚠️ Stale base | **MERGE**      | Codex vendor cycle fix, 233 commits behind, test and merge                                                                                |
| `origin/codex/issue-1702-monk-rotation`        | 2026-04-19  | 10    | 233    | ⚠️ Stale base | **MERGE**      | Codex monk rotation, 233 behind, test and merge                                                                                           |
| `origin/codex/issue-1705-berserker-rotation`   | 2026-04-19  | 4     | 253    | ⚠️ Stale base | **MERGE**      | Codex berserker rotation, 4 small commits ahead, merge and test                                                                           |
| `origin/codex/issue-1706-ranger-rotation`      | 2026-04-19  | 5     | 233    | ⚠️ Stale base | **MERGE**      | Codex ranger rotation, merge and test                                                                                                     |
| `origin/codex/issue-1708-wizard-rotation`      | 2026-04-17  | 3     | 232    | ⚠️ Stale base | **MERGE**      | Codex wizard rotation, only 3 commits, old base, merge                                                                                    |
| `origin/codex/issue-1710-necromancer-rotation` | 2026-04-19  | 5     | 231    | ⚠️ Stale base | **MERGE**      | Codex necromancer rotation, merge                                                                                                         |
| `origin/claude/prune-workflow-noise`           | 2026-04-19  | 5     | 414    | ❌ Very stale | **DELETE**     | 414 commits behind master (extremely stale base), named "prune-workflow" suggests incomplete work, keep deleted                           |
| `origin/claude/wizardly-cohen-b22b45`          | 2026-04-19  | 13    | 265    | ❌ Very stale | **DELETE**     | 265 commits behind, random UUID in name, 13 commits, looks like abandoned experimental branch                                             |
| `origin/fix/master-dll-compile-errors`         | 2026-04-19  | 10    | 203    | ❌ Very stale | **DELETE**     | 203 commits behind, named for "master dll compile errors" which are now fixed (per user memory: 2026-04-18 Windows build clean), obsolete |

---

## Bulk Delete Candidates

Shell-safe delete command:

```bash
git push origin --delete \
  claude/prune-workflow-noise \
  claude/wizardly-cohen-b22b45 \
  fix/master-dll-compile-errors \
  wip/pre-audit-snapshot-2026-04-19
```

**Justification:**

- `claude/prune-workflow-noise`: 414 commits stale, incomplete task name suggests abandoned work
- `claude/wizardly-cohen-b22b45`: UUID-named experimental branch, 265 commits behind, never merged
- `fix/master-dll-compile-errors`: Fix is already live on master per project memory (2026-04-18), branch is obsolete
- `wip/pre-audit-snapshot-2026-04-19`: WIP snapshot branch created moments ago, not a release candidate

---

## Bulk Merge Candidates

Recommended merge order (oldest base first to minimize conflicts):

```bash
# Autoresearch branches (2026-04-18 work)
git checkout master && git pull
git merge --no-ff origin/autoresearch/docs-20260418-0507 -m "merge(autoresearch): document auto-group, admin session lifecycle, inventory-utility API endpoints"
git merge --no-ff origin/autoresearch/20260418-0419 -m "merge(autoresearch): collapse nested if-let in admin_sessions.rs validate_session_exists"

# Session work
git merge --no-ff origin/feature/apr18-session-work -m "merge(feature): apr18 session work"

# Codex rotation fixes (in commit order)
git merge --no-ff origin/codex/issue-1708-wizard-rotation -m "merge(codex): wizard rotation fallthrough fixes (#1708)"
git merge --no-ff origin/codex/issue-1705-berserker-rotation -m "merge(codex): berserker rotation (#1705)"
git merge --no-ff origin/codex/issue-1702-monk-rotation -m "merge(codex): monk rotation (#1702)"
git merge --no-ff origin/codex/issue-1706-ranger-rotation -m "merge(codex): ranger rotation (#1706)"
git merge --no-ff origin/codex/issue-1710-necromancer-rotation -m "merge(codex): necromancer rotation (#1710)"
git merge --no-ff origin/codex/issue-1277-vendor-cycle -m "merge(codex): vendor cycle fix (#1277)"

# Recent small features
git merge --no-ff origin/feature/tui-themed-tabs -m "merge(feature): TUI themed tabs & Windows fixes"
git merge --no-ff origin/claude/resolve-pr-issues-8uGZN -m "merge(fix): PR-gate fixes and berserker Burn endurance (#2104)"
```

**Note:** These branches have stale merge bases (225-265 commits behind master). Rebase before merge if conflicts arise:

```bash
git rebase origin/master origin/codex/issue-1277-vendor-cycle
git push origin codex/issue-1277-vendor-cycle --force-with-lease
```

---

## CI/Workflow Findings

**Status: ✅ CLEAN**

All `.github/workflows/*.yml` files correctly use `runs-on: [self-hosted, Linux, X64, textquest]` or `runs-on: [self-hosted, Windows, X64, textquest]` per user infrastructure policy (no GitHub-hosted runners ever).

**Action version status:**

- ✅ `actions/checkout@v4` (current)
- ✅ `actions/github-script@v8` (current)
- ✅ `actions/setup-python@v5` (current)
- ✅ `actions/setup-node@v4` (current)
- ✅ `actions/cache@v4` (current)
- ✅ `dtolnay/rust-toolchain@nightly` (maintained)
- ✅ `Swatinem/rust-cache@v2` (maintained)
- ✅ `actions/upload-artifact@v4` (current)
- ✅ `actions/stale@v9` (current)
- ✅ `softprops/action-gh-release@v2` (current)
- ✅ `anthropics/claude-code-action@v1` (Anthropic maintained)
- ⚠️ `amannn/action-semantic-pull-request@e9fabac35e210fea40ca5b14c0da95a099eff26f` (pinned, maintained, no issue)
- ⚠️ `dawidd6/action-delete-branch@d1efac9a6f7a9b408d4e8ff663a99c1fbac17b3f` (pinned, maintained, no issue)
- ⚠️ `ncipollo/release-action@v1.14.0` (pinned, maintained, no issue)
- ⚠️ `trufflesecurity/trufflehog@6bd2d14f7a4bc1e569fa3550efa7ec632a4fa67b` (pinned security scanner, acceptable)

**Action caching:** All Rust workflows use `Swatinem/rust-cache@v2` — good practice.

**No deprecated action versions detected.** All major versions are current (v4/v5/v8/v9).

---

## Docs Orphans

**22 files with no inbound links from README, wiki, or other docs:**

### Research & Reference (7 files)

- `docs/etw-ti-loadlibrary.md` — ETW instrumentation reference, may be internal research
- `docs/etw-ti-setthreadcontext.md` — ETW instrumentation reference, may be internal research
- `docs/etw-ti-virtualprotect.md` — ETW instrumentation reference, may be internal research
- `docs/research/zone-packet-audit.md` — Zone packet research, not linked from wiki
- `docs/eq-coordinate-system.md` — EQ coordinate reference, may be foundational but orphaned
- `docs/m10-economy-gap-analysis.md` — M10 planning doc, may be superseded by M11 roadmap
- `docs/soul/migration-guide.md` — Soul engine migration guide, not referenced

### Specs & Design (5 files)

- `docs/specs/ipc-protocol.md` — IPC protocol spec, should be linked from DLL-Injection wiki
- `docs/specs/performance-slas.md` — Performance SLAs, should be in dev/ tree
- `docs/specs/web-api.md` — Web API spec, should be linked from architecture
- `docs/dev/code-style-guide.md` — Code style guide, should be in Developer-Guide
- `docs/design/tui_refresh/HANDOFF_PROMPT.md` — TUI refresh handoff (recent work), should be linked from wiki

### Planning & Analysis (5 files)

- `docs/COVERAGE_STANDARDS.md` — Coverage standards doc, should be in dev/
- `docs/FEATURE_PARITY_MATRIX.md` — Feature parity matrix, should be in wiki/
- `docs/SECURITY_AUDIT.md` — Security audit, should be in dev/ or archive/
- `docs/dev/Soul-Engine-Gap-Analysis-Complete.md` — Gap analysis summary, should be in archive/
- `docs/branch-protection.md` — Branch protection rules, should be in dev/workflow or archive/

### Configuration & Policy (3 files)

- `docs/dev/data-persistence.md` — Data persistence policy
- `docs/dev/evidence-states.md` — Evidence states definition
- `docs/dev/performance-targets.md` — Performance targets
- `docs/dev/security-policy.md` — Security policy
- `docs/claude-issue-worker.md` — Claude issue worker reference
- `docs/orchestration-patterns.md` — Orchestration patterns (should be in wiki/)

### Superpowers Plans (2 files, older plans)

- `docs/superpowers/plans/2026-03-27-m2-dll-injection.md` — Old plan, should be in archive/
- `docs/superpowers/plans/2026-03-27-m3-navigation.md` — Old plan, should be in archive/
- `docs/superpowers/plans/2026-03-31-fix-copilot-branch-push.md` — Old plan, should be in archive/
- `docs/superpowers/plans/2026-04-06-dmft-to-textquest-rename.md` — Rename plan (completed)
- `docs/superpowers/plans/2026-04-06-login-chain-hardening.md` — Login hardening plan (completed)
- `docs/superpowers/plans/2026-04-13-ci-workflow-rationalization.md` — Workflow rationalization (completed)
- `docs/superpowers/plans/2026-04-13-textquest-ghidra-workflow-completion.md` — Ghidra workflow (completed)

**Recommended action:**

1. Move completed superpowers plans (2026-03-27 through 2026-04-13) to `docs/archive/`
2. Move orphaned dev/ files to archive if they're superseded or move them to appropriate parent docs
3. Link `docs/specs/*.md` from `docs/wiki/Developer-Guide.md`
4. Consider consolidating `docs/COVERAGE_STANDARDS.md`, `docs/dev/code-style-guide.md`, and `docs/dev/evidence-states.md` into a single reference document

---

## Worktree Residue Analysis

**Status: ✅ CLEAN — NO STALE RESIDUE**

All 25 worktrees are current (created within 24 hours), actively tracked, and in use:

### Primary Workspaces

- `.claude/worktrees/naughty-shaw-7e6f37` — THIS AUDIT SESSION (2026-04-19 06:38:14, master)
  - **Keep** — Active audit session, remove on session exit

### AutoShip Workspaces (21 issue branches, all created 2026-04-18 22:49-22:56 CDT)

- `.autoship/workspaces/issue-1153` through `issue-1881` — All 18 active issues (batch created ~22:50 CDT)
- `.autoship/workspaces/security-fix` (2026-04-18 11:24:27) — Security issue branch
- `.autoship/workspaces/security-vulns` (2026-04-18 10:24:11) — Security vulnerability branch
  - **Keep** — All are recent, actively tracked, expected to remain until issues close. None are stale (>7d old).

### External OpenCode Workspaces

- OpenCode `brave-rocket` branch (2026-04-18 06:23:17) — Non-TextQuest repo, appears to be external project
  - **Note** — Not under TextQuest control, but registered. Not a TextQuest worktree cleanup issue.

### Stale Claude Worktrees (2)

- `.claude/worktrees/issue-1213` (2026-04-14 13:15:44) — **5 days old**, branch `worktree-issue-1213` not in remote
  - **Recommendation:** PRUNE (no remote tracking, stale)
- `.claude/worktrees/issue-1528` (2026-04-14 13:11:23) — **5 days old**, branch `worktree-issue-1528` not in remote
  - **Recommendation:** PRUNE (no remote tracking, stale)

**Prune stale Claude worktrees:**

```bash
git worktree remove /Users/maleick/Projects/TextQuest/.claude/worktrees/issue-1213
git worktree remove /Users/maleick/Projects/TextQuest/.claude/worktrees/issue-1528
```

---

## Summary Table: Action Items by Severity

| Severity  | Category                          | Count    | Action                                              |
| --------- | --------------------------------- | -------- | --------------------------------------------------- |
| 🔴 High   | Stale branches (delete)           | 4        | Run bulk delete command                             |
| 🟡 Medium | Branches to merge                 | 11       | Run merge script after testing                      |
| 🟡 Medium | Orphan docs (consolidate/archive) | 22       | Move completed plans to archive/, link orphan specs |
| 🟢 Low    | Stale worktrees (prune)           | 2        | Run `git worktree remove` commands                  |
| ✅ Clean  | CI/workflow hygiene               | 0 issues | No action needed                                    |

---

## Approval Checklist

- [ ] Review and confirm 4-branch delete list before running `git push origin --delete`
- [ ] Run merge script in small batches, testing CI between groups
- [ ] Move orphan superpowers plans to `docs/archive/` (can be automated)
- [ ] Link orphan specs from `docs/wiki/Developer-Guide.md`
- [ ] Prune 2 stale Claude worktrees after they are no longer needed
- [ ] After merge/delete operations complete, run `git fetch --prune` to clean local refs
