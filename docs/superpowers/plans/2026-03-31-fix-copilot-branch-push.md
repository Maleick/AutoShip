# Fix Copilot Agent Branch Push Rejection

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve the GH013 repository rule violation that prevents the GitHub Copilot coding agent from pushing branches matching `claude/*`.

**Architecture:** This is a GitHub settings fix, not a code change. The Copilot agent runs as a GitHub Action that commits changes and pushes to the PR branch. A branch creation restriction is blocking the push. We fix it at the source (GitHub rulesets settings), with a fallback CI workflow option if the ruleset can't be modified.

**Tech Stack:** GitHub Repository Rulesets (web UI), `gh` CLI, GitHub Actions

---

## Context

### What happened
- GitHub Copilot coding agent (run `23811627694`) responded to PR review comments on PR #5
- Agent completed code changes, all 1183 tests passed, committed locally as `9b4f993`
- Push to `claude/funny-mclean` was rejected: `GH013: Cannot create ref due to creations being restricted`
- PR #5 is now merged (the code was manually applied), but the failure will recur on future Copilot agent runs

### Why the API shows no rulesets
- `gh api repos/Maleick/DMFT/rulesets` returns `[]`
- `gh api repos/Maleick/DMFT/rulesets?includes_parents=true` returns `[]`
- Branch protection on master is disabled
- Earlier `claude/*` branches (PRs #1, #2, #3) succeeded — this restriction was **added recently**
- GitHub has default/managed rulesets that are enforced server-side but not exposed via REST API
- The restriction is only visible through the **web UI**: `https://github.com/Maleick/DMFT/settings/rules`

### Branch history
| PR | Branch | Status |
|----|--------|--------|
| #1 | `claude/cool-mccarthy` | Merged (push worked) |
| #2 | `claude/inspiring-varahamihira` | Merged (push worked) |
| #3 | `claude/vibrant-ishizaka` | Merged (push worked) |
| #4 | `docs/claude-md-audit-cleanup` | Merged (push worked) |
| #5 | `claude/funny-mclean` | Merged (push **failed** on follow-up) |

---

## Option A: Fix the Ruleset (Recommended — 2 minutes)

This is the correct fix if you want the Copilot agent to keep working as designed.

### Task 1: Inspect and modify the repository ruleset via web UI

- [ ] **Step 1: Open the rulesets page**

Navigate to: `https://github.com/Maleick/DMFT/settings/rules`

Look for any ruleset that has a "Restrict creations" rule enabled. This is likely a recently-added default ruleset or one you configured without realizing it affects `claude/*` branches.

- [ ] **Step 2: Identify the blocking rule**

Click into each ruleset. Look for:
- **Target branches:** Does it target "All branches" or a pattern that matches `claude/*`?
- **Rules:** Is "Restrict creations" enabled?

The one blocking the Copilot agent will have "Restrict creations" enabled and target branches that include `claude/*`.

- [ ] **Step 3: Fix the ruleset (choose one approach)**

**Approach 3a — Add a bypass for the Copilot bot (preferred):**
1. In the ruleset, scroll to "Bypass list"
2. Click "Add bypass"
3. Add: `copilot-swe-agent[bot]` (this is the bot identity from the logs: `COPILOT_AGENT_COMMIT_LOGIN: copilot-swe-agent[bot]`)
4. Set bypass mode to "Always"
5. Save

**Approach 3b — Exclude `claude/*` branches from the ruleset:**
1. In "Target branches", change from "All branches" to specific patterns
2. Add patterns for branches you want to protect (e.g., `master`, `main`, `release/*`)
3. Leave `claude/*` and `docs/*` unprotected
4. Save

**Approach 3c — Disable "Restrict creations" entirely:**
1. In the ruleset, uncheck "Restrict creations"
2. Keep other rules (like "Restrict deletions", "Require PR before merge") if desired
3. Save

- [ ] **Step 4: Verify the fix**

```bash
# Create a test branch and push it
git checkout -b claude/test-push-fix
git commit --allow-empty -m "test: verify Copilot branch push works"
git push origin claude/test-push-fix

# Clean up
git checkout master
git push origin --delete claude/test-push-fix
git branch -d claude/test-push-fix
```

Expected: Push succeeds without GH013 error.

- [ ] **Step 5: Verify via `gh` CLI**

```bash
# After saving the ruleset change, the API should reflect it
gh api repos/Maleick/DMFT/rulesets
gh api "/repos/Maleick/DMFT/rules/branches/claude%2Ftest"
```

---

## Option B: Alternative — If You Can't Find/Modify the Ruleset

If the ruleset is GitHub-managed and not editable, create a new ruleset that explicitly allows `claude/*` branches.

### Task 2: Create a permissive ruleset for Copilot branches via `gh` CLI

- [ ] **Step 1: Create a ruleset that allows `claude/*` branch creation**

```bash
gh api repos/Maleick/DMFT/rulesets \
  --method POST \
  --field name="Allow Copilot Branches" \
  --field target="branch" \
  --field enforcement="active" \
  --field bypass_actors='[{"actor_id": 1, "actor_type": "OrganizationAdmin", "bypass_mode": "always"}]' \
  --field conditions='{"ref_name": {"include": ["refs/heads/claude/*"], "exclude": []}}' \
  --field rules='[]'
```

Note: This creates a ruleset with no restrictions targeting `claude/*` branches, which may override the default restriction. If this doesn't work, the restriction is server-managed and only Option A (web UI) will fix it.

---

## Option C: Nuclear — Contact GitHub Support

If neither Option A nor B works (the restriction is invisible and non-editable):

- [ ] **Step 1: File a support ticket**

Go to: `https://support.github.com/`

Include:
- Repository: `Maleick/DMFT`
- Error: `GH013: Repository rule violations found for refs/heads/claude/funny-mclean`
- Context: Copilot coding agent (SWE agent) cannot push to its own `claude/*` branches
- Evidence: `gh api repos/Maleick/DMFT/rulesets` returns `[]` — no user-configured rulesets visible
- Request: Remove or bypass the default "Restrict creations" rule for Copilot agent branches

---

## Also: CI Failures on PR #5 (Separate Issue)

The merged PR #5 also has **CI failures** (both macOS and Windows checks failed). This is a separate issue from the push rejection. The CI failures should be investigated after the ruleset fix:

```bash
# View the CI failure details
gh run view 23811722919 --repo Maleick/DMFT --log-failed
```

These CI failures are on the standard `ci.yml` workflow (fmt + clippy + test on macOS, build on Windows), not the Copilot agent action. They may indicate the merged test code has issues that need fixing on master.
