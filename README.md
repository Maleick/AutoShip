# TextQuest

<p align="center">
  <img src="art/TextQuest.jpeg" alt="TextQuest Logo" width="600">
</p>

<p align="center">
  <i>"What happens in Neriak, stays in Neriak."</i>
</p>

[![CI](https://github.com/Maleick/TextQuest/actions/workflows/ci.yml/badge.svg)](https://github.com/Maleick/TextQuest/actions/workflows/ci.yml)
[![Release](https://github.com/Maleick/TextQuest/actions/workflows/release.yml/badge.svg)](https://github.com/Maleick/TextQuest/actions/workflows/release.yml)
[![Rust](https://img.shields.io/badge/rust-edition%202024-orange?style=flat-square)](https://www.rust-lang.org/)
[![Rust LOC](https://img.shields.io/badge/Rust%20LOC-136%2C836-blue?style=flat-square)](#testing)
[![Tests](https://img.shields.io/badge/Tests-3%2C017%20exact-brightgreen?style=flat-square)](#testing)
[![Status](https://img.shields.io/badge/status-Active-green?style=flat-square)](#roadmap)
[![License](https://img.shields.io/badge/license-Private-red?style=flat-square)](#license)

External process memory reader, DLL injector, and multibox controller for EverQuest, built in Rust.

TextQuest reads live game state from EQ clients, injects a DLL for direct control via in-process EQ calls, and coordinates multi-client sessions from a TUI-first operator workflow.

## What TextQuest Does Today

- Runs in demo mode on macOS, Linux, or Windows without attached EQ clients so the TUI and operator flow remain usable during normal development
- Runs live on Windows for injection, IPC, login, navigation, combat, camp-loop, and packet-monitoring work
- Exposes five main TUI screens: Characters, Map, Navigation, Debug, and Packets
- Supports command-bar control, camp automation, navmesh tooling, login orchestration, named tracking, and shared-memory session inspection
- Keeps Soul Engine and provider-backed chat behavior separate from the current operator/runtime surface and tracked under `M11` in the canonical roadmap

## Features

- **DLL Injection** — Rust `cdylib` injected into running EQ clients
- **InterpretCmd Control** — Calls EQ's internal slash-command path for direct command execution
- **IPC Pipeline** — Named pipes for commands and shared memory for live game state
- **TUI Operator Surface** — Five-screen dashboard with command mode, help overlay, privacy mode, themes, filters, and client focus controls
- **Camp Automation** — Six-phase camp loop, class-driven combat logic, CH chain support, CC handling, buff maintenance, and recovery flows
- **Login Automation** — Credential store, launch coordination, login FSM, and post-login sequencing
- **Navigation** — Navmesh pathfinding, route diagnostics, waypoint tooling, stuck detection, and recovery support
- **Packet Monitoring** — Live send/receive capture with filtering and opcode decode
- **Web Surface** — Embedded web/dashboard crate for configuration and monitoring work already present in the repo

## Quick Start

### Preflight

```bash
python3 scripts/dev-preflight.py
```

Routine `cargo build` / `cargo test` work does not require external eqlib or MacroQuest reference material.

### Demo mode

Use this on macOS, Linux, or Windows when you do not have a live EQ client attached.

```bash
cargo build
cargo run
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

### Live Windows mode

Use this for real EQ interaction, injection, login, navigation, combat, and packet work.

```powershell
cargo build --release
target\release\textquest.exe inject
target\release\textquest.exe tui
```

Useful direct CLI commands:

```powershell
target\release\textquest.exe dump
target\release\textquest.exe client-status 12345
target\release\textquest.exe client-status-all
target\release\textquest.exe cmd 12345 "/sit"
target\release\textquest.exe navmesh reload gfaydark
target\release\textquest.exe navmesh diagnostics --pid 12345
```

## TUI Basics

Current workspace totals: 136,836 Rust lines and 3,017 exact tests. This line and the badges above are auto-refreshed by `scripts/update_readme_metrics.py`.

| Trigger                | Jobs                                                                   |
| ---------------------- | ---------------------------------------------------------------------- |
| Same-repo pull request | self-hosted `PR gate (fmt + clippy + test + python)`                   |
| Fork pull request      | GitHub-hosted `PR gate (fmt + clippy + test + python)` on Windows      |
| Push to master         | self-hosted `PR gate (fmt + clippy + test + python)`                   |
| Manual `CI` dispatch   | required PR gate, with optional `Windows release build (manual)` input |

Tag-triggered releases (`v*`) build Windows binaries and create GitHub Releases automatically.

Release and wiki automation now run separately on the self-hosted Windows runner:

- wiki auto-publish via `scripts/sync_wiki.py --check`
- wiki auto-publish via `scripts/sync_wiki.py --push`
- rolling nightly prerelease build and artifact upload

## Configuration

Useful starting commands:

```text
:status overview
:camp list
:nav <camp|x y z|zone>
:login all
:ch status
:help camp
```

The full operator guide lives in [`docs/wiki/Operating-the-TUI.md`](docs/wiki/Operating-the-TUI.md) and [`docs/wiki/Command-Reference.md`](docs/wiki/Command-Reference.md).

The TUI `:inject` command is still a placeholder; use the CLI `inject` command for live injection work.

## Configuration

- Main app config: `config/frostreaver.toml`
- Accounts and group-launch metadata: `config/accounts.toml`
- Camps: `config/camps/*.toml`
- Class configs: `config/classes/*.toml`
- Per-toon overrides: `config/toons/*.toml`
- HVT watchlist: `config/hvt_watchlist.toml`
- Zone maps: `config/maps/*.txt`

The detailed configuration guide lives in [`docs/wiki/Configuration.md`](docs/wiki/Configuration.md).

## Documentation

- Start here: [`docs/wiki/Quick-Start.md`](docs/wiki/Quick-Start.md)
- Build and platform setup: [`docs/wiki/Installation-and-Build.md`](docs/wiki/Installation-and-Build.md)
- TUI operator guide: [`docs/wiki/Operating-the-TUI.md`](docs/wiki/Operating-the-TUI.md)
- Command reference: [`docs/wiki/Command-Reference.md`](docs/wiki/Command-Reference.md)
- Troubleshooting: [`docs/wiki/Troubleshooting.md`](docs/wiki/Troubleshooting.md)

## Roadmap

README stays focused on building, running, and operating TextQuest. Milestone order, evidence rules, validation gaps, and project-mirroring rules live in [`docs/implementation-roadmap.md`](docs/implementation-roadmap.md) and the summary page [`docs/wiki/Roadmap-and-Known-Gaps.md`](docs/wiki/Roadmap-and-Known-Gaps.md).

The current roadmap keeps economy work at `M10` and Soul Engine + LLM work at `M11`.

## Requirements

- **Rust** edition 2024
- **Windows** for live EQ interaction
- **Nightly MSVC toolchain on Windows** because `retour` still depends on unstable features
- **EverQuest client** for real injection, login, navigation, combat, and packet validation

## License

Private project.
