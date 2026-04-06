# TextQuest Autonomous Agent Pipeline

This document is the operator manual for the TextQuest issue queue that runs through Codex by default and Claude optionally.

## Roles

- Operator: creates issues, curates roadmap milestones, sets project state, and adds routing or risk labels.
- Codex issue executor: the default worker that claims one eligible issue, implements it, verifies it, and opens the PR.
- Codex PR manager: the only merge bot. It opens missing PRs for agent branches when needed, addresses straightforward review feedback, and merges only when policy allows it.
- Claude issue worker: optional worker that can claim and implement issues when routed with `worker:claude` or an explicit `@claude` mention, but never merges.
- Nightly research automation: separate from issue execution. It updates docs and roadmap materials first, then mirrors mature checkpoint work into GitHub Project items and issues.

## Queue Contract

GitHub Projects mirror repo truth. The repo remains authoritative, especially [`docs/implementation-roadmap.md`](implementation-roadmap.md).

An issue enters the autonomous queue only when both are true:

1. The issue is in the `TextQuest Roadmap` GitHub Project with `Agent Status = Ready for Agent`.
2. The issue has label `agent:ready`.

Expected `Agent Status` values:

- `Backlog`
- `Ready for Agent`
- `Agent Working`
- `PR Open`
- `Blocked`
- `Done`

Queue and policy labels:

- `agent:ready` — eligible for pickup
- `agent:working` — claimed by one worker
- `agent:blocked` — blocked and waiting on a human
- `mode:research` — use the docs-first research workflow
- `worker:claude` — route implementation to Claude instead of Codex
- `merge:auto` — allow unattended merge after all merge gates clear
- `human:required` — do not auto-merge
- `risk:high` — do not auto-merge

## Issue Lifecycle

1. Create the issue with [`.github/ISSUE_TEMPLATE/agent-task.yml`](../.github/ISSUE_TEMPLATE/agent-task.yml).
2. Fill in `Scope`, `Done when`, `Verify`, `Out of scope`, and `Source docs`.
3. Add the issue to the `TextQuest Roadmap` GitHub Project.
4. Leave the issue in `Backlog` until it is ready for hands-off execution.
5. When ready, set `Agent Status = Ready for Agent` and add `agent:ready`.
6. Optionally add:
   - `mode:research` for docs-first research work
   - `worker:claude` to route execution to Claude
   - `merge:auto` to allow unattended merge
   - `human:required` or `risk:high` to require human judgment before merge
7. The worker claims exactly one issue per run:
   - move `Agent Status` to `Agent Working`
   - replace `agent:ready` with `agent:working`
   - post a short claim comment
   - branch from `master`
8. After verification succeeds, the worker opens a non-draft PR into `master` and moves the issue `Agent Status` to `PR Open`.
9. If blocked, the worker moves the issue `Agent Status` to `Blocked`, adds `agent:blocked`, removes `agent:working`, and leaves a concrete unblock comment.
10. After merge, move the issue `Agent Status` to `Done` and clear any stale working or blocked labels.

## Worker Rules

Shared contract:

- Read [`AGENTS.md`](../AGENTS.md) and [`CLAUDE.md`](../CLAUDE.md) before editing.
- Default to implementation mode for concrete issues. The operator should not need to repeat “please code this now.”
- If `mode:research` is present, use the research-first workflow:
  - update roadmap or research docs first
  - run the roadmap or wiki guards
  - only mirror mature checkpoint work back into GitHub afterward
- If work spans multiple independent surfaces, parallelize internally and integrate before opening the PR.
- Never commit transient automation state or autoresearch runtime files.

Verification policy:

- Always run the issue’s `Verify` commands.
- If the diff touches Rust, config, or scripts, also run repo gate commands when feasible:
  - `cargo fmt --all --check`
  - `cargo test`
  - any directly relevant Python or wiki checks
- If the diff is docs, workflow, or prompt only, run lightweight syntax or parse checks plus `python3 scripts/sync_wiki.py --check`.

## Merge Policy

Only the shared Codex PR manager merges TextQuest pull requests.

The PR manager may merge or enable auto-merge only when all are true:

- base branch is `master`
- PR is non-draft
- required Windows `PR gate (fmt + clippy + test + python)` is green
- no unresolved review threads remain
- the PR or linked issue has `merge:auto`
- neither the PR nor linked issue has `human:required`, `risk:high`, or `agent:blocked`

If any of those conditions fail, the PR stays open and the blocker should be summarized in the thread.

## Installed Automation

Codex app automations:

- `TextQuest issue executor`
  - root: the TextQuest repository checkout on the always-on automation host
  - execution mode: worktree
  - purpose: claim one eligible issue, implement it, verify it, and open a PR
  - current cadence: hourly
- `TextQuest PR manager`
  - root: the TextQuest repository checkout on the always-on automation host
  - execution mode: worktree
  - purpose: open missing PRs, address straightforward review feedback, and merge eligible PRs
  - cadence: hourly

GitHub workflow:

- [`.github/workflows/claude-agent.yml`](../.github/workflows/claude-agent.yml)
  - responds only when `worker:claude` is present on an issue or a comment explicitly mentions `@claude`
  - uses [`docs/claude-issue-worker.md`](claude-issue-worker.md) as the repo-tracked worker brief
  - never merges

## External Setup And Constraints

- The Codex app must stay running on the always-on machine that has a TextQuest repository checkout available on disk.
- GitHub CLI auth on that machine needs `project` and `read:project` scopes in order to inspect and mutate the `TextQuest Roadmap` project state.
- The autonomous queue uses the custom `Agent Status` field so the project can keep the built-in `Status` field for broader roadmap progress.
- The optional Claude workflow requires repository secret `ANTHROPIC_API_KEY`.
- `master` stays protected, with the Windows `PR gate (fmt + clippy + test + python)` check as the merge blocker.
- Current Codex app automations only support hourly cadences. The original target cadence for issue pickup was every 15 minutes, but the installed automation currently runs hourly until minute-level scheduling becomes available or the executor is moved to an external scheduler.

## Manual Recovery

- If an issue gets stuck with `agent:working`, either let the current worker finish or move it to `Blocked` before re-queueing it.
- If you want to pause unattended merging for a PR that is already in flight, remove `merge:auto` or add `human:required`.
- If a research issue should stay in docs only, keep `mode:research` and do not add `merge:auto`.
