# Contributing to TextQuest

TextQuest is a Rust-based EverQuest multibox controller. This guide covers everything you need to build, test, and submit changes.

## Getting Started

### Prerequisites

- **Rust**: Stable toolchain for macOS/Linux builds; nightly MSVC toolchain required on Windows (the `retour` hooking dependency uses unstable features)
- **Python 3**: For the dev preflight script and Python test suite
- **CMake 3.5+**: Required by some dependencies; `CMAKE_POLICY_VERSION_MINIMUM` is already set via `.cargo/config.toml` so Cargo builds handle this automatically

### Clone and Build

```bash
git clone <repo-url>
cd TextQuest

cargo build          # Debug build — works on macOS with stub data
cargo build --release  # Release build — requires nightly MSVC on Windows
```

The TUI runs on macOS in demo mode without a live EQ client, which makes UI development possible cross-platform.

## Documentation and Polish Standards

TextQuest maintains high standards for documentation and code quality. For a complete guide, see [Documentation & Polish Standards](docs/dev/polish-standards.md).

### Issue Requirements

Every task issue must include:
- **Task Description**: Summary and links to parent/related tasks.
- **Scope Section**: What is and isn't included, affected files.
- **Design Section**: Architecture, data structures, API contracts.
- **Implementation Notes**: Algorithms, performance, platform handling.
- **Testing**: Unit/integration scenarios and benchmarks.
- **Acceptance Criteria**: Concrete checklist for completion.

### Pull Request Standards

- **Commit Messages**: Follow Conventional Commits. Explain "why" in the description.
- **Commit Hygiene**: One feature per commit; no WIP/debug commits.
- **Code Comments**: Explain "why", not "what". Document `unsafe` blocks with `// SAFETY:`.
- **Public APIs**: Must be documented with `///` and include examples.

### Code Polish Checklist

- [ ] No `unwrap()` or `panic!()` without justification.
- [ ] No dead code or unused imports.
- [ ] Unit tests cover core logic and edge cases (coverage >70%).
- [ ] No clippy warnings.
- [ ] Formatted with `rustfmt`.
- [ ] Error messages are helpful and actionable.

## Development Workflow

### Branch Naming

| Prefix       | Used by                                    |
| ------------ | ------------------------------------------ |
| `feature/*`  | Human developers — new features            |
| `autoship/*` | AutoShip agent — automated issue work      |
| `claude/*`   | Claude agent — human-initiated agent tasks |
| `fix/*`      | Bug fixes                                  |

### Commit Style

Follow conventional commits: `type(scope): description`

```
feat(tui): add hex dump panel to debug tab
fix(ipc): handle pipe reconnect on client restart
docs(agents): add issue decomposition standard
chore: update offsets for April patch
```

### Pull Request Process

1. Run the dev preflight locally — it bundles local formatting with the same wiki/lint/test/Python validation as the required PR gate:
   ```bash
   python3 scripts/dev-preflight.py
   ```
2. Open a PR against `master`. Branch protection requires all CI checks to pass and all conversations resolved before merge.
3. **Claude/AutoShip agents never merge PRs.** The shared Codex PR manager owns merge decisions.

## Code Standards

### Linting and Formatting

CI enforces these — run them before pushing:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Fix clippy warnings; do not `#[allow(...)]` without a comment explaining why.

### Platform Gates

All Windows-only OS APIs must be behind `#[cfg(windows)]` with macOS/Linux stubs. Never use `#[cfg(target_os = "windows")]` — always use `#[cfg(windows)]` / `#[cfg(not(windows))]`.

### Logging

Use `tracing` + `tracing-appender` for structured file logging. Do not use the `log` crate. Reserve `println!` / `eprintln!` for user-facing CLI/TUI output only.

### Error Handling

Return `Result` from fallible functions. Avoid `.unwrap()` in non-test code — use `?` or explicit error handling with context.

### Mutex Poison Recovery

Static `Mutex` locks in DLL-facing code must recover from poison instead of panicking. In an injected DLL, a panic while holding a global lock poisons that lock; later hook, IPC, or tick paths that call `.unwrap()` / `.expect()` on the same lock can turn one failure into repeated panics inside the host process.

Use the poison-recovery pattern for static mutexes:

```rust
let mut guard = STATIC_STATE
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);
```

Bare `.unwrap()` on mutex locks is acceptable only in test code, and the line must include a `// test-only` comment explaining the scope. Use `textquest-dll/src/combat/mod.rs` as the canonical reference implementation.

### Memory Offsets

All EQ addresses in `textquest-common/src/offsets.rs` are preferred-base (`0x140000000`) values. Always call `offsets::rebase(addr, actual_base)` before using them as runtime pointers. Never treat offset constants as ready-to-dereference pointers.

### Struct Reads

Populate `SpawnInfo` and similar structs via individual `proc.read::<T>(addr + OFFSET)` calls — not by casting a raw memory block to a struct. MQ2 struct layouts have gaps that make wholesale struct reads incorrect.

## Testing

### Run the Full Suite

```bash
cargo test                            # All workspace crates (~3,100 tests)
python3 -m unittest discover -s tests -p 'test_*.py' -v  # Python tests
python3 scripts/dev-preflight.py      # Local format + PR-gate validation
```

### Per-Crate and Targeted Tests

```bash
cargo test -p textquest               # Orchestrator only
cargo test -p textquest-common        # Shared types only
cargo test -p textquest-dll           # DLL only (Windows, or stubs on macOS)

cargo test -p textquest test_name     # Single test by name (substring match)
```

### Writing Tests

- Unit tests go in-file under `#[cfg(test)]` modules.
- Platform-independent tests run on macOS; gate Windows-only tests with `#[cfg(windows)]`.
- Test coverage target: >80% on new logic. Focus on correctness of state machines, IPC round-trips, and memory read helpers.
- Use `self.skipTest()` inside Python test bodies (not `@unittest.skipUnless`) for file-existence guards — self-hosted runner workspaces persist between runs but files may vanish after `actions/checkout`.

## Submitting Work

### Pre-Merge Checklist

- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes (zero warnings)
- [ ] `cargo test` passes on macOS
- [ ] `python3 scripts/dev-preflight.py` passes locally
- [ ] New logic has unit tests
- [ ] Platform-specific code is behind `#[cfg(windows)]`
- [ ] No `.unwrap()` in non-test paths without justification
- [ ] Static `Mutex` locks use `.lock().unwrap_or_else(std::sync::PoisonError::into_inner)` instead of `.unwrap()` / `.expect()`
- [ ] Offsets are rebased before use — not used as raw addresses
- [ ] No credential files, `.env`, or secrets staged

### CI Gate

The required check is **"PR gate (fmt + clippy + test + python)"**. It runs on GitHub-hosted Linux.

The check name is retained for branch-protection compatibility. Local `python3 scripts/dev-preflight.py` still runs `cargo fmt` before the wiki/lint/test/Python gate checks that GitHub enforces.

- **GitHub-hosted Linux**: runs the required PR gate; secret scan also runs here
- **Windows self-hosted**: weekly/manual release validation and tagged release builds
- **Linux self-hosted**: issue/PR automation and Claude agent workflows

Do not merge until the CI gate is green and all review conversations are resolved.

## Common Issues

### Build fails on macOS with missing Windows API

This is expected — Windows-only code paths are stubbed. Check that your new code follows the `#[cfg(windows)]` / `#[cfg(not(windows))]` pattern.

### `CMAKE_POLICY_VERSION_MINIMUM` error

This is normally handled by `.cargo/config.toml`. If you see it outside of a normal Cargo build (e.g., running cmake directly), export it manually:

```bash
export CMAKE_POLICY_VERSION_MINIMUM=3.5
```

### `retour` requires nightly on Windows

Windows release builds require the nightly MSVC toolchain. Stable Rust works on macOS and Linux for development, but the nightly toolchain must be present on Windows CI and release runners.

### Offset values look wrong at runtime

EQ offsets in `offsets.rs` are preferred-base addresses, not runtime pointers. If addresses look wrong, verify you called `offsets::rebase(addr, actual_base)` with the actual loaded module base before use.

### Test imports fail on self-hosted runners

If a Python test imports a file that may not exist (e.g., a config or artifact), use `self.skipTest()` inside the test body rather than a module-level decorator. Self-hosted workspaces persist between jobs but `actions/checkout` clears them.
