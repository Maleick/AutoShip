# DMFT (Frostreaver) — Project Overview

## Purpose
Frostreaver is a Rust-based EverQuest multibox controller targeting a 36-box setup on a TLP (Time-Locked Progression) server. It consists of:
1. **External process** (`dmft` crate): Reads game state via `ReadProcessMemory`, displays a TUI dashboard, orchestrates login/combat/navigation
2. **Injected DLL** (`dmft-dll` crate): A `cdylib` that hooks internal EQ functions for direct control (movement, casting, navigation)
3. **Shared types** (`dmft-common` crate): Common types shared between the two runtime components

## Tech Stack
- **Language**: Rust (edition 2024)
- **Workspace**: 3 crates — `dmft`, `dmft-dll`, `dmft-common`
- **TUI**: ratatui + crossterm
- **Platform**: Windows target (with macOS stubs for UI dev using demo data)
- **Crypto**: AES-256-GCM + Argon2id for credential storage
- **Storage**: SQLite for credentials, TOML for config, JSON for offset DB

## Milestone Status
- M1 (complete): External memory reading + TUI dashboard
- M2 (complete): DLL injection + IPC (shared memory + named pipes)
- M2.5 (complete): Login automation + credential store
- M3 (complete): Navigation — waypoint pathfinding, FSM, humanization
- M4 (complete): Combat automation — class strategies, puller FSM, combat coordinator
- M5 (in progress): Soul Engine — LLM-driven character personalities
- M6-M8: Future (LLM chat, RL, economy)
