# Session Handoff — Issue Burndown (2026-04-03 Late Evening)

## Active Work (DO NOT TOUCH — other session is running)

Another Claude Code session is actively running with agents on branch `claude/ghidra-offsets-and-docs` (PR #366):

- **harvester**: Bulk exporting ~20K functions from GhidraMCP (192.168.1.207:8089) to `data/ghidra-export/`
- **schema-builder**: DONE — created `dmft-common/src/ghidra_db.rs` (SQLite module for Ghidra data)
- **tui-researcher**: DONE — researched map rendering, recommends cached grid approach

**Do NOT modify**: `dmft-common/src/offset_db.rs`, `dmft-common/src/offsets.rs`, `dmft-common/src/ghidra_db.rs`, `dmft-common/src/lib.rs`, `dmft-common/Cargo.toml`

## Your Mission: Issue Burndown (98 open issues)

There are 98 open GitHub issues. 86 are tagged `agent:ready`, 75 tagged `post-m7`. Your job is to triage them and solve as many as possible using TeamCreate with parallel agents.

### Phase 1: Triage (~10 min)

```bash
gh issue list --state open --limit 200 --json number,title,labels,body
```

Categorize each issue:

- **Auto-solvable**: Clear implementation tasks an agent can complete independently
- **Stale/done**: Already resolved by merged PRs or overtaken by events — close these
- **Needs human**: Requires design decisions, live testing, or user input — skip for now
- **Blocked**: Depends on unfinished work (e.g., Ghidra harvest) — skip for now

### Phase 2: Parallel Execution

Use **TeamCreate** to spin up agent teams (up to 6 tmux splits). Each agent:

1. Claims an issue: move Agent Status -> Agent Working, replace `agent:ready` with `agent:working`, post a claim comment
2. Creates branch: `claude/issue-<number>-<slug>` from `master`
3. Implements the fix/feature
4. Runs `cargo build && cargo test` (must pass before PR)
5. Opens a non-draft PR into `master`, links the issue
6. Shuts down, reports completion

When an agent finishes, assign the next issue from the queue.

### Agent Type Strategy

Split work between Claude and Codex to stretch both quotas:

- **Claude agents** (TeamCreate, general-purpose): Complex multi-file implementation, architecture decisions, M5 anti-cheat work, DLL code
- **Codex Spark** (codex:rescue skill): Issue triage, research tasks, simple single-file fixes, docs updates, boilerplate generation

The Codex plugin is installed and authenticated (v0.118.0). Use `/codex:rescue` to delegate simpler tasks.

### Key Rules

- **Always use TeamCreate** for parallel work — visible in tmux splits
- Run `cargo test` before opening any PR — never skip this
- **Never merge PRs** — just create them, the PR manager handles merges
- Check `CLAUDE.md` and `AGENTS.md` for full conventions
- The project is Rust (edition 2024), ~78K lines, 3 crates: dmft, dmft-dll, dmft-common
- macOS builds with stubs — `cargo build` and most tests work without Windows
- If blocked, move Agent Status to Blocked, add `agent:blocked` label, leave an unblock comment

### Environment

- Working directory: `/Users/maleick/Projects/DMFT`
- **Switch to master first**: `git checkout master`
- CI: 4 self-hosted runners with `dmft` label
- Frostreaver (Windows box): 192.168.1.207, SSH via `~/.ssh/frostreaver`
- 20/20X max plan — plenty of usage, go fast

### Open PRs (don't duplicate work)

- #366 Ghidra offsets + OffsetDatabase functions HashMap (other session, in progress)
- #364 CI cmake trust (Codex)
- #363 Claude-agent checkout hardening (Codex)
- #362 Cross-machine parity

### Issues with Special Handling

- #365 [M5] Per-client fingerprint spoofing — HIGH priority, complex, needs Claude agent
- #367 [TUI] Offset explorer in debug tab — depends on Ghidra harvest, skip for now
- #368 TUI polish milestone — tracking issue, not implementable, skip
- #355, #354, #353, #352, #351 — marked `agent:working`, check if stale

### Label Reference

| Label           | Meaning                                                    |
| --------------- | ---------------------------------------------------------- |
| `agent:ready`   | Ready for an agent to pick up                              |
| `agent:working` | Currently being worked on                                  |
| `agent:blocked` | Blocked, needs unblock                                     |
| `worker:claude` | Route to Claude (not Codex)                                |
| `post-m7`       | Deferred past M7 (now M5 is active, some may be unblocked) |

### Milestones (current order)

- **M5** Anti-Cheat (ACTIVE) — 6 PRs merged, fingerprint spoofing next
- **M6** Web Dashboard — not started
- **M7** Zoning/Movement
- **M8** Orchestrator
- **M9** Learning/RL
- **M10** Economy
- **M11** Soul Engine + LLM (local only)
