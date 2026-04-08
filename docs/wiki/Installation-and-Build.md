# Installation and Build

## Current Build Requirements

### All platforms

- Rust toolchain with edition 2024 support
- No `.env` file is required for normal build, test, or runtime work
- `.cargo/config.toml` already sets `CMAKE_POLICY_VERSION_MINIMUM=3.5` for normal Cargo commands

### Windows production builds

- CMake 3.5+
- LLVM/Clang available for bindgen
- nightly MSVC Rust toolchain

If the navmesh FFI build complains about CMake policy settings, export:

```bash
export CMAKE_POLICY_VERSION_MINIMUM=3.5
```

The repository already sets this through repo configuration, but the explicit export is still a useful fallback when troubleshooting.

## Local Environment Notes

- Codex and other local tooling automatically pick up checked-in repo configuration such as `.cargo/config.toml`.
- There is no required project `.env` file today.
- The main user-provided setup is installing the host tools: Rust, CMake, and on Windows LLVM/Clang.

## Build Commands

### Standard development build

```bash
cargo build
```

### Release build

```bash
cargo build --release
```

### Format, lint, and test

```bash
cargo fmt --check
cargo clippy
cargo test
```

### Run the default TUI

```bash
cargo run
```

### Run the original one-shot dump path

```bash
cargo run -- --dump
```

## Platform Expectations

### macOS and Linux

- Windows-specific process and injection APIs are stubbed out behind `#[cfg(not(windows))]`.
- `cargo run` is expected to open the TUI in demo mode.
- Use this mode for UI and orchestration development, not for live EQ validation.

### Windows

- `cargo run` launches the real app entrypoint and will attach to live EQ processes if found.
- `textquest.exe inject` stages and injects `textquest_dll.dll`.
- `textquest.exe cmd`, `client-status`, `client-status-all`, `nav`, and `zones` all expect live injected clients.

## Important Files and Paths

- Main config: `config/frostreaver.toml`
- Accounts config: `config/accounts.toml`
- Camp configs: `config/camps/*.toml`
- Class configs: `config/classes/*.toml`
- Named watchlists: `config/named_mobs/*.toml`
- HVT watchlist: `config/hvt_watchlist.toml`
- Offsets snapshot: `config/offsets.json`
- Brewall-style maps: `config/maps/*.txt`
- Canonical wiki source: `docs/wiki/*.md`

## Runtime Logs

- Orchestrator log: `logs/textquest.log`
- DLL log on Windows: `%TEMP%/textquest/textquest-dll.log`
- Session token files: `%TEMP%/textquest/token_<pid>.bin` and `%TEMP%/textquest/login_token_<pid>.bin`

## Internals

- `textquest` builds the external orchestrator and TUI.
- `textquest-dll` builds the injected `cdylib`.
- `textquest-common` provides shared command/response, offsets, nav, login, combat, and soul types.
- Navmesh support uses a Detour/Recast bridge from `textquest/src/nav/mesh.rs` plus the C++ shim compiled by the build.

## Current Behavior vs Roadmap

### Current behavior

- Cross-platform compilation is deliberate; the repo is structured so UI and logic work on non-Windows even when live control cannot.
- `config/offsets.json` is a checked-in offset-data snapshot; compiled defaults still live in `textquest-common/src/offsets.rs`.

### Gaps and caveats

- The build succeeding on macOS does not prove Windows injection or EQ compatibility.
- Offsets and login UI automation should be revalidated after live EQ patches, even when Rust tests are green.
