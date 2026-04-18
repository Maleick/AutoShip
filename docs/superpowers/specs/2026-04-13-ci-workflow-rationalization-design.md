# TextQuest CI Workflow Rationalization Design

Date: 2026-04-13
Status: Complete
Primary objective: Fewer failing workflow runs
Secondary objectives: Preserve trustworthy product validation, improve PR feedback time, reduce workflow sprawl

## Problem

TextQuest currently has too many GitHub Actions workflows doing materially different jobs under the broad umbrella of "CI." Product validation, release packaging, docs publishing, PR/issue automation, formatting autofix, branch cleanup, and agent dispatch are all active at once. This increases failure noise, makes it harder to tell whether the product is broken or the automation is broken, and slows confidence-building on PRs.

The current `ci.yml` is also structurally complex. It contains separate trusted and untrusted validation jobs plus a third job that polls the workflow API waiting for one of those jobs to finish. That orchestration introduces avoidable failure modes unrelated to code correctness.

## Goals

- Reduce the number of failing workflow runs across both PR validation and scheduled automation.
- Preserve one trustworthy merge gate that answers "is this safe to merge?"
- Preserve a broader end-to-end validation lane for the AI-heavy pipeline without forcing it onto every PR.
- Remove or downgrade workflows whose failures do not protect product quality.

## Non-Goals

- Maximize per-PR coverage at any cost.
- Preserve every current workflow unchanged.
- Solve unrelated application test failures inside this design.

## Current Workflow Inventory

### Product validation and release

- `ci.yml`
- `nightly-release.yml`
- `release.yml`

### Docs publishing

- `docs-pages.yml`
- `wiki-nightly.yml`

### Repository automation and maintenance

- `automation.yml`
- `branch-cleanup.yml`
- `readme-metrics.yml`
- `fmt-autofix.yml`
- `copilot-ci-dispatch.yml`
- `claude-agent.yml`

## Target Topology

The workflow set should be organized into three lanes with different reliability expectations.

### 1. Merge Gate

Purpose: answer only whether a PR is safe to merge.

Requirements:

- Small enough to be reliable.
- Fast enough to provide useful PR feedback.
- Independent of self-hosted Windows where possible.
- Free of orchestration-only jobs such as polling for sibling job status.

Expected checks in this lane:

- wiki source validation
- Python unit tests
- Rust format check
- Rust clippy
- Rust tests
- secret scan

Design rule: if a workflow step does not improve merge safety directly, it does not belong here.

### 2. Nightly or Manual Full Validation

Purpose: prove the broader AI pipeline and Windows/release-like build surface still work.

Requirements:

- May use self-hosted Windows.
- May run longer and include heavier checks.
- Failure is actionable but should not block routine merging.
- Manual dispatch must be available for operator confidence before risky merges or releases.

Expected scope:

- Windows release build
- release-like packaging verification
- broader integration checks when they are stable enough to provide real signal
- wiki publication only after successful nightly validation, if wiki publication is retained at all

### 3. Repo Automation

Purpose: issue/PR metadata hygiene, docs/site publishing, and agent workflow support.

Requirements:

- Must not be confused with product health.
- Should be minimized.
- Scheduled maintenance that fails often should be deleted or made manual-only.

## Per-Workflow Decisions

### Keep

- `release.yml`
  Keep as the tagged release workflow. It is already scoped to version tags and does not add routine CI noise.

- `docs-pages.yml`
  Keep if GitHub Pages remains a real operator-facing surface. It is narrow and isolated.

- `claude-agent.yml`
  Keep as an agent automation, not a CI signal. Its failures should not be treated as product regressions.

### Keep but Simplify or Re-scope

- `ci.yml`
  Retain as the single merge gate, but redesign it into a direct validation workflow with no polling gate job.

- `nightly-release.yml`
  Retain as the nightly/manual full-validation workflow. Remove any implication that it is part of the routine PR path.

- `automation.yml`
  Retain only the issue/PR lifecycle automation that materially supports the repo workflow. Treat it as repo automation rather than CI.

- `wiki-nightly.yml`
  Retain only if wiki publication remains required. Prefer manual or post-validation publication over a dispatch chain that is hard to reason about.

### Combine or Absorb

- `fmt-autofix.yml`
  Remove as a standalone workflow. Formatting should fail inside the merge gate instead of racing a bot autofix commit against PR validation.

- `copilot-ci-dispatch.yml`
  Remove if `ci.yml` becomes reliable and directly triggerable. This workflow exists primarily to compensate for CI trigger complexity.

- `automation.yml` `sync-wiki` behavior plus `wiki-nightly.yml`
  Consolidate into one docs publication path. Avoid merge event -> dispatch -> second workflow chains when a single explicit publication workflow is enough.

### Prune

- `branch-cleanup.yml`
  Delete or convert to manual-only. It is maintenance automation with failure modes unrelated to product health.

- `readme-metrics.yml`
  Disable or delete unless there is a strong operator need to keep automated metrics refresh. It creates branches and PRs automatically and adds non-essential failure surface.

## Merge Gate Design

## Overview

`ci.yml` should become the only required PR workflow. It should run a direct sequence of checks instead of routing through trusted and untrusted Windows jobs plus a Linux polling job.

## Structure

- Trusted same-repo PR path:
  Run on self-hosted Linux.
- Fork PR path:
  If fork PRs are allowed, run on a read-only self-hosted Linux lane with the same or slightly reduced safe checks.
- Secret scan:
  Run as a direct sibling job on self-hosted Linux for both paths.

Remove:

- `pr_gate` polling/wait job
- dependency on self-hosted Windows for routine PR validation
- cross-job workflow API polling logic

## Why

This removes an entire class of failures where CI fails because it cannot correctly observe or wait on sibling jobs. It also decouples mergeability from self-hosted Windows runner stability.

## Nightly and Manual Full Validation Design

The nightly lane should become the place where Windows-specific and release-like confidence lives.

Recommended behavior:

- Scheduled nightly run on default branch.
- Manual dispatch for on-demand confidence before risky merges or release prep.
- Windows build and packaging verification.
- If wiki publication is retained, publish only after successful nightly validation.

This lane is explicitly allowed to be broader and slower than the merge gate. Its failure should create a maintenance issue instead of breaking every routine PR decision.

## Repo Automation Design

Repo automation should be treated as operational support and minimized.

Recommended posture:

- Keep `automation.yml` only for issue and PR label/state management that is actively used.
- Keep `claude-agent.yml` if agent execution through GitHub is part of the intended operating model.
- Keep `docs-pages.yml` if Pages is still needed.
- Remove branch cleanup, format autofix, Copilot CI dispatch, and README metrics automation unless a human explicitly wants to maintain them.

## Migration Plan

### Phase 1: Reduce failure surface immediately

- Remove `fmt-autofix.yml`
- Disable or delete `branch-cleanup.yml`
- Disable or delete `readme-metrics.yml`
- Disable or delete `copilot-ci-dispatch.yml`

Expected result: fewer non-product workflow failures with minimal effect on merge safety.

### Phase 2: Simplify merge gate

- Refactor `ci.yml` into one direct required PR workflow.
- Move routine PR validation off self-hosted Windows.
- Keep secret scanning direct and simple.

Expected result: fewer flaky PR runs and faster, more legible feedback.

### Phase 3: Re-scope nightly and docs publication

- Keep `nightly-release.yml` as the broader Windows/full-pipeline lane.
- If wiki publication remains required, attach it to successful nightly validation instead of using an indirect merge-dispatch chain.
- Remove indirect dispatch chains where possible.

Expected result: clearer operational ownership and less coupling between docs and release automation.

## Success Criteria

- The required PR workflow count is reduced to one merge gate plus optional secret scan sibling job inside the same workflow.
- Self-hosted Windows is no longer required for normal PR mergeability.
- At least three non-essential scheduled or bot workflows are removed, disabled, or downgraded to manual-only.
- Workflow failures become easier to classify as either product validation failures or operational automation failures.
- PR feedback becomes faster because merge safety no longer waits on Windows or workflow-internal polling.

## Risks

- Moving Windows out of the PR path can allow Windows-specific breakage to survive until nightly or manual validation.
  Mitigation: keep the nightly/manual lane healthy and easy to trigger.

- Removing bot autofix means developers must format locally or fix CI failures manually.
  Mitigation: this is a good trade if the repo prefers deterministic gates over bot churn.

- Disabling maintenance automations may reintroduce small manual chores.
  Mitigation: accept small manual overhead in exchange for lower CI noise unless a workflow proves operationally essential.

## Recommendation

Proceed with the three-lane model:

- one minimal required merge gate
- one nightly/manual full-validation lane
- one reduced repo-automation surface

This best matches the repo's stated objective of fewer failing workflow runs while preserving trustworthy validation for the broader AI pipeline.
