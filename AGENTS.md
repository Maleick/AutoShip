# AGENTS.md

Shared operating contract for autonomous coding agents in this repository.

## Source Of Truth

- The repository contents and docs are authoritative. GitHub Projects mirror repo truth; they do not replace it.
- `docs/implementation-roadmap.md` remains the source of truth for milestone order, checkpoint batches, and evidence-state rules.
- `feature-list.json` remains the source of truth for multi-session feature tracking.

## Queue Entry

- Codex is the default issue worker.
- Claude is an optional worker only when an issue is labeled `worker:claude` or a comment explicitly mentions `@claude`.
- An issue is eligible for autonomous execution only when both are true:
  - the `TextQuest Roadmap` GitHub Project `Agent Status` field is `Ready for Agent`
  - the issue has label `agent:ready`
- Skip issues that already have `agent:working`, already have an open pull request, or are already blocked with `agent:blocked`.
- GitHub label meanings:
  - `agent:ready` — eligible for pickup
  - `agent:working` — currently claimed by one worker
  - `agent:blocked` — concrete blocker surfaced back to the operator
  - `mode:research` — use the research-first workflow
  - `worker:claude` — route implementation to Claude instead of Codex
  - `merge:auto` — trusted automation may merge unattended once all gates are clear
  - `human:required` or `risk:high` — never auto-merge or auto-close

## Claiming And Execution

- Claim at most one issue per run.
- Claim flow:
  - move the GitHub Project `Agent Status` field to `Agent Working`
  - replace `agent:ready` with `agent:working`
  - post a short claim comment that names the branch being created
- Branch from `master` using `codex/issue-<number>-<slug>` or `claude/issue-<number>-<slug>`.
- Default to implementation mode for concrete issues. Do not wait for a second prompt that says to code.
- If an issue has `mode:research`, use the `codex-autoresearch` skill or the same structured docs-first loop:
  - update roadmap or research docs first
  - run the relevant verifier and guard commands
  - only then mirror mature work back into GitHub issues or project items
- If the issue spans multiple independent surfaces, use subagents or parallel workers automatically and integrate the results before opening the PR.
- Never commit transient automation state or autoresearch runtime artifacts.

## Pull Requests

- Open a non-draft pull request into `master`, link the issue, and summarize scope plus validation.
- Agent-authored PRs are trusted by default:
  - add `codex` for Codex-authored PRs or keep the equivalent worker label already in use
  - add `codex-automation` for automation-opened PRs
  - add `merge:auto` when neither the PR nor the linked issue has `human:required`, `risk:high`, or `agent:blocked`
- Move the GitHub Project `Agent Status` field to `PR Open` after the PR exists.
- When behavior or operator guidance changes, update README and the matching `docs/wiki/` page in the same PR.
## Verification

- Always run every command listed in the issue template's `Verify` section.
- If the diff touches Rust, config, or scripts, also run repo gate commands when feasible:
  - `cargo fmt --all --check`
  - `cargo test`
  - any directly relevant Python or wiki validation commands
- If the diff is docs, workflow, or prompt only, run lightweight syntax or parse checks plus `python3 scripts/sync_wiki.py --check`.

## PR Manager

- The shared Codex PR manager owns unattended merge and cleanup for agent-authored PRs targeting `master`.
- Treat top-level bot overview comments as non-blocking.
- Read inline review state through GraphQL `reviewThreads`; unresolved, non-outdated threads remain blocking until they are actually addressed.
- The PR manager may resolve bot-authored review threads only when the thread is clearly addressed in-branch by a pushed fix or the requested change was already present on the branch.
- Merge or enable auto-merge only when all are true:
  - base branch is `master`
  - PR is non-draft
  - required Windows `PR gate (fmt + clippy + test + python)` is green
  - no unresolved review threads remain
  - the PR or linked issue has `merge:auto`
  - neither the PR nor linked issue has `human:required`, `risk:high`, or `agent:blocked`
- Close stale or superseded agent-authored PRs automatically when both are true:
  - neither the PR nor linked issue has `human:required`, `risk:high`, or `agent:blocked`
  - one of these disposal conditions holds:
    - a newer agent PR for the same linked issue or branch surface is already open or merged
    - the PR has been idle for at least 72 hours and is still conflict-dirty or failing required checks with no check run in progress
- When auto-closing a PR, leave a short comment stating whether it was superseded or stale and what branch or issue should be used instead.

## Blockers

- If blocked:
  - move the GitHub Project `Agent Status` field to `Blocked`
  - remove `agent:working`
  - add `agent:blocked`
  - leave a concrete unblock comment with the exact next human decision or missing prerequisite
- Never silently abandon a claimed issue.
