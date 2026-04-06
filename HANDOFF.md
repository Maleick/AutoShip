# Session Handoff — 2026-04-06

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context.

MacroQuest reference code lives in local git submodules at `third_party/eqlib`
and `third_party/macroquest`. After checkout, run
`git submodule update --init --recursive` before doing offset or struct work.

## Repository Stats

- **Branch:** `master`
- **Total commits:** ~1001
- **Tests:** ~1250 platform-independent tests across 3 crates (textquest + textquest-common + textquest-dll)
- **Rust edition:** 2024

## Workspace Crates

```
textquest        (v0.5.0) — orchestrator, TUI, launcher, combat, nav
textquest-dll    (v0.5.0) — injected DLL, hooks, in-client automation
textquest-common (v0.5.0) — shared types, offsets, IPC protocol
```

## Current State

### Login chain

- **3 fixes on master** hardened the eqmain login phases (direct vtable clicks instead of queued clicks)
- Phases 1-3 confirmed working end-to-end
- Hardening plan drafted at `docs/superpowers/plans/2026-04-06-login-chain-hardening.md`

### Milestones

- M1–M4: COMPLETE
- M5 Anti-Cheat: ~95% — only #355 (launchpad token RE) remains
- M6 Web Dashboard: ~55% — 11 closed, 9 open

### Open PRs

- #489, #486, #485 being triaged/closed during repo cleanup

### CI

- 3 auxiliary workflows (wiki-nightly, nightly-release, codex-related) have issues being addressed
- Core `cargo build` + `cargo test` CI passes clean

### Repo cleanup

- Full DMFT → TextQuest rename completed (crate names, CI, runners, docs)
- Issue burndown: 95 → 30 open issues
- Zero open PRs target in progress

## Key References

- Implementation roadmap: `docs/implementation-roadmap.md`
- Anti-detection research: `docs/anti-detection.md`
- Login automation wiki: `docs/wiki/Login-Automation.md`
- Local eqlib reference: `third_party/eqlib`
- Local MacroQuest reference: `third_party/macroquest`

## Build

```bash
cargo build              # Debug (macOS demo mode)
cargo build --release    # Release (Windows production)
cargo run                # TUI with demo data
cargo test               # ~1250 tests
cargo clippy             # Lint
```

CMAKE_POLICY_VERSION_MINIMUM is set via `.cargo/config.toml`.
