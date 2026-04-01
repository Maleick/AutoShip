# Installation and Build

## Current Build Requirements

### All platforms

- Rust stable with edition 2024 support
- Git submodules initialized with:

```bash
git submodule update --init --recursive
```

### Windows production builds

- CMake 3.5+
- LLVM/Clang available for bindgen
- MSVC Rust toolchain

If the navmesh FFI build complains about CMake policy settings, export:

```bash
export CMAKE_POLICY_VERSION_MINIMUM=3.5
```

The repository also sets this through repo configuration, but the explicit export is still a useful fallback when troubleshooting.

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
- `dmft.exe inject` stages and injects `dmft_dll.dll`.
- `dmft.exe cmd`, `status`, `status-all`, `nav`, and `zones` all expect live injected clients.

## Important Files and Paths

- Main config: `config/frostreaver.toml`
- Accounts config: `config/accounts.toml`
- Camp configs: `config/camps/*.toml`
- Class configs: `config/classes/*.toml`
- Named watchlists: `config/named_mobs/*.toml`
- HVT watchlist: `config/hvt_watchlist.toml`
- Offsets DB: `config/offsets.json`
- Brewall-style maps: `config/maps/*.txt`
- Canonical wiki source: `docs/wiki/*.md`

## Runtime Logs

- Orchestrator log: `logs/dmft.log`
- DLL log on Windows: `%TEMP%/dmft/dmft-dll.log`
- Session token files: `%TEMP%/dmft/token_<pid>.bin` and `%TEMP%/dmft/login_token_<pid>.bin`

## Internals

- `dmft` builds the external orchestrator and TUI.
- `dmft-dll` builds the injected `cdylib`.
- `dmft-common` provides shared command/response, offsets, nav, login, combat, and soul types.
- Navmesh support uses a Detour/Recast bridge from `dmft/src/nav/mesh.rs` plus the C++ shim compiled by the build.

## Current Behavior vs Roadmap

### Current behavior

- Cross-platform compilation is deliberate; the repo is structured so UI and logic work on non-Windows even when live control cannot.
- Submodule-backed reference trees are the normal source for offset and struct investigations.

### Gaps and caveats

- The build succeeding on macOS does not prove Windows injection or EQ compatibility.
- Offsets and login UI automation should be revalidated after live EQ patches, even when Rust tests are green.
