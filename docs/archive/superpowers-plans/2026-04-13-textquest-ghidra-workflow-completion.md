# TextQuest-Ghidra Workflow Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the `TextQuest` / `TextQuest-Ghidra` boundary explicit, keep `TextQuest-Ghidra` as the canonical evidence repo, and normalize the active Ghidra backlog so the patch-cycle workflow is easy to repeat after every game update.

**Architecture:** `TextQuest` stays the operator/docs hub and publishes one clear Ghidra workflow page. `TextQuest-Ghidra` keeps ownership of immutable evidence, intake, and active research issues, while GitHub labels and issue bodies make the backlog legible without collapsing the repos or introducing a submodule. The patch cycle always flows from Ghidra intake and snapshot promotion back into TextQuest pointers.

**Tech Stack:** Markdown docs, GitHub CLI (`gh`), GitHub issue/PR metadata, existing JSON ledger (`feature-list.json`), existing repo docs/handoff files.

---

### Task 1: Publish the single Ghidra workflow page in TextQuest

**Files:**
- Create: `docs/wiki/Ghidra-Evidence-Workflow.md`
- Modify: `README.md`
- Modify: `docs/wiki/Home.md`
- Modify: `docs/wiki/Development-Workflow.md`

- [ ] **Step 1: Write the workflow page content**

Use this exact page structure:

```md
# Ghidra Evidence Workflow

## Canonical split

- `TextQuest-Ghidra` owns immutable snapshots, manifests, baseline selection, and Ghidra-side research state.
- `TextQuest` owns code, docs, runbooks, automation, and lightweight pointers to the canonical evidence.
- Do not copy canonical evidence payloads into `TextQuest`.

## Patch cycle

1. Stage fresh binaries and analysis inputs in `TextQuest-Ghidra`.
2. Refresh the canonical snapshot and baseline selection in `TextQuest-Ghidra`.
3. Update the Ghidra handoff docs and backlog notes.
4. Refresh `TextQuest` links so operators can find the current snapshot and baseline quickly.

## What `TextQuest` should reference

- the current snapshot manifest
- the current baseline-selection file
- the current Ghidra workflow page

## What `TextQuest` should not duplicate

- immutable snapshot payloads
- copied manifest trees
- baseline-selection history
```

- [ ] **Step 2: Replace repeated boundary paragraphs with one short pointer block**

Use this replacement text in `README.md`, `docs/wiki/Home.md`, and `docs/wiki/Development-Workflow.md` wherever the repo split is explained:

```md
Canonical Ghidra evidence lives in the sibling `TextQuest-Ghidra` repo. Use `docs/wiki/Ghidra-Evidence-Workflow.md` for the patch-cycle contract and current snapshot pointers.
```

- [ ] **Step 3: Run doc-level greps to confirm the boundary now has one long-form home**

Run:

```bash
rg -n "TextQuest-Ghidra|Ghidra-Evidence-Workflow" README.md docs/wiki
```

Expected:

- the new workflow page is referenced from the wiki index and README
- no other doc still carries a long-form duplicate of the boundary explanation

- [ ] **Step 4: Commit the TextQuest doc sweep**

Run:

```bash
git add README.md docs/wiki/Home.md docs/wiki/Development-Workflow.md docs/wiki/Ghidra-Evidence-Workflow.md
git commit -m "docs: centralize Ghidra workflow guidance"
```

Expected: a single docs commit on `codex/textquest-ghidra-workflow-cleanup`.

### Task 2: Normalize the Ghidra backlog and handoff docs

**Files:**
- Modify: `/Users/maleick/Projects/TextQuest-Ghidra/README.md`
- Modify: `/Users/maleick/Projects/TextQuest-Ghidra/HANDOFF.md`

- [ ] **Step 1: Add a compact backlog-category section to `HANDOFF.md`**

Use this exact section shape:

```md
## Backlog Categories

- active research and analysis
- tooling and helper scripts
- doc/audit follow-up
- governance and handoff

GitHub issues remain the active execution queue. `feature-list.json` remains the compact ledger for completed canonical workflow milestones.
```

- [ ] **Step 2: Add one short patch-cycle reminder to `README.md`**

Use this exact paragraph near the existing canonical model text:

```md
Every patch cycle starts with Ghidra intake and snapshot promotion here, then TextQuest is updated with the current snapshot and baseline pointers.
```

- [ ] **Step 3: Create a compact label taxonomy for backlog scanning**

Run:

```bash
gh label create roadmap:research --repo Maleick/TextQuest-Ghidra --color 7057ff --description "Active Ghidra research work"
gh label create roadmap:tooling --repo Maleick/TextQuest-Ghidra --color 0e8a16 --description "Helper scripts and tooling"
gh label create roadmap:docs --repo Maleick/TextQuest-Ghidra --color 0075ca --description "Docs and workflow cleanup"
gh label create roadmap:governance --repo Maleick/TextQuest-Ghidra --color 5319e7 --description "Handoff and repo-contract work"
```

Expected:

- the repo now has four custom roadmap labels alongside the built-in GitHub labels
- the labels are narrow enough to scan quickly but broad enough to cover the current open work

- [ ] **Step 4: Apply labels to the open backlog**

Use the current open issue set as the first labeling pass:

```bash
gh issue edit 4 --repo Maleick/TextQuest-Ghidra --add-label roadmap:docs
gh issue edit 10 --repo Maleick/TextQuest-Ghidra --add-label roadmap:tooling
gh issue edit 17 --repo Maleick/TextQuest-Ghidra --add-label roadmap:research
gh issue edit 18 --repo Maleick/TextQuest-Ghidra --add-label roadmap:research
gh issue edit 19 --repo Maleick/TextQuest-Ghidra --add-label roadmap:research
gh issue edit 21 --repo Maleick/TextQuest-Ghidra --add-label roadmap:tooling
gh issue edit 22 --repo Maleick/TextQuest-Ghidra --add-label roadmap:tooling
gh issue edit 23 --repo Maleick/TextQuest-Ghidra --add-label roadmap:tooling
gh issue edit 24 --repo Maleick/TextQuest-Ghidra --add-label roadmap:tooling
gh issue edit 25 --repo Maleick/TextQuest-Ghidra --add-label roadmap:tooling
gh issue edit 26 --repo Maleick/TextQuest-Ghidra --add-label roadmap:research
gh issue edit 27 --repo Maleick/TextQuest-Ghidra --add-label roadmap:research
```

Expected:

- docs/audit work is easy to spot
- tooling work is grouped
- research work is grouped
- the backlog remains open where work is still active

- [ ] **Step 5: Commit the Ghidra handoff doc sweep**

Run:

```bash
git -C /Users/maleick/Projects/TextQuest-Ghidra add README.md HANDOFF.md
git -C /Users/maleick/Projects/TextQuest-Ghidra commit -m "docs: normalize Ghidra workflow backlog"
```

Expected: a docs-only commit in `TextQuest-Ghidra` that does not change snapshots or baseline-selection.

### Task 3: Resolve the governance issue and PR

**Files:**
- GitHub issue `Maleick/TextQuest-Ghidra#4`
- GitHub pull request `Maleick/TextQuest-Ghidra#29`

- [ ] **Step 1: Link issue #4 to the new workflow page and close it**

Run:

```bash
gh issue comment 4 --repo Maleick/TextQuest-Ghidra --body "The unpublished landing-page draft is now surfaced as the Ghidra workflow page in TextQuest: https://github.com/Maleick/TextQuest/blob/master/docs/wiki/Ghidra-Evidence-Workflow.md"
gh issue close 4 --repo Maleick/TextQuest-Ghidra
```

Expected:

- issue #4 has a clear closure note
- the new workflow page is the published replacement for the unpublished draft

- [ ] **Step 2: Merge PR #29 after confirming it only records the archive-only handoff decision**

Run:

```bash
gh pr view 29 --repo Maleick/TextQuest-Ghidra --json number,title,state,body
gh pr merge 29 --repo Maleick/TextQuest-Ghidra --squash
```

Expected:

- PR #29 is merged cleanly with a squash merge
- the repo keeps the governance note that PRs #14/#15 are archive-only references

- [ ] **Step 3: Verify that no open governance item remains**

Run:

```bash
gh issue list --repo Maleick/TextQuest-Ghidra --state open --json number,title | rg '(^| )4$'
gh pr list --repo Maleick/TextQuest-Ghidra --state open --json number,title | rg '29'
```

Expected:

- issue #4 is gone from the open issue list
- PR #29 is gone from the open PR list

- [ ] **Step 4: Commit any final docs-only follow-up in TextQuest if the workflow page needs a link back from the wiki index**

If the issue closure or PR merge exposes a missing link path, update `docs/wiki/Home.md` and commit the tiny follow-up immediately:

```bash
git add docs/wiki/Home.md
git commit -m "docs: refresh wiki index after Ghidra cleanup"
```

### Task 4: Final repository verification

**Files:**
- `README.md`
- `docs/wiki/`
- `/Users/maleick/Projects/TextQuest-Ghidra/README.md`
- `/Users/maleick/Projects/TextQuest-Ghidra/HANDOFF.md`
- `/Users/maleick/Projects/TextQuest-Ghidra/feature-list.json`

- [ ] **Step 1: Confirm the boundary text is now concentrated**

Run:

```bash
rg -n "Canonical Ghidra evidence|Ghidra Evidence Workflow|TextQuest-Ghidra" README.md docs/wiki
```

Expected:

- a single workflow page carries the detailed contract
- other docs only carry short pointer text

- [ ] **Step 2: Confirm the Ghidra backlog is categorized**

Run:

```bash
gh issue list --repo Maleick/TextQuest-Ghidra --state open --json number,title,labels
```

Expected:

- each open issue has at least one roadmap label
- active work is still open
- governance items are resolved

- [ ] **Step 3: Validate the canonical ledger remains intact**

Run:

```bash
python3 -m json.tool /Users/maleick/Projects/TextQuest-Ghidra/feature-list.json >/dev/null
```

Expected: valid JSON, with the existing completed-only ledger preserved.

- [ ] **Step 4: Check both worktrees for cleanliness**

Run:

```bash
git status --short --branch
git -C /Users/maleick/Projects/TextQuest-Ghidra status --short --branch
```

Expected:

- both repos are clean except for intentionally committed follow-ups
- no stray edits remain before handoff
