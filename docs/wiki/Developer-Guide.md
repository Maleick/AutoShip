# Developer Guide

This guide is for developers who want to understand, extend, or contribute to the TextQuest codebase.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Development Workflow](#development-workflow)
3. [Testing Guide](#testing-guide)
4. [Adding New Features](#adding-new-features)
5. [Code Conventions](#code-conventions)
6. [Debugging and Troubleshooting](#debugging-and-troubleshooting)

---

## Architecture Overview

### The Four Crates

TextQuest is organized as a Rust workspace with four crates:

| Crate              | Type         | Purpose                                                                           |
| ------------------ | ------------ | --------------------------------------------------------------------------------- |
| `textquest`        | Binary       | External orchestrator, TUI, config, process reading, injection, camp loop         |
| `textquest-dll`    | DLL (cdylib) | Injected into EQ processes; hooks game loop, controls movement/combat, IPC server |
| `textquest-common` | Library      | Shared types, offsets, IPC protocol, spawn structures, nav and combat logic       |
| `textquest-web`    | Binary       | Axum REST API backend + React SPA for web-based configuration and monitoring      |

### Runtime Modes

#### Demo Mode (non-Windows or no EQ clients)

- TUI loads simulated character data from `textquest/src/tui/demo_data.rs`
- All five screens are interactive
- No process memory reads or DLL injection
- Perfect for UI and orchestration logic development

#### Live Mode (Windows with EQ clients)

- `ReadProcessMemory` reads spawn list and game state
- DLL is injected via reflective loader
- Named pipes and shared memory enable IPC
- All combat, navigation, and login logic executes in the injected DLL

### Data Flow

```
Orchestrator (textquest)
    ├─ TUI renders state
    ├─ Reads game memory via ReadProcessMemory
    ├─ Sends commands to DLL via named pipe
    ├─ Reads live state snapshots from shared memory
    └─ Executes camp loop, login FSM, nav routing

Injected DLL (textquest-dll)
    ├─ Hooks CEverQuest::MainLoop to execute every frame
    ├─ Runs combat FSM (class-driven rotation)
    ├─ Runs nav FSM (movement + stuck recovery)
    ├─ Runs login FSM (widget manipulation)
    ├─ Publishes GameState snapshots to shared memory
    └─ Listens on named pipe for commands
```

### Key Module Boundaries

#### In `textquest` (Orchestrator)

```
src/
├── main.rs              CLI entry point and subcommand parsing
├── cli.rs               Subcommand handlers
├── config.rs            Config file types and loading
├── process/             OS process discovery and memory reading
├── eq/                  External spawn list traversal and state reading
├── tui/                 Ratatui dashboard (5 screens)
│   ├── app.rs           TUI state machine and command parser
│   ├── theme.rs         Color themes
│   ├── demo_data.rs     Simulated state for non-Windows
│   └── ui/              Screen renderers (Characters, Map, Nav, Debug, Packets)
├── inject/              DLL staging and remote-thread injection
├── ipc/                 Named pipe client and shared-memory reader
├── client/              Per-client sessions and state monitors
├── nav/                 Route planning, mesh loading, waypoint queue
├── camp/                Camp FSM phases (pull, fight, loot, med, buff)
├── combat/              Assist logic, CH chain coordination
├── launcher/            EQ launch/login orchestration
├── credentials/         Encrypted password store
└── soul/                Personality, memory, social dynamics
```

#### In `textquest-dll` (Injected DLL)

```
src/
├── lib.rs               DLL entry point
├── hooks/               Game loop, render, input interception
├── eq/                  EQ function bindings (InterpretCmd, etc.)
├── ipc/                 Named pipe server and shared-memory writer
├── nav/                 Navigator FSM, pathfinding, stuck recovery
├── combat/              Combat FSM, 16 class strategies
├── login/               Login FSM and widget manipulation
└── dialog.rs            Auto-accept invites, dialogs
```

#### In `textquest-common` (Shared Types)

```
src/
├── offsets.rs           EQ memory offsets (rebased at runtime)
├── ipc.rs               Command/response types, pipe naming
├── spawn.rs             SpawnInfo structure and traversal
├── nav.rs               NavMesh types, waypoints
├── combat.rs            Combat rotation, ability types
├── login.rs             Login FSM state and types
└── soul.rs              Personality, memory structures
```

### Cross-Crate Dependencies

```
textquest ──┬──→ textquest-common
            └──→ textquest-web (optional)

textquest-dll ──→ textquest-common

textquest-web ──→ textquest-common
```

---

## Development Workflow

### 1. Local Setup

```bash
# Clone the repo
git clone https://github.com/Maleick/TextQuest.git
cd TextQuest

# Verify environment
python3 scripts/dev-preflight.py

# Build in debug mode
cargo build
```

### 2. Pick Your Dev Path

#### UI and Orchestration Work (macOS or Linux)

```bash
# Build and run in demo mode
cargo run

# Make changes to TUI or camp logic
# Rebuild and rerun
cargo run
```

**Files to edit:**

- `textquest/src/tui/` for UI changes
- `textquest/src/camp/` for camp loop logic
- `textquest/src/combat/` for assist/CH logic

#### DLL, Navigation, Combat Work (Windows)

```bash
# Build release binary
cargo build --release

# Inject and test live
textquest.exe inject
textquest.exe tui

# Or test a specific command
textquest.exe nav <pid> 100 200 50
textquest.exe cmd <pid> "/sit"
```

**Files to edit:**

- `textquest-dll/src/combat/` for class rotations
- `textquest-dll/src/nav/` for navigation FSM
- `textquest-dll/src/login/` for login automation

#### IPC and Integration Work

```bash
# Changes to shared types
# Edit: textquest-common/src/ipc.rs

# Rebuild both crates
cargo build
cargo test -p textquest-common
cargo test -p textquest-dll
```

#### SDK Documentation Work

```bash
# Install mdbook
cargo install mdbook

# Build SDK docs
cd textquest-client/docs
mdbook build
mdbook serve  # Preview locally
```

**SDK docs are at:** `textquest-client/docs/src/`

### 3. Before Every Commit

```bash
# Format code
cargo fmt

# Lint with Clippy
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test

# Python tests
python3 -m unittest discover -s tests -p 'test_*.py' -v

# All together (recommended: local format + PR-gate validation)
python3 scripts/dev-preflight.py
```

### 4. When You're Ready to Push

```bash
# Ensure local format + PR-gate validation pass
python3 scripts/dev-preflight.py

# Create a feature branch
git checkout -b feature/your-feature-name

# Make your changes, commit, and push
git add .
git commit -m "feat: describe your change"
git push origin feature/your-feature-name

# Open a pull request on GitHub
```

---

## Testing Guide

### Running Tests

```bash
# Full workspace test suite
cargo test

# Tests for a specific crate
cargo test -p textquest
cargo test -p textquest-dll
cargo test -p textquest-common

# Tests matching a pattern
cargo test test_navigation
cargo test test_combat

# Run one specific test
cargo test -p textquest test_tui_command_parser -- --exact

# Show test output (don't capture)
cargo test -- --nocapture

# Run tests with backtrace on panic
RUST_BACKTRACE=1 cargo test
```

### Platform-Gated Tests

Tests are organized by platform:

```rust
#[cfg(test)]
mod tests {
    // Runs on all platforms
    #[test]
    fn test_spawn_parsing() { }

    // Windows-only
    #[test]
    #[cfg(windows)]
    fn test_dll_injection() { }

    // Non-Windows (macOS/Linux)
    #[test]
    #[cfg(not(windows))]
    fn test_demo_data_initialization() { }
}
```

**Key rule:** Platform-independent tests run on macOS/Linux for rapid iteration. Windows-specific tests (injection, memory reading) require Windows.

### Writing Tests

Follow the pattern in the existing codebase:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camp_transition() {
        // Arrange
        let mut camp = Camp::new("gfaydark", /* ... */);

        // Act
        camp.transition_to_next_phase();

        // Assert
        assert_eq!(camp.current_phase, Phase::Pull);
    }

    #[test]
    fn test_nav_stuck_recovery() {
        // Arrange
        let mut nav = Navigator::new();
        nav.set_position(100.0, 200.0, 50.0);
        nav.set_destination(100.0, 200.0, 50.0); // Same = stuck

        // Act
        nav.update();

        // Assert
        assert!(nav.is_stuck());
    }
}
```

### Python Tests

TextQuest also has Python integration tests:

```bash
# Run all Python tests
python3 -m unittest discover -s tests -p 'test_*.py' -v

# Run a specific test file
python3 -m unittest tests.test_cli_runner

# Run a specific test case
python3 -m unittest tests.test_cli_runner.TestConfigValidation.test_accounts_toml_parsing
```

**Location:** `tests/` directory at repo root.

---

## Adding New Features

### 1. Adding a New TUI Screen

Screens are defined in `textquest/src/tui/app.rs` and rendered in `textquest/src/tui/ui/mod.rs`.

**Steps:**

1. **Add to the enum** (e.g., `ActiveScreen::Inventory`):

```rust
// textquest/src/tui/app.rs
pub enum ActiveScreen {
    Overview,
    Map,
    Navigation,
    Debug,
    Packets,
    Inventory,  // New screen
}

impl ActiveScreen {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Overview => "Characters (1)",
            Self::Map => "Map (2)",
            Self::Navigation => "Navigation (3)",
            Self::Debug => "Debug (4)",
            Self::Packets => "Packets (5)",
            Self::Inventory => "Inventory (6)",  // New
        }
    }

    // Update other match statements in the same file
    // (layout_presets, next_screen, previous_screen)
}
```

2. **Create the renderer** (e.g., `textquest/src/tui/ui/inventory.rs`):

```rust
pub fn render_inventory(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
) {
    // Use ratatui widgets to render your screen
    let items: Vec<&str> = vec!["Sword", "Shield", "Potion"];
    let list = List::new(items);
    f.render_widget(list, area);
}
```

3. **Register in dispatcher** (e.g., `textquest/src/tui/ui/mod.rs`):

```rust
pub fn render_screen(f: &mut Frame, app: &mut App, area: Rect) {
    match app.active_screen {
        ActiveScreen::Overview => { /* ... */ }
        ActiveScreen::Map => { /* ... */ }
        ActiveScreen::Inventory => render_inventory(f, app, area),  // New
        _ => { /* ... */ }
    }
}
```

4. **Add keybinding** in `textquest/src/tui/app.rs`:

```rust
match event.code {
    KeyCode::Char('1') => app.active_screen = ActiveScreen::Overview,
    KeyCode::Char('2') => app.active_screen = ActiveScreen::Map,
    KeyCode::Char('6') => app.active_screen = ActiveScreen::Inventory,  // New
    _ => {}
}
```

5. **Add help text** in the help overlay (search for `help_text()`).

6. **Test in demo mode:**

```bash
cargo run
# Press 6 to see your new screen
```

### 2. Adding an IPC Command

Commands flow from TUI or CLI → named pipe → DLL → game memory.

**Steps:**

1. **Define the command** in `textquest-common/src/ipc.rs`:

```rust
pub enum Command {
    Sit,
    Stand,
    CastSpell { spell_name: String },
    MyNewCommand { param: String },  // New
}
```

2. **Handle in DLL** (`textquest-dll/src/ipc/handler.rs`):

```rust
pub fn handle_command(cmd: Command) {
    match cmd {
        Command::Sit => { /* call InterpretCmd "/sit" */ }
        Command::MyNewCommand { param } => {
            // Implement your command
            unsafe { InterpretCmd(format!("/{}", param)) }
        }
        _ => {}
    }
}
```

3. **Add TUI command** in `textquest/src/tui/app.rs`:

```rust
if input == ":mycommand param" {
    let cmd = Command::MyNewCommand {
        param: "param".to_string(),
    };
    send_command_to_dll(cmd);
}
```

4. **Test:**

```bash
cargo test -p textquest-common
# On Windows with EQ running:
cargo build --release
textquest.exe cmd <pid> "/mycommand"
```

### 3. Adding a New Class Rotation

Class rotations are in `textquest-dll/src/combat/rotations/` or config files.

**Steps:**

1. **Create rotation** in `textquest-dll/src/combat/rotations/your_class.rs`:

```rust
pub fn your_class_rotation(state: &CombatState) -> Vec<Action> {
    vec![
        Action::spell("Your Spell", 10),  // 10 sec cooldown
        Action::ability("Your Ability", 6),
        Action::discipline("Your Discipline", 300),
    ]
}
```

2. **Register in dispatcher** (`textquest-dll/src/combat/rotations/mod.rs`):

```rust
pub fn get_rotation(class: Class) -> Box<dyn Rotation> {
    match class {
        Class::YourClass => Box::new(YourClassRotation {}),
        _ => { /* existing */ }
    }
}
```

3. **Test in demo mode** (rotation logic is platform-independent):

```bash
cargo test -p textquest-dll test_your_class_rotation
```

4. **Validate live** on Windows with EQ running.

### 4. Adding Configuration Options

Configuration is loaded in `textquest/src/config.rs` and stored in `config/textquest.toml`.

**Steps:**

1. **Add to struct** in `textquest/src/config.rs`:

```rust
pub struct AppConfig {
    pub process_name: String,
    pub my_new_option: bool,  // New
    // ...
}
```

2. **Add to TOML** in `config/textquest.toml`:

```toml
process_name = "eqgame"
my_new_option = true
```

3. **Load in main** (`textquest/src/main.rs`):

```rust
let config = Config::load("config/textquest.toml")?;
if config.my_new_option {
    println!("My feature is enabled!");
}
```

4. **Document** in the wiki: [Operator Guide](Operator-Guide.md).

---

## Code Conventions

### 1. Logging: `tracing` not `log`

TextQuest uses `tracing` for structured logging. Never use the `log` crate.

```rust
// GOOD
use tracing::{info, debug, warn, error};

info!("Navigating to coordinates: x={}, y={}", x, y);
debug!("Current state: {:?}", state);
warn!("Failed to read spawn list: {}", err);
error!("Injection failed: {}", err);

// NOT: log::info!(), println!() for structured logs
```

**Logging location:**

- Orchestrator: `logs/textquest.log`
- DLL: `%TEMP%/textquest/textquest-dll.log`

### 2. Platform Gates: `#[cfg(windows)]` not `#[cfg(target_os)]`

Always use `#[cfg(windows)]` for Windows-specific code. Never use `target_os`.

```rust
// GOOD
#[cfg(windows)]
use windows::Win32::System::Memory::ReadProcessMemory;

#[cfg(not(windows))]
fn read_process_memory_stub() {
    // macOS/Linux stub
}

// NOT: #[cfg(target_os = "windows")]
```

### 3. Offset Rebasing: Always `rebase(preferred_base, actual_base)`

Offsets in `offsets.rs` are preferred-base addresses. Always rebase before using:

```rust
// GOOD
let player_ptr = offsets::rebase(PLAYER_BASE, actual_base) as *const PlayerClient;
let spawns = unsafe { (*player_ptr).spawns as *mut SpawnInfo };

// NOT: Using PLAYER_BASE directly without rebasing
```

**Why:** EQ loads at different addresses on each run. Without rebasing, you're reading garbage.

### 4. Field-by-Field Reads, Not Struct Casts

Read EQ structures field by field, not wholesale:

```rust
// GOOD
let spawn_id = proc.read::<u32>(addr + OFFSET_SPAWN_ID)?;
let x = proc.read::<f32>(addr + OFFSET_X)?;
let y = proc.read::<f32>(addr + OFFSET_Y)?;
let z = proc.read::<f32>(addr + OFFSET_Z)?;

// NOT: Reading the entire struct at once
// let spawn = proc.read::<SpawnInfo>(addr)?;
```

**Why:** EQ struct layouts have gaps; offsets from MQ2 are not always contiguous.

### 5. IPC Naming: Use `ipc::pipe_name()` and `ipc::shared_memory_name()`

Named pipes and shared memory names must be derived, not hardcoded:

```rust
// GOOD
use textquest_common::ipc::{pipe_name, shared_memory_name};

let pipe = pipe_name(session_id, client_id);      // -> "\\.\pipe\{session_id:x}_cmd_{client_id}"
let shmem = shared_memory_name(session_id, client_id); // -> "{session_id:x}_state_{client_id}"

// NOT: Hardcoding "textquest_cmd_", "textquest_state_", etc.
```

**Why:** Multiple sessions can run simultaneously. Hardcoded names → collisions and lost state.

### 6. No `unwrap()` in Production Code

Use `?` operator or explicit error handling:

```rust
// GOOD
let config = Config::load("config/textquest.toml")?;
let spawn = proc.read::<u32>(addr).context("Failed to read spawn ID")?;

// NOT: config.unwrap(), proc.read().unwrap()
```

**Why:** Unwrap panics crash the entire orchestrator or DLL. Errors should be logged and recovered.

### 7. const fn Constraints

Drop `const fn` if the function allocates or mutates:

```rust
// GOOD
fn parse_config(path: &str) -> Result<Config> {
    // Allocates, so not const
    let content = std::fs::read_to_string(path)?;
}

// NOT: const fn parse_config() { /* alloc */ }
```

### 8. Comments for Non-Obvious Logic

```rust
// GOOD
// Offset rebase: PLAYER_BASE is preferred-base; actual_base is runtime address
let rebased = offsets::rebase(PLAYER_BASE, actual_base);

// Round-trip through IPC to ensure DLL has processed the command
let result = send_and_wait_for_response(cmd, timeout)?;

// NOT: // This is obvious
let x = spawn.x; // Set X
```

### 9. Tests: Unit Tests In-File, Integration Tests in `tests/`

```rust
// In src/lib.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation() {
        // Test nav logic
    }
}

// In tests/integration_test.rs (multifile integration)
#[test]
fn test_end_to_end_camp_loop() {
    // Test full flow
}
```

### 10. Async Patterns

TextQuest uses Tokio. Keep async/await consistent:

```rust
// GOOD
#[tokio::main]
async fn main() {
    let result = some_async_operation().await;
}

async fn read_game_state() -> Result<GameState> {
    // async code
}

// NOT: Mixing block_on and await, or sync I/O in async context
```

---

## Debugging and Troubleshooting

### Reading Logs

#### Orchestrator Log

```bash
tail -f logs/textquest.log
```

Look for:

- `ERROR`: Fatal issues (config, injection, IPC)
- `WARN`: Recoverable issues (missing offset, nav timeout)
- `INFO`: Major events (DLL injected, camp phase change)
- `DEBUG`: Detailed state (frame-by-frame activity)

#### DLL Log

```powershell
# Windows
Get-Content -Tail 20 -Wait $env:TEMP\textquest\textquest-dll.log
```

Look for:

- `ERROR`: Hook failures, command errors
- `WARN`: Offset misreads, navigation issues
- `INFO`: Main loop updates, FSM transitions
- `DEBUG`: Detailed memory reads, combat decisions

### Debugging DLL Injection

```powershell
# 1. Confirm DLL exists
ls target\release\textquest_dll.dll

# 2. Inject with verbose logging
textquest.exe inject --verbose

# 3. Check token files
dir $env:TEMP\textquest\
# Look for: login_token_<pid>.bin, cmd_<session>_<client>.token

# 4. Try a simple command
textquest.exe cmd <pid> "/sit"

# 5. Check DLL log
Get-Content $env:TEMP\textquest\textquest-dll.log
```

### Debugging Navigation

```powershell
# 1. Inspect navmesh cache
textquest.exe navmesh diagnostics gfaydark --pid <pid>

# 2. Check navigation state
textquest.exe client-status <pid> | grep -A 10 "navigation"

# 3. Try a simpler waypoint
:nav x y z

# 4. Check for stuck state
# Open TUI → Navigation screen (3) → Look for "Stuck"
```

### Debugging Combat/Rotation

**In demo mode:**

```bash
cargo run
# Open Debug screen (4)
# Spawn filter: "your character"
# Look at hex dump to see buff/debuff state
```

**Live (Windows):**

```powershell
# Enable debug logging in DLL
# Edit textquest-dll/src/combat/mod.rs, set log level to DEBUG

cargo build --release
textquest.exe tui
# Watch DLL log for rotation decisions
Get-Content -Tail 50 -Wait $env:TEMP\textquest\textquest-dll.log | Select-String "action"
```

### Debugging Config Issues

```bash
# Validate TOML syntax
python3 -c "import toml; toml.load(open('config/textquest.toml'))"

# Check which config was loaded
grep -i "loaded config" logs/textquest.log

# Print parsed config (add debug code)
eprintln!("{:#?}", config);
```

### Performance Profiling

```bash
# Build with optimizations
cargo build --release

# On Windows, use ETW or profiling tools
# For Rust-specific profiling, use flamegraph:
cargo install flamegraph
sudo cargo flamegraph -- cargo run

# Open flamegraph.svg to find hot paths
```

---

## SDK Development

External applications can interact with TextQuest-managed EQ clients via the IPC protocol.

### SDK Packages

| Language | Package | Registry | Status |
|----------|---------|----------|--------|
| Rust | `textquest-common` | [crates.io](https://crates.io/crates/textquest-common) | Published |
| Python | `textquest` | PyPI | Planned |
| TypeScript | `@textquest/client` | npm | Planned |

### SDK Documentation

Full SDK documentation with language-specific quickstarts and API references is available at:

- **mdBook Docs**: `textquest-client/docs/` (published to GitHub Pages)
- **IPC Protocol**: [Specs-and-Protocols/IPC-Protocol.md](Specs-and-Protocols/IPC-Protocol.md)

### Building the SDK Docs

```bash
# Install mdbook
cargo install mdbook

# Build the SDK documentation
cd textquest-client/docs
mdbook build
```

### Publishing SDK Packages

Publication workflows are defined in:

- `.github/workflows/publish-python.yml` — PyPI publication
- `.github/workflows/publish-npm.yml` — npm publication

**Required secrets (not yet configured):**

| Registry | Secret | Instructions |
|----------|--------|--------------|
| PyPI | `PYPI_API_TOKEN` | Generate at pypi.org/manage/account |
| npm | `NPM_TOKEN` | Generate at npmjs.com/settings/tokens |

The `textquest-common` crate is already published to crates.io via the existing `release.yml` workflow.

## Further Reading

- **[Architecture Overview](Architecture-Overview.md)** — Deep dive into data flow and module boundaries
- **[DLL Injection and IPC Pipeline](DLL-Injection-and-IPC-Pipeline.md)** — How injection and communication work
- **[Navigation and Maps](Navigation-and-Maps.md)** — Navmesh, pathfinding, and stuck recovery
- **[Combat and Camp Loop](Combat-and-Camp-Loop.md)** — Combat FSM, class rotations, camp phases
- **[Login Automation](Login-Automation.md)** — Widget manipulation and credential storage
- **[Development Workflow](Development-Workflow.md)** — PR, CI, and wiki sync process
- **[IPC Protocol Specification](Specs-and-Protocols/IPC-Protocol.md)** — Complete command/response reference

For questions about the EQ memory layout or offsets, see [Offsets, EQ Internals, and MacroQuest References](Offsets-EQ-Internals-and-MacroQuest-References.md).
