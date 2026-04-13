# TextQuest-Ghidra Workflow Completion Design Spec

## Summary

Keep `TextQuest` and `TextQuest-Ghidra` as separate private repos. Do not merge them and do not convert `TextQuest-Ghidra` into a submodule.

The cleanup goal is not to empty the Ghidra backlog. The goal is to make the repo boundary and the patch-cycle workflow explicit, so `TextQuest` can depend on canonical Ghidra evidence without duplicating that evidence locally.

This design makes `TextQuest` the single operator/docs surface and keeps `TextQuest-Ghidra` as the canonical repository for immutable snapshots, baselines, and Ghidra-side research/workflow state.

## Current State

- `TextQuest` already points at `TextQuest-Ghidra` as the canonical evidence repo in `README.md`, `docs/wiki/Home.md`, `docs/wiki/Development-Workflow.md`, and related wiki pages.
- `TextQuest-Ghidra` already has the canonical evidence model in place: `snapshots/`, `baseline-selection/`, `HANDOFF.md`, `WINDOWS_HANDOFF.md`, and `feature-list.json`.
- The Ghidra repo is still active, not archival. As of this session it has 12 open issues and 1 open PR.
- The open backlog is mixed:
  - active research and analysis work
  - helper/tooling work
  - one audit / doc-verification item
  - one governance/handoff PR
- `feature-list.json` in `TextQuest-Ghidra` already shows the canonical workflow milestones as complete, so it should remain a compact ledger rather than becoming a second roadmap system.

## Design

### 1. Make the repo boundary explicit and centralized

`TextQuest` becomes the single place where operators learn how the two repos relate.

Planned changes on the `TextQuest` side:

- Add or update one dedicated wiki page for the Ghidra evidence workflow. The page should explain:
  - `TextQuest-Ghidra` is canonical for immutable snapshots, manifests, and baseline selection.
  - `TextQuest` is canonical for code, docs, runbooks, automation, and lightweight pointers.
  - every patch cycle starts in `TextQuest-Ghidra` and only then gets reflected back into `TextQuest`.
- Trim repeated boundary wording from scattered docs so there is one authoritative explanation instead of several slightly different ones.
- Keep links to current Ghidra snapshot and baseline paths in `TextQuest`, but do not copy canonical evidence payloads into this repo.

### 2. Normalize the Ghidra backlog without changing the repo shape

`TextQuest-Ghidra` keeps GitHub issues as the active execution queue.
`feature-list.json` stays the summary ledger for completed canonical workflow milestones.

Backlog cleanup will follow these rules:

- keep issues open when they represent active research, active tooling, or unresolved workflow work
- rewrite issue titles or bodies when the next action is not obvious
- close only issues that are stale, duplicated, or fully absorbed by the new workflow docs
- keep PRs focused on one governance or workflow outcome at a time

The current open backlog should be categorized into these buckets:

- active research and analysis
- tooling and helper scripts
- doc/audit follow-up
- governance and handoff

This is a normalization pass, not a forced shutdown of the research queue.

### 3. Define a patch-cycle operating model

Every patch cycle should follow the same sequence:

1. Update or verify the intake and analysis state in `TextQuest-Ghidra`.
2. Promote or refresh the canonical snapshot and baseline selection in `TextQuest-Ghidra`.
3. Update the Ghidra repo handoff docs and backlog notes so the current state is obvious.
4. Refresh the `TextQuest` wiki/docs pointers to the current canonical snapshot and baseline.

The rule is simple: `TextQuest` references the evidence; `TextQuest-Ghidra` owns the evidence.

## Files and Surfaces

Likely `TextQuest` changes:

- `README.md`
- `docs/wiki/Home.md`
- `docs/wiki/Development-Workflow.md`
- a dedicated Ghidra workflow page under `docs/wiki/`

Likely `TextQuest-Ghidra` changes:

- `README.md`
- `HANDOFF.md`
- selected issue bodies / PR metadata
- `feature-list.json` only if a short workflow note is needed

## Verification

The cleanup is correct when these checks pass:

- `TextQuest` has one clear boundary statement for the Ghidra dependency.
- `TextQuest` points at canonical Ghidra evidence paths instead of duplicating evidence payloads.
- `TextQuest-Ghidra` documents the patch-cycle workflow in a way that is easy to follow after each game update.
- `feature-list.json` still reflects completed canonical milestones only.
- the Ghidra backlog is clearly categorized, with no uncategorized open item left behind.
- PR #29 is resolved.
- no `.gitmodules` file or submodule wiring is introduced.

## Assumptions

- The separate-repo structure stays permanent.
- GitHub issues remain the source of truth for active Ghidra work.
- `feature-list.json` remains a compact ledger, not the primary roadmap.
- "Complete" means workflow-complete and boundary-complete, not backlog-empty.
