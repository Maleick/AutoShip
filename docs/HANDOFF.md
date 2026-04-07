# Session Handoff — 2026-04-06 (late)

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context.

MacroQuest reference code lives in local git submodules at `third_party/eqlib`
and `third_party/macroquest`. After checkout, run
`git submodule update --init --recursive` before doing offset or struct work.

## Repository Stats

- **Branch:** `master`
- **Total commits:** ~1015
- **Tests:** ~2,574 platform-independent across 4 crates
- **Rust edition:** 2024

## Workspace Crates

```
textquest        (v0.5.0) — orchestrator, TUI, launcher, combat, nav
textquest-dll    (v0.5.0) — injected DLL, hooks, in-client automation
textquest-common (v0.5.0) — shared types, offsets, IPC protocol
textquest-web    (v0.1.0) — axum web dashboard (early)
```

## Current State

### Login chain

- **3 fixes on master** hardened eqmain login phases (direct vtable clicks instead of queued)
- `click_button_for_phase()` helper prevents queue-during-eqmain class of bugs
- **Kara rewrote login on Frostreaver** to use HWBP hook on `LoginController::GiveTime()` (main thread, MQ2 approach)
  - 5 commits NOT yet pushed to origin
  - Needs cross-thread HWBP fix (SetThreadContext) to fire on correct thread
  - Push Kara's commits first, then rebase PRs on top

### Milestones

- M1-M4: COMPLETE
- M5 Anti-Cheat: ~97% — #355 closed, only #487 (sleep obfuscation) + #491 (reflective loader) remain
- M6 Web Dashboard + TUI: ~70% — 9 PRs open covering most remaining features

### Open PRs (9, all from this session)

| PR   | Issue | What                                                            |
| ---- | ----- | --------------------------------------------------------------- |
| #499 | #490  | /cleartarget + /target fix                                      |
| #500 | #493  | Chat message hook (dsp_chat HWBP)                               |
| #501 | #496  | Session lifecycle enum (CampingOut/Exited/Relaunching)          |
| #502 | #495  | Graceful camp-out before restart                                |
| #503 | #492  | Spawn search + sort + nav-to                                    |
| #504 | #480  | Screenshot IPC (one-frame render capture)                       |
| #505 | #481  | Deep null renderer (DX9 DrawPrimitive hooks)                    |
| #506 | #497  | DX11 API update (windows crate v0.54)                           |
| #507 | #54   | Priority TUI panel (**BLOCKED: missing dashboard.rs renderer**) |

PRs #499-#506 have Gemini auto-reviews. PR #507 needs dashboard.rs fix before merge.

### Open Issues (87 total)

- **18 active** (M5/M6/M7 scope)
- **69 deferred** (post-M7, mostly MQ2 parity: map, nav, moveutils features)
- **3 bugs**: #487 sleep obfuscation, #490 (PR open), #497 (PR open)

### CI

- Main `ci.yml` pipeline: GREEN
- Wiki nightly sync: WORKING
- Nightly release: WORKING
- 5 auxiliary workflows fixed this session (ubuntu-latest -> self-hosted)
- `readme-metrics.yml` is manual-only (no cron schedule)

## Next Session Plan

1. **Merge 9 PRs** (fix #507 dashboard.rs first, then #499-#506)
2. **Run simplify + codex** on merged code
3. **Burn down issues with 6-agent teams**, priority order:

**Wave 1 — Close out M5 + active M6 (6 agents):**
| Issue | What | Why first |
|-------|------|-----------|
| #487 | Sleep obfuscation crash fix | M5 bug, blocks milestone close |
| #491 | Reflective loader import resolution | M5 feature, blocks milestone close |
| #494 | Orchestrator main loop | M6 core, wires everything together |
| #55 | Per-toon/group config panels | M6 TUI feature |
| #46 | Rolling nightly prerelease CI | Infrastructure, oldest open |
| #50 | Zone transition state mapping | M7 research, oldest enhancement |

**Wave 2 — Oldest post-M7 issues (6 agents):**
#97, #99, #100, #101, #102, #103

**Wave 3+ — Continue post-M7 backlog oldest-first:**
#105, #106, #107, #116, #117, #120, then remaining 57 issues

**After each wave:** Run simplify + code-review + codex

4. **Coordinate with Kara** — her GiveTime rewrite is on master (pushed), needs HWBP cross-thread fix
5. **Live login test** on Frostreaver

## Key References

- Implementation roadmap: `docs/implementation-roadmap.md`
- Login hardening plan: `docs/superpowers/plans/2026-04-06-login-chain-hardening.md`
- Login automation wiki: `docs/wiki/Login-Automation.md`
- Local eqlib reference: `third_party/eqlib`
- Local MacroQuest reference: `third_party/macroquest`

## Build

```bash
cargo build              # Debug (macOS demo mode)
cargo build --release    # Release (Windows production)
cargo run                # TUI with demo data
cargo test               # ~2,574 tests
cargo clippy             # Lint
```

CMAKE_POLICY_VERSION_MINIMUM is set via `.cargo/config.toml`.
