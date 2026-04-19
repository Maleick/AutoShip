# Common Rust Patterns in TextQuest

This guide documents recurring patterns and conventions used throughout the TextQuest codebase. Follow these patterns when adding new code.

---

## Error Handling with `anyhow::Result<T>`

**Pattern**: All fallible functions return `anyhow::Result<T>` from the `anyhow` crate.

**Why**: Provides a unified error type with context chains, avoiding boilerplate. Errors bubble up naturally and can be logged with full context.

### Example: Reading EQ Offsets

```rust
use anyhow::{anyhow, Context, Result};

fn read_player_position(pid: u32) -> Result<(f32, f32, f32)> {
    let handle = ProcessHandle::open(pid)
        .context("Failed to open process")?;
    
    let base = handle.get_module_base("eqgame.exe")
        .context("Failed to find eqgame.exe module")?;
    
    let x = handle.read_f32(base + 0x34a00)
        .context("Failed to read player X coordinate")?;
    let y = handle.read_f32(base + 0x34a04)
        .context("Failed to read player Y coordinate")?;
    let z = handle.read_f32(base + 0x34a08)
        .context("Failed to read player Z coordinate")?;
    
    Ok((x, y, z))
}

fn main() -> Result<()> {
    let (x, y, z) = read_player_position(1234)
        .context("Could not retrieve player position")?;
    println!("Player at ({}, {}, {})", x, y, z);
    Ok(())
}
```

**Key points**:
- Use `.context()` to add context as you propagate errors
- Use `anyhow!()` to create errors from scratch
- Never use `.unwrap()` in production code
- Use `?` operator to propagate errors

---

## Platform Gates with `#[cfg(windows)]`

**Pattern**: Windows-specific code (DLL injection, process APIs, memory reading) is gated behind `#[cfg(windows)]`. macOS/Linux get stubs.

**Why**: TextQuest builds on all platforms, but live EQ control only works on Windows. Demo mode stubs allow cross-platform compilation and testing.

### Example: Process Discovery

```rust
#[cfg(windows)]
pub fn find_eq_process() -> Result<ProcessHandle> {
    // Real implementation using Windows APIs
    use winapi::um::processthreadsapi::*;
    
    // Search running processes for eqgame.exe
    let processes = ProcessSnapshot::all()?;
    let eq = processes.iter()
        .find(|p| p.name() == "eqgame.exe")
        .ok_or_else(|| anyhow!("EQ not running"))?;
    
    Ok(ProcessHandle::from(eq))
}

#[cfg(not(windows))]
pub fn find_eq_process() -> Result<ProcessHandle> {
    // Stub: return a demo process handle
    Err(anyhow!(
        "Live EQ process discovery only supported on Windows. Running in demo mode."
    ))
}
```

**Key points**:
- Use `#[cfg(windows)]` for Windows-only functions
- Use `#[cfg(not(windows))]` for the stub/fallback
- Both branches must exist; don't leave `#[cfg(not(windows))]` branches empty
- Tests for Windows code should be `#[cfg(test)]` + `#[cfg(windows)]` (skip on CI)

### Example: Conditional Compilation in Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn test_inject_dll() {
        // Only runs on Windows
        let result = inject_dll(1234, "textquest.dll");
        assert!(result.is_ok());
    }

    #[test]
    fn test_demo_mode() {
        // Runs on all platforms
        let app = TextQuestApp::new_demo();
        assert_eq!(app.clients.len(), 6); // Demo has 6 clients
    }
}
```

---

## Tracing and Structured Logging

**Pattern**: Use `tracing` spans and events for observable logging. Every important state transition, error, and performance-sensitive operation should be logged.

**Why**: Enables post-mortem debugging, performance profiling, and operational monitoring without recompiling.

### Example: Camp Loop with Spans

```rust
use tracing::{info, debug, span, Level};

#[tracing::instrument(skip(camp, group))]
pub async fn camp_loop(
    camp: &CampConfig,
    group: &Group,
    mut rx: Receiver<CampCommand>,
) -> Result<()> {
    let span = span!(Level::INFO, "camp_loop", zone = camp.zone);
    let _guard = span.enter();
    
    info!(
        camp.center = ?camp.center,
        camp.radius = camp.radius,
        "Starting camp loop"
    );
    
    loop {
        debug!("Checking for spawns");
        let spawns = group.nearby_spawns(camp.center, camp.radius)?;
        info!(spawn_count = spawns.len(), "Found spawns");
        
        if spawns.is_empty() {
            debug!("No spawns, recovering");
            // Recovery logic
        } else {
            debug!("Starting pull");
            self.pull_next_spawn(&spawns[0]).await?;
        }
        
        // Handle commands
        if let Ok(cmd) = rx.try_recv() {
            match cmd {
                CampCommand::Stop => {
                    info!("Camp loop stopped by operator");
                    break;
                }
                CampCommand::MoveCenter(new_center) => {
                    info!(new_center = ?new_center, "Moving camp center");
                }
            }
        }
    }
    
    info!("Camp loop ended");
    Ok(())
}
```

**Key points**:
- Use `#[tracing::instrument]` on async functions and important sync functions
- Use `span!()` for nested context
- Use `info!()` for significant events (camp started, enemy died, login complete)
- Use `debug!()` for per-frame or high-frequency events (spawn checks, ability rotations)
- Use `warn!()` for recoverable errors (missed heal, stuck detection)
- Use `error!()` for unrecoverable failures
- Structured fields are logged with `key = value` syntax

### Running with Different Log Levels

```bash
# See everything
RUST_LOG=debug cargo run

# Only important events
RUST_LOG=info cargo run

# Only warnings and errors
RUST_LOG=warn cargo run

# Specific crate
RUST_LOG=textquest::camp=debug cargo run

# Multiple crates
RUST_LOG=textquest::camp=debug,textquest::combat=info cargo run
```

---

## SpawnInfo Field Access

**Pattern**: Game entity data (spawns, players, NPCs) is accessed through `SpawnInfo` structs that abstract pointer chasing and offset calculation.

**Why**: Centralizes offset management; makes code robust to EQ patches.

### Example: Reading Spawn Data

```rust
use textquest_common::spawn::{SpawnInfo, StandState};

pub fn is_valid_pull_target(spawn: &SpawnInfo, group_level: u8) -> bool {
    // Use SpawnInfo getters instead of raw pointers
    if spawn.is_player() {
        return false; // Don't pull players
    }
    
    if spawn.is_corpse() {
        return false; // Don't pull corpses
    }
    
    // Check difficulty based on level delta
    let level_delta = (spawn.level() as i32) - (group_level as i32);
    if level_delta > 10 {
        debug!("Spawn {} is too high level ({})", spawn.name(), spawn.level());
        return false;
    }
    
    // Check if already targeted by another group
    if spawn.is_being_attacked() {
        return false;
    }
    
    true
}
```

### Example: Writing to SpawnInfo

```rust
pub fn update_player_position(spawn: &mut SpawnInfo, x: f32, y: f32, z: f32) -> Result<()> {
    spawn.set_position(x, y, z)
        .context("Failed to update spawn position")?;
    Ok(())
}
```

**Key points**:
- Never read offsets directly; use `SpawnInfo` getters
- `SpawnInfo` is typically `&self` (read-only) for most operations
- Use `&mut SpawnInfo` only for in-place mutations (rare)
- Offsets are validated at startup; if a getter returns `None`, it's a post-patch issue
- See `textquest-common/src/spawn.rs` for the full API

---

## StandState Enum Matching

**Pattern**: Character stance (sitting, standing, feigned, dead) is represented as a Rust enum and matched explicitly.

**Why**: Type-safe; prevents accidentally treating dead characters as alive.

### Example: Stand State in Combat

```rust
use textquest_common::stand_state::StandState;

pub async fn rest_until_ready(character: &Character, threshold: f32) -> Result<()> {
    // Ensure we're sitting
    if !character.is_standing() {
        character.command("/sit")?;
    }
    
    loop {
        match character.stand_state() {
            StandState::Sitting => {
                // OK, we're sitting
                debug!("Character sitting, regenerating");
            }
            StandState::Standing => {
                // Got knocked up or something; sit again
                info!("Character stood up unexpectedly");
                character.command("/sit")?;
            }
            StandState::FeignedDeath => {
                // This shouldn't happen in rest; something's wrong
                warn!("Character in feigned death during rest");
                character.command("/stand")?;
            }
            StandState::Dead => {
                // Critical error; character is dead
                return Err(anyhow!("Character died during rest"));
            }
            StandState::Unknown(code) => {
                // New stance we don't recognize
                warn!(stand_state_code = code, "Unknown stand state");
            }
        }
        
        if character.mana_pct() > threshold {
            debug!(mana_pct = character.mana_pct(), "Mana recovered");
            break;
        }
        
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    Ok(())
}
```

### Example: Navigation and StandState

```rust
pub fn can_navigate(character: &Character) -> bool {
    // Can only navigate if standing or moving
    matches!(
        character.stand_state(),
        StandState::Standing | StandState::Moving
    )
}

pub fn apply_feign_death_for_evade(character: &Character) -> Result<()> {
    // Only works if standing
    if !matches!(character.stand_state(), StandState::Standing) {
        return Err(anyhow!("Cannot feign death while sitting"));
    }
    
    character.command("/feigndeath")?;
    
    // Wait for the stand state to change
    let mut attempts = 0;
    while !matches!(character.stand_state(), StandState::FeignedDeath) {
        tokio::time::sleep(Duration::from_millis(50)).await;
        attempts += 1;
        if attempts > 20 {
            return Err(anyhow!("Feign death did not apply"));
        }
    }
    
    Ok(())
}
```

**Key points**:
- Always use `match` on `StandState`, never `if let`
- Include a catch-all for `StandState::Unknown` (new EQ features)
- Never assume a character is standing; explicitly check the state
- `StandState::FeignedDeath` is distinct from `StandState::Dead`
- See `textquest-common/src/stand_state.rs` for the full enum

---

## Hot Path Optimization: Precompute Cooldown Keys

**Pattern**: Cooldown tracking uses pre-hashed keys that are computed once at ability initialization, not per-check.

**Why**: The combat rotation evaluates 100+ ability conditions every frame. Computing hash keys per-check is expensive; precomputing saves CPU.

### Example: Ability Cooldown Tracking

```rust
use textquest_common::cooldown::{AbilityCooldownTracker, CooldownKey};

pub fn build_rotation(abilities: Vec<AbilityDef>) -> Result<Rotation> {
    let mut tracker = AbilityCooldownTracker::new();
    let mut entries = Vec::new();
    
    for ability in abilities {
        // Precompute the cooldown key ONCE at build time
        let cooldown_key = stable_cooldown_key("warrior", &ability.name);
        
        // Store it in the entry
        entries.push(RotationEntry {
            name: ability.name.clone(),
            action_type: ability.action.clone(),
            cooldown_key, // <-- Stored, not recomputed per frame
            cooldown_ticks: ability.cooldown_ticks,
            condition: ability.condition.clone(),
        });
    }
    
    Ok(Rotation { entries, tracker })
}

pub async fn evaluate_rotation(rotation: &Rotation) -> Result<Option<Action>> {
    for entry in &rotation.entries {
        // During evaluation, just use the precomputed key
        if rotation.tracker.is_ready(&entry.cooldown_key) {
            // Check conditions and return action
            return Ok(Some(entry.action_type.clone()));
        }
    }
    
    Ok(None)
}
```

### Example: Shared Cooldown Keys

Some abilities share cooldowns (e.g., all disciplines on a shared timer). Use stable keys:

```rust
pub fn stable_cooldown_key(class: &str, ability_name: &str) -> CooldownKey {
    // Return a stable key; same ability always gets same key
    CooldownKey::hash(&format!("{}:{}", class, ability_name))
}

// In rotation TOML:
// [[ability_sets]]
// name = "SpinningSlash"
// shared_cooldown_key = "warrior-offensive-disc"
// shared_cooldown_ticks = 36000

// The tracker ensures only one of {Spinning Slash, Deflection, Spirit of Rage}
// can be active at a time if they share the key.
```

**Key points**:
- Compute keys at rotation **build time**, not at **evaluation time**
- Use `stable_cooldown_key()` so the same ability always hashes to the same key
- For shared cooldowns, use the exact same key string in multiple entries
- See `textquest-dll/src/combat/ability_cooldowns.rs` for the full tracker API

---

## Testing Patterns

### Test Setup with `test_support`

**Pattern**: Centralized test fixtures in `test_support.rs` reduce boilerplate and keep tests consistent.

```rust
// In textquest/src/test_support.rs
pub fn make_test_state() -> AppState {
    AppState {
        clients: vec![
            TestClient { name: "Client 1", level: 60 },
            TestClient { name: "Client 2", level: 60 },
        ],
        camp: Some(test_camp()),
        rotation_state: RotationState::default(),
        // ... other fields ...
    }
}

pub fn test_camp() -> CampConfig {
    CampConfig {
        zone: "gfaydark",
        center: [100.0, 200.0, 50.0],
        radius: 200.0,
        // ... other fields ...
    }
}

// In tests:
#[test]
fn test_camp_pull() {
    let state = make_test_state();
    let spawns = state.spawns_in_camp();
    assert!(!spawns.is_empty());
}
```

### Platform-Specific Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    #[cfg(windows)]
    fn test_process_injection() {
        // Only on Windows CI
        assert!(inject_dll(1234, "test.dll").is_ok());
    }

    #[test]
    fn test_rotation_engine() {
        // All platforms
        let rotation = load_rotation("warrior");
        assert_eq!(rotation.entries.len(), 15);
    }
}
```

---

## Naming Conventions

| Category | Convention | Example |
|----------|-----------|---------|
| **Module** | lowercase, underscores | `combat_state.rs` |
| **Struct** | PascalCase | `AbilityCooldownTracker` |
| **Enum** | PascalCase | `StandState` |
| **Function** | snake_case | `read_player_position()` |
| **Constant** | UPPER_CASE | `MAX_SPAWNS = 10000` |
| **Offset identifier** | lowercase with underscore | `player_x_offset` |
| **Config file** | kebab-case | `hvt_watchlist.toml` |
| **Zone names** | lowercase | `gfaydark`, `oasis` |

---

## Summary

- **Errors**: Always use `anyhow::Result<T>` and chain context
- **Platforms**: Gate Windows code with `#[cfg(windows)]`; stub on other platforms
- **Logging**: Use `tracing` spans and structured fields
- **Game data**: Access via `SpawnInfo` and enum matchers, never raw pointers
- **Performance**: Precompute keys and state at build time, not per-frame
- **Tests**: Use centralized fixtures and platform gates
- **Names**: Follow Rust conventions (snake_case for functions, PascalCase for types)

See the codebase files for more examples:
- `textquest-dll/src/combat/ability_cooldowns.rs` — Cooldown tracking
- `textquest-common/src/spawn.rs` — SpawnInfo API
- `textquest/src/test_support.rs` — Test fixtures
- `textquest-dll/src/hooks.rs` — Tracing in hot paths
