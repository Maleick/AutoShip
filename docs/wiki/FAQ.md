# TextQuest FAQ

Frequently asked questions for operators, developers, and new users exploring TextQuest.

## General

### What is TextQuest?

TextQuest is a Rust-based **EverQuest multibox automation platform** that reads live game state from EQ clients and injects a DLL for direct in-process control. It coordinates multiple characters from a single TUI (Terminal User Interface) dashboard with full autonomy for combat, navigation, looting, camping, and login sequencing.

Key features:
- **DLL injection** — Rust `cdylib` injected into running EQ clients for direct game control
- **TUI dashboard** — 5 operator screens (Characters, Map, Navigation, Debug, Packets)
- **Camp automation** — 6-phase loop (pull → fight → loot → med → buff → recover)
- **Combat engine** — Class-driven rotations for all 16 EQ classes
- **Login automation** — Credential store, staggered launch, post-login sequencing
- **Navigation** — Navmesh pathfinding, waypoint tooling, stuck detection
- **Web dashboard** — Axum REST backend + React SPA for configuration and monitoring
- **Soul Engine** — LLM-backed character personalities and social dynamics

### What platforms does TextQuest run on?

- **Windows** — Full live injection, DLL control, and automation (required for production use)
- **macOS/Linux** — Demo mode only; TUI runs with simulated data for development and UI iteration

### Is TextQuest a MQ2 replacement?

TextQuest is a **complete redesign** with different tradeoffs:

- **Faster iteration** — Native Rust instead of Lua plugins; compile once per build
- **Stronger anti-detection** — Custom DLL + PEB unlink, page encryption, VEH hooks (vs MQ2's extensible plugin system)
- **Modern architecture** — IPC three-channel design (named pipes + shared memory), web dashboard, LLM personalities
- **Learning focus** — Purpose-built for teaching automation, reverse engineering, and game internals

Unlike MQ2, TextQuest has a single operator workflow (TUI-first) and tighter runtime coupling. Both tools solve multibox; choose TextQuest if you want a self-contained, hardened, and Rust-based alternative.

### What is Frostreaver?

**Frostreaver** is the live testing machine — a Windows 11 mini PC where autonomous sessions run end-to-end with real EQ clients and network interaction. It's used for:

- Validating offset compatibility after EQ patches (typically Wednesdays)
- Running farming economy tests (Sebilis camps, Krono generation)
- Live integration testing of new features before merge to master

Access to Frostreaver is SSH-based; see `.claude.local.md` for connection details.

### What is the Neriak theme?

**Neriak** is the primary visual theme for TextQuest's TUI — inspired by the dark elf city of Neriak in EverQuest. It provides:

- High-contrast dark background for extended operator sessions
- Evocative color palette matching EQ's UI aesthetic
- Consistent styling across all 5 TUI screens

The theme is hardcoded in the TUI renderer; alternative themes are not currently configurable.

---

## Architecture & DLL Injection

### How does DLL injection work?

TextQuest injects a custom Rust DLL (`textquest-dll.dll`) into running EQ clients to gain direct in-process control. The process:

1. **Staging** — The DLL is staged to a temporary file with a session token
2. **Injection** — The orchestrator uses `CreateRemoteThread` to load the DLL into the EQ process
3. **Hook installation** — The DLL hooks `CEverQuest::MainLoop` (the game's core update loop)
4. **IPC setup** — The DLL starts an authenticated named-pipe IPC server and shared-memory writer
5. **Command execution** — The orchestrator sends commands to the DLL; the DLL executes them each frame

The DLL remains hidden via:
- **PEB unlink** — Removes the DLL from the process's module list
- **Page encryption** — Code/data pages are encrypted at rest, decrypted on access
- **VEH hooks** — Custom exception handlers for sensitive operations
- **Frame-rate timing** — Commands execute in-frame to avoid detectable async patterns

### What is IPC (Inter-Process Communication)?

IPC is how the **external orchestrator** (TUI, camp loop, login FSM) talks to the **injected DLL**. TextQuest uses a **three-channel design**:

1. **Named pipes (bidirectional)** — For high-latency command/response (login, camp state changes, nav routes)
2. **Shared memory (write-once)** — For high-frequency game state (spawn list, player position, mana, health)
3. **Multicast UDP (optional)** — For peer-discovery between multiple orchestrator instances on the same network

Each channel is authenticated with a per-session token to prevent unauthorized access.

### Which EQ internals does TextQuest depend on?

TextQuest reads and writes offsets into EQ's in-memory data structures, such as:

- **`CEverQuest`** — Main game class; holds player state, active target, camera
- **`SpawnManager`** — Linked list of all spawns (players, NPCs, corpses)
- **`InventorySlot`** — Item and container information
- **`CharacterBase`** — Level, class, abilities, inventory, effects
- **`StandState`** — Sitting/standing/feigned death state machine
- **`CAbility`** — Combat abilities, cooldown timers, spell IDs

Offsets are versioned in `data/` and validated post-patch. See [`docs/wiki/Offsets-EQ-Internals-and-MacroQuest-References.md`](Offsets-EQ-Internals-and-MacroQuest-References.md) for the full reference.

---

## Getting Started

### How do I add a new character?

1. **Create a Daybreak account** and launch an EQ client logged out
2. **Edit `config/accounts.toml`** and add an entry:
   ```toml
   [[accounts]]
   name = "account_name"
   server = "Firiona Vie"        # or your server
   character = "CharacterName"
   class = "WAR"                 # or any of 16 classes
   group = 1                     # 1-6, for group coordination
   ```
3. **Use TUI login** — In the TUI, type `:login account_name` and enter the password when prompted
4. **Verify in Characters screen** — Press `1` to see the roster; your character should appear with status `Logged In`

Passwords are stored securely in an encrypted credential store; they're never written to config files.

### How do I configure a camp?

1. **Visit your camp location** in EQ
2. **Find your coordinates** — Use `:status` in the TUI to see X, Y, Z
3. **Create a camp file** — `config/camps/zone_name.toml`:
   ```toml
   zone = "gfaydark"
   camp_center = [200, -100, 50]     # Where camp is
   camp_radius = 200                 # Radius for pulls
   pull_point = [250, -150, 50]      # Where to stand and pull from
   pull_radius = 500                 # Max distance to pull
   leash_radius = 1000               # Where to let things run
   heal_at_pct = 30                  # Request heal at 30% HP
   mana_regen_secs = 6               # Mana recovery timer
   ```
4. **Start the camp** — Use `:camp start zone_name` in the TUI
5. **Monitor the Characters screen** — Watch DPS, healing, and recovery times

Full guide: [`docs/wiki/Configuration.md`](Configuration.md)

### How do I set up class rotations?

TextQuest comes with pre-tuned rotations for all 16 classes. To customize:

1. **Edit the class file** — `config/classes/warrior.toml` (for example)
2. **Adjust ability priorities** — Each ability has a cooldown, priority, and condition
3. **Per-character overrides** — Create `config/toons/CharacterName.toml` for specific tuning
4. **Reload the DLL** — Use `:inject` in the TUI to reload with new configs

See [`docs/wiki/Operator-Guide.md`](Operator-Guide.md) for rotation syntax and examples.

---

## Gameplay & Zones

### What are zones?

In EverQuest, a **zone** is a discrete geographic area (e.g., `gfaydark`, `oasis`, `highkeep`). TextQuest treats zones as:

- **Navigation boundaries** — Nav routes don't cross zone lines; zoning is a separate FSM
- **Camp containers** — Each camp is scoped to a single zone
- **State transitions** — Zoning resets stance, clears effects, updates player position

Zone names are lowercase and referenced in:
- Camp configs (`zone = "gfaydark"`)
- Navmesh files (`gfaydark.nav`)
- Waypoint commands (`:nav gfaydark zone_entrance`)

Full zone guide: [`docs/wiki/P99-Zone-Guide.md`](P99-Zone-Guide.md)

### How do I troubleshoot a failed login?

TextQuest's login FSM handles credential entry, waiting for the character to load, and post-login sequencing. If a login fails:

1. **Check the TUI** — Press `1` (Characters screen) and look for error messages on the character row
2. **Check credentials** — Use `:login` again and verify the password is correct
3. **Check server connectivity** — Verify your internet connection and that the server is online
4. **Check for stuck states** — The login FSM might be waiting for a widget or prompt; use `:status overview` to see the state machine
5. **Look at logs** — Use `cargo run -- dump` to export a memory snapshot for offline inspection
6. **Check the debug screen** — Press `4` in the TUI to see the hex dump and offset browser

Common causes:
- **Wrong password** — Login will time out waiting for the character select screen
- **Account banned** — EQ will show a prompt; the DLL auto-accepts it (configurable)
- **Character full** — The server rejected the login; check if you're at account limit
- **Stuck in menu** — The login FSM might miss a widget update; retry `:login`

Full guide: [`docs/wiki/Login-Automation.md`](Login-Automation.md)

---

## Development & Architecture

### How is TextQuest structured?

TextQuest is a **6-crate Rust workspace**:

| Crate | Role |
|-------|------|
| `textquest` | Orchestrator: TUI, camp loop, login, nav, IPC client |
| `textquest-dll` | Injected DLL: game hooks, combat, nav, IPC server |
| `textquest-common` | Shared types: IPC, offsets, spawns, enums |
| `textquest-client` | Per-client session management and monitoring |
| `textquest-soul` | LLM-backed personalities and persistent memory |
| `textquest-web` | Axum REST + React SPA for web dashboard |

Each crate is independently testable. See [`docs/wiki/Architecture-Overview.md`](Architecture-Overview.md) for the full data flow and module boundaries.

### What Rust patterns are used?

Common patterns in TextQuest:

1. **`#[cfg(windows)]` platform gates** — Windows-specific code (DLL injection, process APIs) is gated; macOS/Linux get stubs
2. **`anyhow::Result<T>`** — Universal error type; all fallible functions return `anyhow::Result`
3. **`tracing` spans and events** — Structured logging for debugging; use `tracing::info!`, `span!`, etc.
4. **`SpawnInfo` field access** — Game data is accessed through builder patterns and pointer chasing
5. **`StandState` enum matching** — Stance (sitting, standing, feigned, dead) is matched in combat and navigation FSMs

See [`docs/dev/common-patterns.md`](../dev/common-patterns.md) for code examples and best practices.

---

## Troubleshooting

### The TUI won't start

1. **Check Rust version** — Run `rustc --version`; must be stable or nightly 2024 edition
2. **Check dependencies** — Run `cargo build` to see if there are missing system libraries
3. **Check for EQ clients** — On Windows, TextQuest auto-detects running EQ; if none exist, it falls back to demo mode
4. **Check the logs** — Run `RUST_LOG=debug cargo run` to see verbose output

### The DLL won't inject

1. **Check Windows version** — Windows 10+ required; Windows 7 is not supported
2. **Check admin privileges** — Injection requires admin; re-run the command prompt as Administrator
3. **Check EQ version** — Offsets might be stale after a patch; see [`docs/wiki/patch-day-runbook.md`](patch-day-runbook.md)
4. **Check for crashes** — If EQ crashes immediately after injection, an offset is likely wrong; use Frostreaver's Ghidra instance to validate

### Navigation is stuck

1. **Check the Navigation screen** — Press `3` in the TUI to see waypoint queue and FSM state
2. **Check coordinates** — Use `:status` to see current X, Y, Z
3. **Check the navmesh** — Run `:navmesh diagnostics --pid <PID>` to see collision info
4. **Reset navigation** — Use `:nav clear` to flush the waypoint queue

### Character is dying repeatedly

1. **Check the camp setup** — Use `:camp status` to see pull/leash settings
2. **Check the rotation** — Look at the Classes screen (`class_name.toml`) for ability priority order
3. **Check healing** — Verify clerics/druids have heal rotations configured
4. **Lower difficulty** — Reduce camp radius or pull radius to avoid harder mobs

---

## Support & Community

For bugs, feature requests, and architecture questions:

- **GitHub Issues** — File issues at https://github.com/Maleick/TextQuest/issues
- **Documentation** — Full guides in [`docs/wiki/`](.)
- **Code examples** — See [`docs/dev/`](../dev) for developer patterns and testing
- **AutoShip automation** — TextQuest uses autonomous agents (OpenCode Go, OpenCode Zen free, OpenAI, Claude, Codex) for issue dispatch and PR validation
