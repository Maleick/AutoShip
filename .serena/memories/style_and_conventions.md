# Code Style & Conventions

## Rust Conventions
- Edition 2024
- Standard Rust naming: `snake_case` for functions/variables, `PascalCase` for types/traits
- No tests yet — no test conventions established
- Minimal doc comments; code is expected to be self-documenting

## Key Patterns
- **Offset rebasing**: All EQ pointers in `offsets.rs` are absolute preferred-base addresses (`0x140000000`). Use `offsets::rebase(preferred_addr, actual_base)` at runtime.
- **Field-by-field reads**: `SpawnInfo` is populated by individual `proc.read::<T>(addr + OFFSET)` calls, not by reading a C struct wholesale.
- **Spawn linked list**: Walk `NEXT` pointers with max-count safety limit.
- **Cross-platform stubs**: All Windows APIs behind `#[cfg(windows)]` with macOS/Linux stubs.
- **FSM pattern**: Navigation, combat, login, and puller all use finite state machine patterns.
- **Trait-based strategies**: `ClassStrategy` trait for per-class combat implementations.

## Project Structure
- `dmft/` — Main orchestrator crate (TUI, process reading, coordination)
- `dmft-dll/` — Injected DLL crate (cdylib, hooks, in-process engines)
- `dmft-common/` — Shared types across crates
- `config/` — TOML configuration files
- `mq2-reference/` — MacroQuest2 source for offset extraction (gitignored)
