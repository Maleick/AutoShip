# TextQuest Agent Workflow

## Repository Ownership

- Live is the primary product target going forward.
- Test is historical and reference-only.
- **Machine-test epic** — GitHub issue #3596 tracks ~176 deferred issues that require live Windows/EQ validation before they can be closed. These were deferred during the 2026-04-26 burndown wave and are not `agent:ready` until live-validation evidence is provided.
- `TextQuest-Ghidra` is canonical for immutable snapshots, manifests, baseline selection, curated Ghidra evidence, and Ghidra intake and analysis flow.
- `TextQuest` is canonical for code, `docs/wiki/`, runbooks, automation, and lightweight references that point at canonical evidence.
- `docs/wiki/` is the canonical documentation surface. The GitHub wiki is a lightweight landing page only.

## Master-Safe Integration

- Base every branch on fresh `origin/master`.
- Keep PRs focused and honest. Prefer multiple small PRs over one large migration branch.
- Never merge active Test defaults, Test offsets, or Test-only workflow guidance into `master`.
- Keep Test references historical-only when they still help explain current behavior or evidence lineage.
- If work is not solved in the current branch, open or update a strict GitHub issue instead of burying the gap in docs, memory, or thread context.

## Standards and Quality

All work performed by agents or human developers must adhere to the [Documentation & Polish Standards](docs/dev/polish-standards.md). This includes specific requirements for issue structure, PR content, code documentation, and testing.

## GitHub Tracking Policy

- Issues are the default unit of work.
- Milestones are release and initiative grouping buckets.
- PRs are the implementation and review unit.
- GitHub Projects are retired and historical-only.
- Parent epic issues stay open as coordination shells until all child issues are complete.
- Mark an issue `agent:ready` only when the body follows the [Documentation & Polish Standards](docs/dev/polish-standards.md), specifically including the Task Description, Scope, Design (if needed), Implementation Notes, Testing, and Acceptance Criteria sections.
- Use `agent:blocked` or `human:required` when required evidence, access, or policy decisions are missing.

## Issue Decomposition Standard

Every identified gap, bug, or feature must be filed as a parent GitHub issue and decomposed into sub-issues. Sub-issues must be sized for a single agent pass — one file, one function, one test, or one config change per sub-issue.

**Agent tier assignment:**

| Label                | Agent         | Use for                                                                      |
| -------------------- | ------------- | ---------------------------------------------------------------------------- |
| `agent:haiku`        | Claude Haiku  | Single-file edits, config changes, boilerplate, simple test additions        |
| `agent:gemini-flash` | Gemini Flash  | Multi-file changes within a single module, straightforward feature additions |
| `agent:gpt-mini`     | GPT-4o Mini   | Documentation, README updates, minor refactors                               |
| `agent:ready`        | Any available | No tier preference — first available agent picks it up                       |
| `worker:claude`      | Claude Sonnet | Architecture decisions, cross-cutting changes, security-sensitive work       |

**Rules:**

- Never assign a sub-issue that spans more than one logical concern.
- If a sub-issue requires reading more than 3 files to understand, split it further.
- Expensive agents (Sonnet/Opus) only for architecture, security, or cross-cutting changes.
- Parent issues must list all sub-issues as a checklist in the body before being marked `agent:ready`.

## Evidence And Docs

- Link to canonical `TextQuest-Ghidra` snapshot and manifest paths instead of copying immutable evidence into this repo.
- Treat repo-local caches such as `data/ghidra.db` or local export folders as mutable runtime and debug state, not canonical evidence.
- Update the matching `docs/wiki/` page in the same PR whenever behavior or operator workflow changes.

## Quality Commands

Run these before every push and PR:

```bash
# Fast feedback loop (macOS/Linux)
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
python3 scripts/dev-preflight.py   # bundles wiki/lint/test/python — mirrors CI gate
```

- **`dev-preflight.py`** is the canonical local pre-push check. It runs `cargo fmt`, wiki sync validation, offset sync validation, Rust tests, and Python tests.
- Python tests are **advisory** (they do not block merge) — CI runs them with `continue-on-error: true`. If they fail, note it in the PR body.
- Do not `#[allow(...)]` clippy lints without an inline comment explaining why.
- `cargo test` on macOS runs all workspace crates. Windows-only tests are gated `#[cfg(windows)]`.

## Agent-Facing Environment Requirements

- `TEXTQUEST_DATA_DIR` is mandatory for Frostreaver-style deployments and should be set to the absolute deployment root containing `config/` and `data/`.
- `TEXTQUEST_ALERT_DB_PATH` is optional and can be used to relocate the shared alert SQLite database independently of `TEXTQUEST_DATA_DIR`.
- If `TEXTQUEST_DATA_DIR` is not set, runtime fallback is executable parent directory, then `.` (current working directory), which is still valid only for local launch and not recommended for production hosts.

## Requesting Code Review

**Use the `requesting-code-review` skill before opening a PR or marking implementation complete.** It guides verification of scope, coverage, and code quality before requesting human review.

When the PR is green and all conversations are resolved:
- Enable auto-merge with `gh pr merge --auto --squash --delete-branch`.
- Move the GitHub issue to `Merging` once auto-merge is active.

## Branch Naming

| Prefix          | Owner    | Use for                                                      |
| --------------- | -------- | ------------------------------------------------------------ |
| `feature/*`     | Humans   | New features, user-initiated                                 |
| `fix/*`         | Humans   | Bug fixes, user-initiated                                    |
| `claude/*`      | Claude   | Human-initiated agent tasks                                  |
| `codex/*`       | Codex    | Agent-initiated work (orchestrator, AI-driven)              |
| `autoship/*`    | AutoShip | Automated issue work                                         |

Branch off `origin/master` for every piece of work. Keep PRs small and honest.

## Definition of Done

A leaf issue is done when:
1. All acceptance criteria in the issue body are met and verified.
2. `cargo fmt --check` and `cargo clippy --all-targets --all-features -- -D warnings` pass.
3. `cargo test` passes.
4. `python3 scripts/dev-preflight.py` passes.
5. New logic has unit tests (coverage target: >80% on new code, >70% on modified).
6. `docs/wiki/` is updated if behavior or operator workflow changed.
7. PR is open with a closing keyword for the issue.
