# TextQuest Code Style Guide

This guide documents the coding conventions and standards used across the TextQuest project. All new code should follow these guidelines to maintain consistency and readability.

## Rust Edition & Toolchain

- **Edition**: 2021 (or later)
- **MSRV**: 1.70 (minimum supported Rust version)
- **Formatter**: `cargo fmt` (rustfmt configuration in `rustfmt.toml`)
- **Linter**: `cargo clippy --all-targets --all-features -- -D warnings`

## Quality Standards

All code must:
1. **Pass clippy**: `cargo clippy --all-targets --all-features -- -D warnings`
2. **Pass formatting**: `cargo fmt --all -- --check`
3. **Pass tests**: `cargo test --lib` (all existing tests must pass)
4. **Have no dead code**: Remove `#[allow(dead_code)]` unless clearly documented for future use

## Logging & Tracing

- **Use `tracing` instead of `log`**: All diagnostic output uses the `tracing` crate
  - `tracing::trace!()` for low-level frame/tick-level details
  - `tracing::debug!()` for normal runtime diagnostics
  - `tracing::info!()` for significant state changes (logins, zone transitions)
  - `tracing::warn!()` for recoverable issues
  - `tracing::error!()` for critical failures

**Example:**
```rust
use tracing::{info, warn};

info!("Group {} entering zone: {}", group_id, zone_name);
warn!("Failed to find NPC {}, skipping", npc_id);
```

- **Do not use**: `println!`, `eprintln!`, `log::*`, `dbg!`

## Error Handling

- **Use `anyhow::Result<T>` for fallible functions**: Provides context chains and ergonomic error messages
- **No `unwrap()` in production code**: All `.unwrap()`, `.expect()`, and `panic!` calls must:
  - Have `#[cfg(test)]` or `#[cfg(windows)]` guards if platform-specific
  - Be documented with a comment explaining why panic is safe
  - Be reviewed carefully for thread safety and race conditions

**Example:**
```rust
use anyhow::Result;

fn process_packet(data: &[u8]) -> Result<Packet> {
    let packet = Packet::deserialize(data)
        .context("Failed to deserialize packet")?;
    Ok(packet)
}
```

## Platform-Specific Code

- **Use `#[cfg(windows)]` for Windows-only code**: Never use `target_os = "windows"` style directives
- **Use `#[cfg(test)]` for test-only code**
- **Provide stub implementations on macOS** for Windows-only functionality

**Example:**
```rust
#[cfg(windows)]
pub fn inject_dll(pid: u32) -> Result<()> {
    // Windows implementation
}

#[cfg(not(windows))]
pub fn inject_dll(_pid: u32) -> Result<()> {
    // Stub for other platforms
    Ok(())
}
```

## Module Organization

- **One concept per file**: Each file should have a clear, focused responsibility
- **Use `mod.rs` for module organization**: Subdirectories should contain `mod.rs` with public re-exports
- **Document public items**: All public functions, structs, and enums must have doc comments
- **Document complex private items**: Private items that are complex or non-obvious should also be documented

## Naming Conventions

- **Types (PascalCase)**: `struct ClientState`, `enum EqClass`
- **Functions (snake_case)**: `fn process_packet()`, `fn is_player_buffed()`
- **Constants (SCREAMING_SNAKE_CASE)**: `const MAX_CLIENTS: usize = 36`
- **Generics (single uppercase letter or PascalCase)**: `T`, `Id`, `Config`

## Comments & Documentation

- **Module-level doc comments**: Each module should have a `//!` comment explaining its purpose
- **Item doc comments**: Use `///` for public items
- **Inline comments**: Use `//` for explaining *why*, not *what* the code does
- **Section separators**: Use comment lines like `// ─── Section Name ───────────` for readability

**Example:**
```rust
/// Configuration for the autonomous combat rotation.
///
/// Handles ability sequencing, cooldown tracking, and CC detection.
#[derive(Debug, Clone)]
pub struct RotationConfig {
    /// Ordered list of abilities to cast in sequence
    pub abilities: Vec<Ability>,
    /// Cooldown tracker for shared cooldowns
    pub cooldowns: AbilityCooldownTracker,
}
```

## Imports & Visibility

- **Group imports logically**:
  1. Standard library (`use std::...`)
  2. External crates (`use tokio::...`)
  3. Internal modules (`use crate::...`)
  4. Module-relative imports (`use super::...`)

- **Use `pub use` for public re-exports**: Make commonly used types accessible from parent modules
- **Avoid wildcard imports**: Be explicit about what you're importing

## Testing

- **Place unit tests in module-local test modules**: Use `#[cfg(test)] mod tests { ... }`
- **Use `test_support` helpers**: Centralized test fixtures and builders in `textquest/src/test_support.rs`
- **Name test functions clearly**: `test_<what>_<result>` (e.g., `test_cooldown_tracker_advances_time`)
- **Document test intent**: Add comments explaining what scenario the test covers

**Example:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::make_test_state;

    #[test]
    fn test_ability_on_cooldown_returns_false() {
        let mut state = make_test_state();
        state.cooldowns.mark_cooldown(&cooldown_key, ticks);
        assert!(!state.is_ability_ready(&ability_id));
    }
}
```

## Cooldown Tracking

- **Use `AbilityCooldownTracker`** from `textquest-dll/src/combat/ability_cooldowns.rs`
- **All rotation entries must include cooldown info**: `entry_with_cooldown(name, action_type, cooldown_key, ticks)`
- **Use stable keys for shared cooldowns**: `stable_cooldown_key("rotation", ability_key)` for multi-ability timers
- **Precompute stable keys at build time**: Never hash cooldown keys in hot paths (evaluation loops)

**Example:**
```rust
let cast_heal_key = stable_cooldown_key("healing", "cast_heal");
rotation.add_entry(
    entry_with_cooldown("Cast Heal", ActionType::Cast, cast_heal_key, 30)
);
```

## Async & Concurrency

- **Use `tokio` for async runtime**: All async code uses Tokio
- **Avoid blocking calls in async contexts**: Use `tokio::task::spawn_blocking` for CPU-bound work
- **Document Send + Sync requirements**: If a type is shared across threads, document why
- **Use proper synchronization primitives**: `tokio::sync::Mutex`, `Arc`, `RwLock`

## Performance Considerations

- **Avoid allocations in hot paths**: Frame-by-frame code should minimize Vec clones, String copies
- **Precompute derived data**: Build stable keys, hashes, and filters at startup, not during evaluation
- **Use references for large data**: Pass `&Vec`, `&HashMap` instead of cloning
- **Profile before optimizing**: Measure actual bottlenecks with `cargo flamegraph` or profilers

## Dependencies

- **Review before adding**: New dependencies must be justified and reviewed for:
  - Maintenance status and security
  - Binary size impact
  - Async runtime conflicts (avoid multiple async runtimes)
- **Use `patch` sparingly**: Only for temporary fixes; contribute upstream fixes
- **Keep transitive dependencies minimal**: Avoid crates that pull in heavy dependency trees

## Commit Messages

All commits should:
- **Use conventional format**: `type: description (#issue)`
- **Types**: `fix`, `feat`, `refactor`, `docs`, `test`, `perf`, `ci`, `chore`
- **Reference issues**: Include GitHub issue number if applicable
- **Be descriptive**: Explain *why* the change, not just *what* changed

**Examples:**
```
fix: remove dead code from aes_decrypt function (#1244)
refactor: use is_some_and instead of map_or for optional checks (#1244)
docs: create code style guide with conventions (#1244)
```

## Code Review Checklist

Before submitting code for review:
- [ ] Passes `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Passes `cargo fmt --all -- --check`
- [ ] Passes `cargo test --lib`
- [ ] No `#[allow(dead_code)]` on genuinely unused code
- [ ] No `unwrap()` or `panic!()` in production code paths
- [ ] All public items have doc comments
- [ ] Logging uses `tracing` not `log` or `println!`
- [ ] Async code properly handles cancellation and timeouts
- [ ] Platform-specific code uses `#[cfg(windows)]` not target-specific attributes

## References

- [CLAUDE.md](../../CLAUDE.md) — Project-specific architecture and conventions
- `.wolf/anatomy.md` — File registry and module structure
- [Polish Standards](./polish-standards.md) — Code quality baseline
- [Testing Scenarios](./testing-scenarios.md) — Integration test patterns
