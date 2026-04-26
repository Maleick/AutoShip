# TextQuest v0.8.0 — Workspace Cleanup & Release-Readiness

**Release date**: 2026-04-26
**Status**: Pre-T.O.P.-launch hardening release

## Overview

v0.8.0 is a release-readiness pass: the master branch had accumulated significant compile drift across multiple crates and CI was silently broken. This release fixes all of it and aggressively cleans up the surface that built up between v0.7.0 and now (1,000+ commits, ~2 weeks of AutoShip activity).

## Highlights

### ✅ Master compile drift resolved

`cargo check --workspace --all-targets --all-features` went from **~270 errors across 5 crates** → **0**.

- **textquest-dll**: 100+ `CombatContext` test fixture sites missing `burn_state`, `burnnow_triggered`, `burn_cooldown_ticks`, `positional`; 10+ `RotationGroup` sites missing `burn_duration_ticks`, `burn_cooldown_duration_ticks`; missing `bandolier` module declaration; `Command::PollChecksumAlerts` not in match arm
- **textquest-soul**: lancedb API drift (`only_if` moved to `QueryBase` trait, `Float32Type` import path, Scannable trait bound), Eq derive over `f64`, missing arg in `respond_to_player`
- **textquest-learn**: broken imports of removed types; missing deps (`clap`, `tokio`, `tracing`, `tracing-subscriber`); dead `full`-feature modules with unresolved `rusqlite` and `ndarray`
- **textquest-web**: `AppState` test fixtures missing 6-10 fields each across 5 sites
- **textquest**: `ClassConfig` test fixtures missing 3 fields; replay borrow-checker error
- **llm-client**: dead integration test referencing removed `OllamaClient`

### ✅ CI gates green

`cargo clippy --workspace --all-targets --all-features -- -D warnings` passes. fmt clean. tests compile clean.

Root causes of latent CI failure on every PR:

1. **Duplicate `.rustfmt.toml`** with duplicate `comment_width = 80` key + wrong `edition = "2021"` (project is on 2024) — broke fmt-check
2. **Dead `pub mod` declarations** referencing non-existent files (`anomaly`, `audio_dispatcher`, `audio_integration_example`, `emacs`, `conflict_dialog`) — only surfaced under cargo fmt's no-cfg-respect
3. **textquest-learn `full` feature** declared in lib.rs `#[cfg]` gates but undeclared in Cargo.toml; gated 6 dead modules with broken imports

### 🧹 Workspace structure cleanup

- Removed orphan crates: `textquest-net` (EQBC/DanNet/NetBots transports, never wired) and `textquest-voice` (TTS stubs, never wired). Both had `Cargo.toml` but were not in `[workspace] members`.
- Removed legacy frontend: `web/` (227MB, demoted 2026-04-24). The active TS frontend lives at `textquest-web/frontend/`.
- Removed 6 dead feature-gated modules: `bookmarks.rs`, `dataset.rs`, `ledger.rs`, `onnx_export.rs`, `quality_gates.rs`, `training.rs`.
- Removed dead admin binary `textquest-learn/src/bin/main.rs` (referenced removed types).

### ⚙️ CI workflow consolidation (23 → 17 files)

Consolidated `.github/workflows/`:

- Deleted 7 workflows: `03-format.yml`, `04-clippy.yml`, `05-test.yml` (duplicated `ci.yml`'s `pr_gate` job — double-execution on every PR), `06-cleanup-stale.yml` (opened a new GitHub issue every weekly run), and 3 validation workflows merged into one `validate.yml`.
- Branch-name regex updated to accept all current branch prefixes (`autoship/`, `fix/`, `refactor/`, `ci/`, `test/`).

### 📚 Docs purge (33 files removed)

- 8x `OVERNIGHT-*.md` sprint artifacts
- M7/M8 agent bootstrap (milestone passed)
- Date-tagged audits (canonical copies live in `docs/audits/`)
- Issue index snapshots (GitHub is source of truth)
- Old planning docs superseded by `docs/implementation-roadmap.md`
- `docs/superpowers/` (9 files; framework lives in `~/.claude/`, not the repo)

### 💾 Disk + repo footprint

- Local checkout: **65GB → 1.8GB after `cargo clean`** (target/ regenerates on demand)
- `.git`: **1.9GB → 381MB** after `git lfs prune` (1GB orphaned LFS, 0 currently tracked) + `git gc --aggressive`
- 24 `beacon-pre-simplify-*` snapshot tags removed
- Stale local + remote branches cleaned (32 deleted total)

## Feature work landed since v0.7.0

(Selected highlights from 1,000+ commits — see `git log v0.7.0..HEAD` for the full list.)

- **MezTracker** (PR #3564, #3535): immune-list with zone-change expiry, wired into ENC + BRD rotation modules.
- **TargetScanner**: XTarget aggro%, configurable priority ordering, safe-targeting predicate.
- **Multi-zone path planning**: A\* over the zone graph with camp config (`config/camps/howling_stones_entrance.toml` added in this release).
- **Self-improvement loop**: Bayesian posteriors driving suggestion engine.
- **Counter-hook layer**: defensive baseline for anti-detection research (ETW-TI coverage map in `docs/etw-ti-*.md`).
- **`PlayerIsStealthed` ConditionExpr** (PR #3610): unblocked the rotation-test surface that was importing the variant before it was defined.

## Known issues

- **Dependabot alerts** (tracking issue [#3611](https://github.com/Maleick/TextQuest/issues/3611)): `rustls-webpki` (HIGH) and `lru` (LOW). Both blocked by deep transitive chains:
  - `rustls-webpki 0.102.8` is pinned via `serenity 0.12 → tokio-tungstenite 0.21 → rustls 0.22`. No upstream fix in 0.102.x; only 0.103+. Practical exploitability LOW (Discord doesn't send CRLs).
  - `lru 0.12.5` is pinned via `tantivy 0.24 → lance 4.0 → lancedb 0.27`. Miri-only correctness lint, not a runtime exploit.
- Two macOS-only test files were gated `#![cfg(windows)]` (`metrics_integration`, `metrics_orchestrator_integration`, `test_stats_aggregation`) since the underlying `metrics` and `stats` modules are themselves `#[cfg(windows)]`. The dead `metrics_event_hooks.rs` empty stub was deleted.

## Upgrade guide

No breaking API changes. Just `cargo build` and you're current.

If you had local branches that referenced removed modules (`textquest-net`, `textquest-voice`, `textquest-learn::bookmarks/dataset/ledger/onnx_export/quality_gates/training`, or any of the dead `pub mod` declarations), rebase onto master and remove those references.

## Pull requests

| #     | Title                                                            |
| ----- | ---------------------------------------------------------------- |
| #3609 | feat(camps): add Howling Stones entrance camp config             |
| #3610 | fix(combat): add missing PlayerIsStealthed ConditionExpr variant |
| #3612 | chore(workspace): resolve compile drift across 5 crates          |
| #3615 | fix(ci): unbreak fmt/clippy/test                                 |
| #3616 | chore(ci): consolidate workflows 23 → 17                         |

Plus 1,000+ commits of AutoShip-driven feature work since v0.7.0.

---

**Compare**: [v0.7.0...v0.8.0](https://github.com/Maleick/TextQuest/compare/v0.7.0...v0.8.0)
