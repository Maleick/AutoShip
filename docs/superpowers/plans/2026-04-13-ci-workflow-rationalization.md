# CI Workflow Rationalization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Execution note:** This plan has been completed and verified. The checklist remains as an implementation record, not an open task list.

**Goal:** Reduce failing GitHub Actions runs by pruning non-essential workflows, simplifying the PR merge gate, and moving Windows/full-pipeline validation out of the routine PR path.

**Architecture:** Treat `.github/workflows/` as three lanes: one direct Linux-based merge gate in `ci.yml`, one nightly/manual Windows validation lane in `nightly-release.yml`, and a reduced repo-automation surface for labels, docs publication, and agent helpers. Lock the new contract down with a small Python test that inspects workflow files as text so workflow sprawl and polling logic cannot quietly return.

**Tech Stack:** GitHub Actions YAML, Python `unittest`, repo docs in `docs/wiki/` and `docs/README.md`

---

## File Structure

- Create: `tests/test_workflow_contract.py`
  Verifies the rationalized workflow inventory and key CI invariants.

- Modify: `.github/workflows/ci.yml`
  Replace the current trusted/untrusted/polling structure with one direct merge-gate workflow plus direct secret scanning.

- Modify: `.github/workflows/nightly-release.yml`
  Keep Windows release-style validation here and make the nightly/manual role explicit.

- Modify: `.github/workflows/wiki-nightly.yml`
  Retain publication only behind successful nightly validation or manual dispatch.

- Modify: `.github/workflows/automation.yml`
  Remove merge-dispatch wiki coupling and keep only issue/PR lifecycle automation that still matters.

- Delete: `.github/workflows/fmt-autofix.yml`
- Delete: `.github/workflows/branch-cleanup.yml`
- Delete: `.github/workflows/readme-metrics.yml`
- Delete: `.github/workflows/copilot-ci-dispatch.yml`

- Modify: `docs/wiki/Development-Workflow.md`
  Document the new three-lane workflow model and the lighter merge gate.

- Modify: `docs/wiki/Maintaining-the-Wiki.md`
  Update wiki publication expectations to match the retained workflow behavior.

- Modify: `docs/README.md`
  Refresh the docs/build workflow references so the repo index matches reality.

- Modify: `docs/dev/Branch-Cleanup-Strategy.md`
  Mark the old cleanup workflow historical-only or remove references that claim it is active.

- Modify: `feature-list.json`
  Add or update a workflow-rationalization feature entry so the multi-session ledger exists and stays current.

## Task 1: Add a workflow contract test and feature ledger

**Files:**
- Create: `tests/test_workflow_contract.py`
- Create or Modify: `feature-list.json`

- [ ] **Step 1: Write the failing workflow contract test**

```python
from __future__ import annotations

from pathlib import Path
import json
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = REPO_ROOT / ".github" / "workflows"


class WorkflowContractTests(unittest.TestCase):
    def read(self, name: str) -> str:
        return (WORKFLOWS / name).read_text(encoding="utf-8")

    def test_pruned_workflows_are_absent(self) -> None:
        for name in [
            "fmt-autofix.yml",
            "branch-cleanup.yml",
            "readme-metrics.yml",
            "copilot-ci-dispatch.yml",
        ]:
            self.assertFalse((WORKFLOWS / name).exists(), f"{name} should be removed")

    def test_ci_no_longer_contains_polling_gate_or_windows_pr_path(self) -> None:
        text = self.read("ci.yml")
        self.assertNotIn("Wait for runner-specific gate result", text)
        self.assertNotIn("listJobsForWorkflowRun", text)
        self.assertNotIn("PR gate (trusted path)", text)
        self.assertNotIn("PR gate (fork PR path)", text)
        self.assertNotIn("runs-on: [self-hosted, Windows, X64, textquest]", text)
        self.assertIn("ubuntu-latest", text)

    def test_nightly_release_remains_present(self) -> None:
        text = self.read("nightly-release.yml")
        self.assertIn("workflow_dispatch", text)
        self.assertIn("schedule:", text)
        self.assertIn("Build rolling nightly prerelease", text)

    def test_feature_list_tracks_workflow_rationalization(self) -> None:
        data = json.loads((REPO_ROOT / "feature-list.json").read_text(encoding="utf-8"))
        ids = {entry["id"] for entry in data["features"]}
        self.assertIn("ci-workflow-rationalization", ids)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the new test and verify it fails against the current repo**

Run: `python3 -m unittest tests.test_workflow_contract -v`

Expected: FAIL because the four pruned workflows still exist, `ci.yml` still contains the polling gate, and `feature-list.json` does not exist yet.

- [ ] **Step 3: Create the feature ledger with the workflow entry**

```json
{
  "features": [
    {
      "id": "ci-workflow-rationalization",
      "name": "CI workflow rationalization",
      "status": "in_progress",
      "acceptance_tests": [
        "python3 -m unittest tests.test_workflow_contract -v",
        "python3 scripts/sync_wiki.py --check"
      ],
      "notes": "Reduce failing workflow runs by pruning maintenance workflows, simplifying ci.yml, and moving Windows validation to nightly/manual lanes."
    }
  ]
}
```

- [ ] **Step 4: Re-run the test to confirm only the intended workflow assertions still fail**

Run: `python3 -m unittest tests.test_workflow_contract -v`

Expected: FAIL only on the workflow-shape assertions until the YAML changes land.

- [ ] **Step 5: Commit the test and feature ledger scaffold**

```bash
git add tests/test_workflow_contract.py feature-list.json
git commit -m "test: add CI workflow contract coverage"
```

## Task 2: Prune non-essential maintenance workflows

**Files:**
- Delete: `.github/workflows/fmt-autofix.yml`
- Delete: `.github/workflows/branch-cleanup.yml`
- Delete: `.github/workflows/readme-metrics.yml`
- Delete: `.github/workflows/copilot-ci-dispatch.yml`
- Modify: `docs/dev/Branch-Cleanup-Strategy.md`

- [ ] **Step 1: Delete the four non-essential maintenance workflows**

```bash
rm .github/workflows/fmt-autofix.yml
rm .github/workflows/branch-cleanup.yml
rm .github/workflows/readme-metrics.yml
rm .github/workflows/copilot-ci-dispatch.yml
```

- [ ] **Step 2: Update the branch-cleanup strategy doc so it no longer claims the automation is active**

Use this replacement block near the top of `docs/dev/Branch-Cleanup-Strategy.md`:

```md
> Historical note: `.github/workflows/branch-cleanup.yml` was removed during CI workflow rationalization on 2026-04-13. This document remains as reference only and does not describe an active GitHub Actions workflow.
```

- [ ] **Step 3: Run the workflow contract test**

Run: `python3 -m unittest tests.test_workflow_contract -v`

Expected: the pruned-workflow assertion passes; `ci.yml` shape assertions still fail until Task 3 is complete.

- [ ] **Step 4: Inspect the workflow directory**

Run: `ls .github/workflows`

Expected: `fmt-autofix.yml`, `branch-cleanup.yml`, `readme-metrics.yml`, and `copilot-ci-dispatch.yml` are absent.

- [ ] **Step 5: Commit the pruning pass**

```bash
git add .github/workflows docs/dev/Branch-Cleanup-Strategy.md
git commit -m "ci: prune non-essential maintenance workflows"
```

## Task 3: Replace `ci.yml` with one direct Linux merge gate

**Files:**
- Modify: `.github/workflows/ci.yml`
- Test: `tests/test_workflow_contract.py`

- [ ] **Step 1: Replace the PR workflow header and job outline in `ci.yml`**

Use this structure at the top of `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  pull_request:
    branches: [master]
  push:
    branches: [master]
  workflow_dispatch:
    inputs:
      run_release_build:
        description: "Also run the Windows nightly-style build after CI"
        required: false
        default: false
        type: boolean

concurrency:
  group: ci-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true

permissions:
  contents: read

env:
  CARGO_TERM_COLOR: always

jobs:
  merge_gate:
    name: Merge gate
    runs-on: ubuntu-latest
    timeout-minutes: 30
```

- [ ] **Step 2: Replace the current PR validation steps with one direct Linux validation sequence**

Use this step body under `merge_gate`:

```yaml
    steps:
      - uses: actions/checkout@v4

      - name: Setup Python
        uses: actions/setup-python@v5
        with:
          python-version: "3.12"

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@nightly
        with:
          components: rustfmt, clippy

      - name: Cache cargo registry
        uses: Swatinem/rust-cache@v2

      - name: Validate wiki sources
        run: python3 scripts/sync_wiki.py --check

      - name: Run Python tests
        run: python3 -m unittest discover -s tests -p 'test_*.py' -v

      - name: Check formatting
        run: cargo fmt --all --check

      - name: Run clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Run Rust tests
        run: cargo test --all --all-features
```

- [ ] **Step 3: Replace the existing secret scan jobs with one direct hosted job**

Append this job below `merge_gate`:

```yaml
  secrets_scan:
    name: Secret scan
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: TruffleHog OSS
        uses: trufflesecurity/trufflehog@6bd2d14f7a4bc1e569fa3550efa7ec632a4fa67b
        with:
          extra_args: --only-verified
```

- [ ] **Step 4: Keep the manual Windows build optional and downstream**

Retain the `windows` job, but make it depend on `merge_gate` instead of the removed polling job:

```yaml
  windows:
    name: Windows release build (manual)
    if: github.event_name == 'workflow_dispatch' && inputs.run_release_build
    needs: merge_gate
```

- [ ] **Step 5: Run the workflow contract test and inspect the new CI file**

Run: `python3 -m unittest tests.test_workflow_contract -v`

Expected: PASS for the `ci.yml` shape assertions.

Run: `rg -n "Wait for runner-specific gate result|listJobsForWorkflowRun|PR gate \\(trusted path\\)|PR gate \\(fork PR path\\)" .github/workflows/ci.yml`

Expected: no output.

- [ ] **Step 6: Commit the CI simplification**

```bash
git add .github/workflows/ci.yml tests/test_workflow_contract.py
git commit -m "ci: simplify merge gate"
```

## Task 4: Re-scope nightly validation and docs publication

**Files:**
- Modify: `.github/workflows/nightly-release.yml`
- Modify: `.github/workflows/wiki-nightly.yml`
- Modify: `.github/workflows/automation.yml`

- [ ] **Step 1: Rename the nightly workflow’s gate job so the purpose is obvious**

Replace the top job heading in `.github/workflows/nightly-release.yml` with:

```yaml
jobs:
  run_window:
    name: Nightly/manual run window
```

Keep the existing local-time behavior, but update job names and comments so the workflow clearly reads as the full-validation lane rather than a general CI path.

- [ ] **Step 2: Remove wiki dispatch coupling from `automation.yml`**

Delete the entire `sync-wiki` job from `.github/workflows/automation.yml`, starting from:

```yaml
  sync-wiki:
    name: Trigger wiki publish after merge
```

through the end of that job block.

- [ ] **Step 3: Keep wiki publication behind nightly success or manual dispatch only**

Retain this guard in `.github/workflows/wiki-nightly.yml`:

```yaml
  publish-wiki:
    name: Publish wiki snapshot
    if: github.event_name != 'workflow_run' || github.event.workflow_run.conclusion == 'success'
```

and update the workflow name to make the role explicit:

```yaml
name: Wiki Publish
```

- [ ] **Step 4: Verify the remaining workflow surface**

Run: `ls .github/workflows`

Expected: only `automation.yml`, `ci.yml`, `claude-agent.yml`, `docs-pages.yml`, `nightly-release.yml`, `release.yml`, and `wiki-nightly.yml` remain.

Run: `python3 -m unittest tests.test_workflow_contract -v`

Expected: PASS.

- [ ] **Step 5: Commit the nightly/docs automation changes**

```bash
git add .github/workflows/automation.yml .github/workflows/nightly-release.yml .github/workflows/wiki-nightly.yml
git commit -m "ci: separate nightly validation from repo automation"
```

## Task 5: Update developer and wiki-facing docs to match the new workflow model

**Files:**
- Modify: `docs/wiki/Development-Workflow.md`
- Modify: `docs/wiki/Maintaining-the-Wiki.md`
- Modify: `docs/README.md`
- Modify: `feature-list.json`

- [ ] **Step 1: Add the three-lane workflow summary to `docs/wiki/Development-Workflow.md`**

Insert this block near the CI/workflow guidance:

```md
## CI lanes

- `ci.yml` is the only routine merge gate. It runs Linux-based wiki validation, Python tests, Rust format/lint/test, and secret scanning.
- `nightly-release.yml` is the broader Windows/manual validation lane. Use it for release-like confidence and patch-sensitive pipeline checks.
- Repo automation workflows are operational helpers, not product-health signals. Failures there should be triaged separately from merge safety.
```

- [ ] **Step 2: Update `docs/wiki/Maintaining-the-Wiki.md` to match the retained publication flow**

Replace the CI expectation block with:

```md
## CI and PR Expectations

- PR CI runs `python3 scripts/sync_wiki.py --check`.
- GitHub Pages publication is handled separately from the merge gate.
- If wiki mirror publication is retained, it runs only after successful nightly validation or by manual dispatch.
```

- [ ] **Step 3: Update `docs/README.md` workflow references**

Use this replacement list in the workflow section:

```md
- Wiki source sync validation: `scripts/sync_wiki.py`
- Merge gate workflow: `.github/workflows/ci.yml`
- Public docs site workflow: `.github/workflows/docs-pages.yml`
- Nightly/manual wiki publication workflow: `.github/workflows/wiki-nightly.yml`
```

- [ ] **Step 4: Mark the feature complete in `feature-list.json`**

Change the workflow entry to:

```json
{
  "id": "ci-workflow-rationalization",
  "name": "CI workflow rationalization",
  "status": "complete",
  "acceptance_tests": [
    "python3 -m unittest tests.test_workflow_contract -v",
    "python3 scripts/sync_wiki.py --check"
  ],
  "notes": "Pruned non-essential workflows, simplified ci.yml into a direct Linux merge gate, and moved Windows/full-pipeline validation into nightly/manual lanes."
}
```

- [ ] **Step 5: Run docs and workflow verification**

Run: `python3 -m unittest tests.test_workflow_contract -v`

Expected: PASS.

Run: `python3 scripts/sync_wiki.py --check`

Expected: PASS.

- [ ] **Step 6: Commit the docs and ledger updates**

```bash
git add docs/wiki/Development-Workflow.md docs/wiki/Maintaining-the-Wiki.md docs/README.md feature-list.json
git commit -m "docs: document rationalized CI workflow model"
```

## Task 6: Final verification and review artifact

**Files:**
- Modify: `feature-list.json` if verification reveals any incomplete acceptance criteria

- [ ] **Step 1: Review the surviving workflow inventory**

Run: `ls .github/workflows`

Expected:

```text
automation.yml
ci.yml
claude-agent.yml
docs-pages.yml
nightly-release.yml
release.yml
wiki-nightly.yml
```

- [ ] **Step 2: Run the workflow contract test suite**

Run: `python3 -m unittest tests.test_workflow_contract -v`

Expected: PASS.

- [ ] **Step 3: Run wiki validation**

Run: `python3 scripts/sync_wiki.py --check`

Expected: PASS.

- [ ] **Step 4: Inspect the final diff**

Run: `git diff --stat origin/master...HEAD`

Expected: shows workflow deletions, `ci.yml` simplification, nightly/wiki automation cleanup, docs updates, and the new workflow contract test.

- [ ] **Step 5: Create the final integration commit**

```bash
git add .github/workflows tests/test_workflow_contract.py docs/wiki/Development-Workflow.md docs/wiki/Maintaining-the-Wiki.md docs/README.md docs/dev/Branch-Cleanup-Strategy.md feature-list.json
git commit -m "ci: rationalize workflow surface"
```
