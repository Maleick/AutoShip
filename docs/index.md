---
layout: home
title: TextQuest
---

# TextQuest

**EverQuest multibox controller for TLP servers.**

TextQuest is a Rust-based orchestration tool for managing large box crews on EverQuest Time-Locked Progression servers. It combines a live terminal dashboard, automated camp management, and a group coordination engine into a single operator-facing control plane.

---

## Key Features

### Terminal Dashboard (TUI)

A full-featured `ratatui`-powered terminal UI with **5 screens** and **16 panels** across **4 visual themes** (Neriak Dark, Sunrise, Forest, Arctic).

| Screen | What it shows |
|--------|--------------|
| **Dashboard** | Character roster, HP/mana bars, group assignments, cast strips |
| **Spawns** | Live spawn list with filtering, search, and hex viewer |
| **Groups** | Per-group HP/mana grid for all box slots |
| **Map** | Zone map renderer with spawn positions and named mob tracker |
| **Navigation** | Per-character pathfinding status and waypoint controls |

### Camp Automation

- Pull, fight, loot, and med cycle with configurable thresholds
- Per-zone camp definitions with pull point, radius, and mana floors
- Auto-loot FSM, vendor sell cycle, and buff maintenance
- Hunt mode for roaming tank while group holds formation

### Group Management

- Dynamic group templates with role-based slot assignment
- Cross-group heal arbitration and Complete Heal chain coordination
- Crowd control assignment and mez queue
- Per-character personality profiles for natural-feeling timing variation

### Navigation

- Waypoint-based autonomous movement
- Humanized movement with per-character speed jitter and heading wobble
- Stuck detection with escalating recovery
- Zone graph reading for multi-zone route planning

### Multi-Client Coordination

- Discovers and manages all EQ client sessions automatically
- Staggered launch and login coordination for large box crews
- Per-client session monitoring with self-healing crash recovery
- Encrypted credential store for account management

### Soul Engine

Character personalities powered by configurable trait profiles (Big Five + EQ-themed traits). Each character has persistent autobiographical memory, social graph tracking, and idle behavior generation.

---

## Architecture

TextQuest is a **Rust workspace** with three crates:

| Crate | Role |
|-------|------|
| `textquest` | External orchestrator, TUI, camp loop, launcher |
| `textquest-dll` | In-process EQ integration for direct game control |
| `textquest-common` | Shared types for IPC, navigation, and game data |

The orchestrator runs outside EQ, reads game state, renders the TUI, and makes all orchestration decisions. The in-process component exposes game control surfaces and publishes state snapshots back to the orchestrator via IPC.

A **web dashboard** (`textquest-web`, Axum + React SPA) provides a browser-based view for session monitoring and configuration.

---

## Milestone Roadmap

| Milestone | Status |
|-----------|--------|
| M1 — Process Discovery & Memory Reading | Complete |
| M2 — Input Dispatch & Multi-Client Manager | Complete |
| M3 — Navigation & Pathfinding | Complete |
| M4 — Login Automation | Complete |
| M5 — Anti-Cheat Research | Complete |
| M6 — Web Dashboard | Complete |
| M7 — Zoning / Movement | In Progress |
| M8 — Orchestrator Loop | In Progress |
| M9 — Learning / RL | Planned |
| M10 — Economy | Planned |
| M11 — Soul Engine + LLM | Planned |

---

## Build & Install

TextQuest targets Windows (live EQ clients) with macOS/Linux support for development via demo mode.

```bash
# Clone and build (macOS / Linux — demo mode)
git clone https://github.com/Maleick/TextQuest
cd TextQuest
cargo build
cargo run        # launches TUI with demo data

# Run tests
cargo test

# Dev preflight (same checks as CI)
python3 scripts/dev-preflight.py
```

**Windows requirements:** Rust nightly (MSVC toolchain), Visual Studio Build Tools.

See the [Installation and Build wiki page](https://github.com/Maleick/TextQuest/wiki/Installation-and-Build) for the full setup guide.

---

## Configuration

| File | Purpose |
|------|---------|
| `config/textquest.toml` | Main config — process, polling, group settings |
| `config/accounts.toml` | Per-account name, server, character, class, group |
| `config/camps/*.toml` | Camp definitions — zone, pull point/radius, mana thresholds |
| `config/classes/*.toml` | 16 class ability configs with cooldowns and priorities |

---

## Documentation

Full documentation lives in the [TextQuest Wiki](https://github.com/Maleick/TextQuest/wiki):

- [Quick Start](https://github.com/Maleick/TextQuest/wiki/Quick-Start)
- [Operating the TUI](https://github.com/Maleick/TextQuest/wiki/Operating-the-TUI)
- [Configuration](https://github.com/Maleick/TextQuest/wiki/Configuration)
- [Combat and Camp Loop](https://github.com/Maleick/TextQuest/wiki/Combat-and-Camp-Loop)
- [Navigation and Maps](https://github.com/Maleick/TextQuest/wiki/Navigation-and-Maps)
- [Architecture Overview](https://github.com/Maleick/TextQuest/wiki/Architecture-Overview)

---

## Contributing

PRs welcome. See [CONTRIBUTING.md](https://github.com/Maleick/TextQuest/blob/master/CONTRIBUTING.md) for the development workflow, branch conventions, and CI requirements.

The repo uses an autonomous agent pipeline ([AGENTS.md](https://github.com/Maleick/TextQuest/blob/master/AGENTS.md)) for issue-driven development. Issues tagged `agent-ready` are routed to the agent queue automatically.

---

_Built with Rust. Runs on Frostreaver._
