# Architecture Overview

## Current Architecture

DMFT is a three-crate Rust workspace:

| Crate | Role |
| --- | --- |
| `dmft` | External orchestrator, TUI, config, process reading, injection, launcher, camp loop, Soul coordinator |
| `dmft-dll` | Injected DLL for in-process EQ control, hooks, IPC server, login/nav/combat FSMs |
| `dmft-common` | Shared types for IPC, offsets, nav, combat, login, soul, and wire formats |

## Runtime Modes

### TUI mode

- Default mode
- Shows live data when EQ is available
- Falls back to deterministic demo data on non-Windows or when no live EQ client is attached

### Dump mode

- Original one-shot memory-reading path
- Useful for quick inspection without entering the full TUI

## High-Level Data Flow

1. `dmft` discovers or launches EQ clients.
2. `dmft` stages a session token and injects `dmft_dll.dll`.
3. `dmft-dll` hooks into the game, reads internal state, and exposes control surfaces.
4. `dmft-dll` publishes `GameState` snapshots over shared memory.
5. `dmft` reads those snapshots, renders the TUI, and makes orchestration decisions.
6. Operator commands or orchestrator decisions are serialized as IPC commands and sent back to the DLL over authenticated named pipes.

## Key Module Boundaries

### In `dmft`

- `process/`: OS process discovery and memory access
- `eq/`: external memory reading and spawn traversal
- `tui/`: UI state, events, themes, rendering, overlays
- `inject/`: DLL staging and remote-thread injection
- `ipc/`: named pipe client and shared-memory reader
- `client/`: per-client sessions and monitors
- `nav/`: orchestrator-side route planning and mesh loading
- `camp/`: camp loop phases, buffs, positioning, hunt logic
- `combat/`: assist coordination and CH chain logic
- `launcher/`: EQ spawn/login coordination
- `credentials/`: encrypted credential store
- `soul/`: personality, memory, social graph, idle behavior

### In `dmft-dll`

- `hooks/`: game loop, render, and command execution hooks
- `eq/`: EQ function bindings and UI widget helpers
- `ipc/`: pipe server and shared-memory writer
- `nav/`: navigator FSM, humanization, stuck recovery
- `combat/`: combat FSM, class strategies, HolyShit overrides
- `login/`: in-client login state machine and widget manipulation
- `dialog.rs`: auto-accept handling for invites and similar dialogs

## Operator Path Through the System

- TUI input is parsed in `dmft/src/tui/app.rs`.
- CLI commands are parsed in `dmft/src/main.rs`.
- Both eventually issue `dmft_common::ipc::Command` messages or mutate orchestrator state.
- The DLL executes the game-facing behavior and reports results through shared state or async responses.

## Current Behavior vs Roadmap

### Current behavior

- The split between external orchestration and in-process execution is fundamental to the codebase.
- Demo mode is not an afterthought; it is part of the intended architecture for non-Windows development.

### Roadmap and validation notes

- M6 is where real LLM-backed character responses are expected to join the existing Soul scaffolding.
- Some higher-level flows such as fully automated post-login group formation are present as structure and IPC types, but still need live validation and continued wiring.
