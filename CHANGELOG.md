# Changelog

All notable changes to TextQuest will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking Changes

### Features

- **AutoShip helper scripts** — `scripts/autoship-helpers/` adds `batch-dispatch-codex.sh`, `batch-dispatch-claude.sh`, `verify-and-pr.sh`, `bulk-rebase.sh` for high-throughput PR generation and conflict cascade recovery (#2855)

### Bug Fixes

- **Master compile** — Restored after 70-PR rebase wave: missing module decls (`textquest-learn`), duplicate `pub mod self_improvement`, missing `arc-swap` workspace dep, dual `[features]` block, unclosed `Command::SendMailToMule` enum variant, duplicate `data_dir`, missing `improve::router()` (#2853, #2867)
- **Linkdb match** — `Command::QueryLinkdb` and `Command::RecordLinkdbItem` stubbed via `send_unsupported_command("Linkdb")` until DLL implementation lands (#2867)
- **Mail match** — `Command::OpenMailWindow`, `QueryMailWindowState`, `SendMailToMule` stubbed similarly (#2867)

### Performance

### Security

- **Python deps** — Bumped torch 2.0→2.8, onnx 1.14→1.21, scikit-learn 1.3→1.5, d3rlpy 1.1.1→2.7.0 in `textquest-learn/python/requirements.txt`. Resolves 16 Dependabot alerts (1 critical torch RCE, 5 high onnx path-traversal, etc.) (#2842)

### Documentation

- **Session 2026-04-25 learnings** — CLAUDE.md gotchas updated with `git rebase -X ours` caveats, AutoShip PR# vs issue# distinction, codex model fallback chain, sub-issue file overlap pattern

## [0.6.0] - 2026-04-14

### Features

- M7 feature parity milestone: soul engine, party-level AI, async orchestration loop
- Combat rotation system with ability priorities and cooldown tracking (#1234)
- DLL injection framework for Windows client control
- Named pipe and shared memory IPC for process-to-process communication
- Multi-client state synchronization and polling orchestrator
- TUI dashboard with spawn list, player/target panels, navigation, and group views
- Web dashboard (React + Axum) for browser-based monitoring
- Demo mode support for TUI development on macOS (no live EQ client required)
- Camp configuration system with zone, pull point, mana thresholds
- Class ability configurations with cooldowns and priorities (16 classes)
- High-value target (HVT) watchlist and Discord alerts
- Account management system with per-character class and group assignments
- Integration with MacroQuest offset research for EQ memory layout
- Release management infrastructure with versioning and tagging

### Bug Fixes

- Fixed spawn linked list traversal with safety limits (#1223)
- Corrected field-by-field spawn data population for offset alignment (#1230)
- IPC dual-channel naming for multi-session support (#1245)
- Offset rebasing for preferred-base to runtime address conversion (#1250)
- Platform stubs for macOS/Linux builds (Windows APIs gated behind cfg)

### Performance

- Optimized EQ memory reads with batch processing
- Async orchestration loop for non-blocking client updates
- Efficient spawn list walking with max-count safety limits

### Security

- DLL injection validates EQ process memory layout before hook installation
- Named pipe access control via IPC channel isolation per session_id
- Shared memory segmentation to prevent cross-client data leakage
- All Windows APIs behind cfg(windows) to prevent cross-platform leaks

### Documentation

- Comprehensive README.md with usage and quick-start guide
- Developer reference in docs/wiki/ (on-going)
- Implementation roadmap in docs/implementation-roadmap.md
- Code comments for EQ offset discovery and memory layout assumptions
- Configuration file examples for camps, classes, and accounts

### Known Issues

- EQ patch detection requires offset revalidation post-patch (pattern scanner provided)
- Nightly release has 3am CT time gate on Windows runner
- Self-hosted runner workspaces persist files between runs (use skipTest guards)

## [0.5.0] - 2026-03-15

### Features

- Initial M6 milestone: web dashboard and group builder
- Axum web framework foundation
- React SPA for dashboard UI

### Bug Fixes

- Fixed zone transition state tracking

### Performance

- Baseline performance metrics established

## [0.4.0] - 2026-02-15

### Features

- M5 milestone: anti-cheat evasion and voice integration
- Initial TUI framework
- Player/target data capture

## [0.3.0] - 2026-01-15

### Features

- M4 milestone: core EQ internals
- Memory reading infrastructure
- Offset discovery and validation

## [0.2.0] - 2025-12-15

### Features

- M3 milestone: DLL injection foundation
- Windows process injection
- Shared memory IPC

## [0.1.0] - 2025-11-15

### Features

- M1-M2 foundation: project setup, CI/CD, cross-platform architecture
- Workspace organization (textquest, textquest-common, textquest-dll, textquest-web)
- GitHub Actions CI pipeline with self-hosted runners
- Rust edition 2024 project structure
- Cross-platform stubs for macOS/Linux support

---

## How to Contribute

When preparing a release, update this file following the template above:

1. Add new entries under `[Unreleased]` during development
2. Before release, move entries to a versioned section with release date
3. Link to issues/PRs where applicable (e.g., `#1234`)
4. Use categories: Breaking Changes, Features, Bug Fixes, Performance, Security, Documentation, Known Issues
5. Run `scripts/prepare-release.sh` to validate and tag the release

See [Release Process Documentation](docs/dev/release-process.md) for full details.

## Release Schedule

- **Regular releases**: Monthly (1st week of each month, pending milestones)
- **Nightly releases**: Automated at 3am CT on Windows runners
- **Hotfixes**: Ad-hoc for critical security/stability issues

For version numbering and semantic versioning rules, see [Semantic Versioning](https://semver.org/).
