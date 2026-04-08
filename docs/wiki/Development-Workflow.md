# Development Workflow

## Daily Working Loop

### 1. Use reference trees only when needed

`third_party/eqlib` and `third_party/macroquest` are optional local reference paths, not required bootstrap steps.

Do it before:

- offset work
- eqlib or MacroQuest reference lookups
- struct-field investigations
- upstream behavior comparisons

## Build and Test

Use the normal Rust flow from the repo root:

```bash
cargo build
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

For release validation on Windows:

```bash
cargo build --release
```

## Recommended Working Modes

### UI and orchestration changes

- prefer demo mode first
- iterate with `cargo run`
- confirm the five main screens, command bar, and scope controls

### Injection, packet, zoning, login, and live combat changes

- use Windows
- validate against live EQ clients
- capture logs and state what was and was not revalidated

## Roadmap and Research Workflow

The roadmap source of truth is `docs/implementation-roadmap.md`.

Supporting research surfaces live in:

- `docs/external-research/`
- `docs/research-imports/`

Current packet/zoning intake rule:

- curate send-path, state, and validation conclusions into `docs/external-research/packet-zoning-send-path-and-state-ledger.md` before treating raw imports as roadmap-ready

Rules:

- external research may add milestone slices, validation tasks, and evidence updates
- external research may not reorder milestones on its own
- use evidence states when promoting research into execution work
- keep the GitHub Project mirror in sync only after repo docs are updated
- keep milestone epics as draft items, but promote mature research, task, and validation items into GitHub issues
- remove overlapping draft items after an issue promotion so the project has one execution item per slice
- use the live roadmap mirror at `https://github.com/users/Maleick/projects/1`

Default roadmap verifier:

- `python scripts/validate_roadmap_unknowns.py --plan docs/implementation-roadmap.md --domains packet,zoning,anticheat`

## Documentation Expectations

The repo treats `docs/wiki/` as the canonical source for the GitHub wiki.

When behavior or roadmap guidance changes:

1. update the code or source docs
2. update the matching page in `docs/wiki/`
3. run `python scripts/sync_wiki.py --check`
4. run `python scripts/sync_wiki.py --dry-run` before a manual publish
5. include the wiki source changes in the same PR when possible

## Nightly Automation

Nightly automation now runs across the self-hosted Windows runner and GitHub-hosted Linux jobs:

- `.github/workflows/wiki-nightly.yml` publishes the wiki snapshot
- `.github/workflows/nightly-release.yml` builds and refreshes the rolling nightly prerelease
- `.github/workflows/copilot-ci-dispatch.yml` sweeps open same-repo Copilot PRs from `master`, dispatches `CI` when the PR-triggered run is stuck in approval, and skips PRs that edit workflow files so those still require manual review
- `.github/workflows/agent-ready.yml` keeps `agent:ready` vs `agent:skip-ready` aligned on issue events plus an hourly sweep, suppresses `agent:ready` when an issue already has an open linked PR or active `agent:working` / `agent:blocked` state, bootstraps those labels when missing, and treats roadmap-container titles that start with `M<number>` or `Mx` as skip-ready
- `scripts/reconcile-agent-queue.sh` plus the scheduled TextQuest issue-queue reconciler automation add missing open issues to the roadmap project, set `Agent Status`, clean stale `agent:ready` / `agent:working` labels off non-ready items, and promote every other open non-epic issue to `Ready for Agent`
- `.github/workflows/agent-close-pr.yml` closes only agent-authored PRs when they carry the `agent:close` label and the PR is agent-owned via a `codex/*` or `claude/*` head branch or the literal `codex-automation` label
- the external-research Codex automation follows those workflows and can sync the roadmap mirror after the repo docs are current (currently paused — Codex quota exhausted until April 8, 2026)
- the issue executor opens trusted agent PRs with `merge:auto` by default unless the PR or linked issue is marked `human:required`, `risk:high`, or `agent:blocked`
- the PR manager may resolve clearly addressed bot review threads, merge clean trusted PRs once the required gate is green, and close stale or superseded trusted agent PRs automatically
- as of 2026-04-03, Claude Code is the primary active agent worker; Codex automations are paused

These workflows mirror repo state. They do not replace keeping source docs current.

Nightly sync order:

1. source docs and research ledgers
2. roadmap verifier and wiki guard
3. GitHub issues for mature checkpoint items
4. GitHub Project mirror fields and cards

## Logging and Debugging

Primary log locations:

- `logs/textquest.log`
- `%TEMP%/textquest/textquest-dll.log`

When debugging IPC or injection:

- verify the session token files exist
- verify the DLL log is updating
- verify `status` or `status-all` can read live shared memory

## Generated and Derived Files

Do not hand-edit generated sources without also updating the generator flow:

- `HANDOFF.md` is generated by `scripts/gen-handoff.sh`
- README metrics are refreshed by `scripts/update_readme_metrics.py`

## Internals and Reference Discipline

- prefer current code over older design docs when there is a conflict
- prefer `third_party/eqlib` for eqlib references when that local tree is available
- use `third_party/macroquest` for broader upstream context such as login, routing, and scripting behavior when that local tree is available
- treat `third_party/macroquest/src/eqlib` as vendored upstream context, not the primary TextQuest eqlib citation path
- use MacroQuest docs, RedGuides docs, and public comparison repos as roadmap inputs, not as proof that TextQuest already implements a feature

## Current Behavior vs Roadmap

### Current behavior

- demo mode is the standard inner loop for non-Windows development
- CI validates wiki structure with `python scripts/sync_wiki.py --check`

### Validation notes

- passing tests on macOS does not validate Windows-only injection, packet, zoning, or login paths
- offset and widget-sensitive changes still require live-client verification after EQ updates
