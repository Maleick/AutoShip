# Architecture Overview

## Current Architecture

TextQuest is a three-crate Rust workspace:

| Crate              | Role                                                                                                  |
| ------------------ | ----------------------------------------------------------------------------------------------------- |
| `textquest`        | External orchestrator, TUI, config, process reading, injection, launcher, camp loop, Soul coordinator |
| `textquest-dll`    | Injected DLL for in-process EQ control, hooks, IPC server, login/nav/combat FSMs                      |
| `textquest-common` | Shared types for IPC, offsets, nav, combat, login, soul, and wire formats                             |

## Runtime Modes

### TUI mode

- Default mode
- Shows live data when EQ is available
- Falls back to deterministic demo data on non-Windows or when no live EQ client is attached

### Dump mode

- Original one-shot memory-reading path
- Useful for quick inspection without entering the full TUI

## High-Level Data Flow

1. `textquest` discovers or launches EQ clients.
2. If enabled, `textquest` also advertises its current local-session roster over UDP multicast and listens for remote orchestrator peers.
3. `textquest` stages a session token and injects `textquest_dll.dll`.
4. `textquest-dll` hooks into the game, reads internal state, and exposes control surfaces.
5. `textquest-dll` publishes `GameState` snapshots over shared memory.
6. `textquest` reads those snapshots, renders the TUI, and makes orchestration decisions.
7. Operator commands or orchestrator decisions are serialized as IPC commands and sent back to the DLL over authenticated named pipes.

```mermaid
flowchart TD
    subgraph Operator["🖥️ Operator Machine"]
        TUI["TUI Dashboard\n(Characters / Map / Nav / Debug / Packets)"]
        Orchestrator["textquest\norchestrator"]
        CampLoop["Camp Loop FSM\npull→fight→loot→med→buff"]
        LoginSM["Login Coordinator\nstaggered launch + FSM"]
        NavRouter["Nav Router\nnavmesh + route planning"]
        Web["textquest-web\nAxum REST + React SPA"]
    end

    subgraph Common["📦 textquest-common"]
        IpcTypes["IPC Command/Response types"]
        Offsets["EQ Offsets (rebased)"]
        NavTypes["NavMesh / Spawn types"]
    end

    subgraph EQProcess["🎮 EverQuest Process (per client)"]
        DLL["textquest-dll\n(injected cdylib)"]
        GameLoop["GameLoop Hook\nCEverQuest::MainLoop"]
        CombatFSM["Combat FSM\n16 class strategies"]
        NavFSM["Nav FSM\nmovement + stuck recovery"]
        LoginDLL["Login FSM\nwidget manipulation"]
        SharedMem["Shared Memory\nGameState snapshots"]
        Pipe["Named Pipe\nauthenticated IPC"]
    end

    TUI <--> Orchestrator
    Orchestrator --> CampLoop
    Orchestrator --> LoginSM
    Orchestrator --> NavRouter
    Orchestrator <--> Web

    Orchestrator -->|"ReadProcessMemory"| EQProcess
    Orchestrator -->|"inject DLL + token"| DLL
    Orchestrator <-->|"Commands / Results"| Pipe
    Orchestrator <--|"GameState snapshots"| SharedMem

    DLL --> GameLoop
    GameLoop --> CombatFSM
    GameLoop --> NavFSM
    GameLoop --> LoginDLL
    DLL --> SharedMem
    DLL --> Pipe

    Orchestrator --- Common
    DLL --- Common
```

## Key Module Boundaries

### In `textquest`

- `process/`: OS process discovery and memory access
- `eq/`: external memory reading and spawn traversal
- `tui/`: UI state, events, themes, rendering, overlays
- `inject/`: DLL staging and remote-thread injection
- `ipc/`: named pipe client and shared-memory reader
- `client/`: per-client sessions and monitors
- `client/discovery.rs`: optional UDP multicast peer-discovery transport for orchestrator instances
- `nav/`: orchestrator-side route planning and mesh loading
- `camp/`: camp loop phases, buffs, positioning, hunt logic
- `combat/`: assist coordination and CH chain logic
- `launcher/`: EQ spawn/login coordination
- `credentials/`: encrypted credential store
- `soul/`: personality, memory, social graph, idle behavior

### In `textquest-dll`

- `hooks/`: game loop, render, and command execution hooks
- `eq/`: EQ function bindings and UI widget helpers
- `ipc/`: pipe server and shared-memory writer
- `nav/`: navigator FSM, humanization, stuck recovery
- `combat/`: combat FSM, class strategies, HolyShit overrides
- `login/`: in-client login state machine and widget manipulation
- `dialog.rs`: auto-accept handling for invites and similar dialogs

## IPC Three-Channel Design

Communication between the **external orchestrator** and the **injected DLL** uses three independent channels:

### 1. Named Pipes (Bidirectional Command/Response)

- **Protocol** — Binary serialization of `textquest_common::ipc::Command` and `Response` enums
- **Latency** — Sub-millisecond round-trip on local machine
- **Use cases** — Login, camp state changes, navigation route updates, ability queries
- **Authentication** — Per-session token to prevent unauthorized access
- **Reliability** — Messages are ACK'd; timeouts trigger reconnect logic

Example:
```rust
// Orchestrator sends
IpcCommand::SetCampActive { zone: "gfaydark", center: [100, 200, 50] }

// DLL processes and responds
IpcResponse::CampActivated { success: true, spawns_visible: 42 }
```

### 2. Shared Memory (Write-Once, High-Frequency Game State)

- **Protocol** — Memory-mapped file containing `GameState` struct
- **Frequency** — Updated every frame (~60-144 Hz)
- **Size** — ~256 KB per client (spawn list, player pos, mana, health, effects)
- **Readers** — Orchestrator and any monitoring tools can read without blocking
- **Write pattern** — DLL writes; orchestrator reads; no synchronization needed (eventual consistency)

Example:
```rust
// DLL publishes every frame
shared_mem.player_x = 100.5;
shared_mem.player_y = 200.3;
shared_mem.player_z = 50.1;
shared_mem.spawn_count = 42;
// ... update spawn array ...

// Orchestrator reads asynchronously
if shared_mem.player_z < -5000.0 {
    println!("Player is in the water!");
}
```

### 3. Multicast UDP (Optional Peer Discovery)

- **Protocol** — Serialized `PeerAnnouncement` with local orchestrator session ID
- **Frequency** — ~10 second heartbeat
- **Use case** — Auto-discovery of other orchestrator instances on the same network
- **Reliability** — Best-effort; lost packets are non-fatal (next announcement will arrive)

This channel is used for **federated orchestration** where multiple machines need to discover each other, but is optional for single-machine deployments.

## Operator Path Through the System

- TUI input is parsed in `textquest/src/tui/app.rs`.
- CLI commands are parsed in `textquest/src/main.rs`.
- Both eventually issue `textquest_common::ipc::Command` messages or mutate orchestrator state.
- The DLL executes the game-facing behavior and reports results through shared state (channel 2) or async responses (channel 1).
- High-frequency telemetry (spawn position, health) flows through shared memory; low-frequency control flows through named pipes.

## Current Behavior vs Roadmap

### Current behavior

- The split between external orchestration and in-process execution is fundamental to the codebase.
- Demo mode is not an afterthought; it is part of the intended architecture for non-Windows development.

### Roadmap and validation notes

- The current command/control boundary is still authenticated IPC into in-process DLL execution. Packet send-path seams remain research-backed candidates and are tracked in roadmap issues, not live repo capabilities.
- Provider-backed Soul and LLM behavior now belongs to `M11` in the canonical roadmap, after packet, zoning, anti-cheat, orchestration, learning, and economy work.
- Some higher-level flows such as fully automated post-login group formation are present as structure and IPC types, but still need live validation and continued wiring.
