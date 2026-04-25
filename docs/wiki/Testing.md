# Testing and Platform Stubs

This document describes the testing infrastructure and how stub implementations enable TextQuest to compile and test on non-Windows platforms.

## Overview

TextQuest contains platform-specific code for Windows (DLL injection, process memory reading, window enumeration). To support development and testing on macOS, we provide no-op stub implementations for Windows-only types and functions. This allows:

- **Compilation on macOS**: All code paths compile without #[cfg(windows)] guards on every module
- **Unit testing**: Tests run on macOS against mock data, not real EQ processes
- **Type safety**: The same trait bounds and signatures work across platforms

## Platform Gates

All platform-specific code uses explicit gates:

```rust
#[cfg(windows)]
fn real_implementation() { /* ... */ }

#[cfg(not(windows))]
fn stub_implementation() { /* ... */ }
```

We use `#[cfg(windows)]` / `#[cfg(not(windows))]` consistently, never `#[cfg(target_os = "...")]`.

## Stub Implementations

### Process Memory Reading

**Types**: `ProcessHandle` in `textquest/src/process/memory.rs`

| Method          | Windows                                             | Non-Windows                          |
| --------------- | --------------------------------------------------- | ------------------------------------ |
| `open(pid)`     | Opens real process handle via `OpenProcess`         | Returns stub handle                  |
| `read<T>()`     | Calls `ReadProcessMemory`                           | Returns error                        |
| `read_string()` | Reads null-terminated string from process           | Returns error                        |
| `read_bytes()`  | Reads raw bytes from process                        | Returns error                        |
| `module_base()` | Queries actual module base via `EnumProcessModules` | Returns preferred base `0x140000000` |

The non-Windows stub for `module_base()` returns the preferred base address, allowing offset calculations to work in tests.

### EQ Process Reader Trait

**Location**: `textquest/src/testing/mocks.rs`

Three implementations of the `EqProcessReader` trait:

1. **MockProcessReader** (all platforms)
   - Pre-configured memory values for deterministic testing
   - Used in unit tests to simulate process state
   - Builder pattern: `.with_value()`, `.with_string()`, `.with_wide_string()`

2. **RealProcessReader** (Windows only)
   - Wraps `ProcessHandle` to read actual EQ process memory
   - Fails on non-Windows platforms (by design — no real process to read)

3. **RealProcessReader stub** (non-Windows)
   - Identical type signature to Windows version
   - All methods return errors explaining that process reading is unsupported
   - Enables code that accepts `EqProcessReader` to compile on all platforms

### Window Enumeration

**Type**: `WindowHandle` in `textquest/src/process/window.rs`

```rust
pub struct WindowHandle {
    #[cfg(windows)]
    pub hwnd: windows::Win32::Foundation::HWND,
    pub title: String,
    pub pid: u32,
}
```

Functions:

- `find_windows_by_title(substring)` — Returns empty vec on non-Windows
- `get_foreground_pid(pids)` — Returns None on non-Windows

## Testing Strategy

### Unit Tests

Tests run on both Windows and macOS:

```bash
cargo test --lib
```

**On Windows**: Tests use `RealProcessReader` to test against actual memory structures (when available).

**On macOS**: Tests use `MockProcessReader` for deterministic, isolated testing. Real process reading is not available.

### Platform-Specific Tests

```rust
#[cfg(not(windows))]
#[test]
fn non_windows_stub_fails_appropriately() {
    let mut reader = RealProcessReader::new(1234).unwrap();
    assert!(reader.read::<u32>(0x140000000).is_err());
}
```

Platform-specific tests live in `#[cfg(...)] mod tests { ... }` blocks within the module.

## Integration Tests

Located in `textquest/tests/scenarios/`, these run on all platforms using mocked process state. No live EQ connection is required.

## Mock Infrastructure

The `textquest::testing::mocks` module provides:

- **EqProcessReader trait** — Abstract interface for reading process memory
- **MockProcessReader** — Configurable test double with builder pattern
- **RealProcessReader** — Production reader (Windows) / no-op stub (non-Windows)

`textquest::testing::scenario` now also includes reusable scenario stubs and test-data generators for integration tests that should run on non-Windows:

- **MockScenario** — Deterministic pass/fail scenario with optional attached metrics.
- **CountdownScenario** — Scenario that decrements a remaining tick counter and fails when exhausted.
- **FastFailScenario** — Scenario that fails immediately for negative-path coverage.
- **account_info** — Compact builder for `AccountInfo`.
- **spawn_entry / spawn_wave** — Builders for realistic `SpawnData` test fixtures.

Example usage in tests:

```rust
use textquest::testing::MockProcessReader;

let mut mock = MockProcessReader::new(1234)
    .with_value(0x140000000, 0xCAFEBABEu32)
    .with_string(0x140000100, "TestCharacter");

let value: u32 = mock.read(0x140000000)?;
assert_eq!(value, 0xCAFEBABE);
```

## Windows-Only Code in DLL

The `textquest-dll` crate (Windows-only cdylib) contains:

- Offset-based memory reads and writes
- Hook installation (via retour detour)
- Window enumeration
- Message passing (IPC)

All DLL modules compile with `#[allow(dead_code)]` because the entire DLL is dead code on macOS. Module-level stubs are not required — the dead_code lint is suppressed.

## Acceptance Criteria

✅ Stub implementations compile on macOS

- All code paths compile; no unresolved types or functions on non-Windows

✅ At least 2 platform-gated types have stubs

- `RealProcessReader` (Windows impl + non-Windows stub)
- `ProcessHandle` (Windows methods + non-Windows fallbacks)
- `WindowHandle` methods (`find_windows_by_title`, `get_foreground_pid`)

✅ Integration tests run to completion on macOS

- `cargo test --lib` passes on macOS
- Mock-based scenarios run without Windows dependencies

## Running Tests

```bash
# All unit tests (macOS or Windows)
cargo test --lib

# Specific crate
cargo test --lib -p textquest

# With logging (shows stub warnings)
RUST_LOG=debug cargo test --lib

# On macOS (cross-compile)
cargo test --lib --target aarch64-apple-darwin
```

## See Also

- `.wolf/cerebrum.md` — Platform gate conventions
- `textquest/src/process/` — Memory and window interfaces
- `textquest/src/testing/` — Mock implementations
