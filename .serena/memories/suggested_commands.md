# Suggested Commands

## Build & Run
```bash
cargo build              # Debug build (works on macOS with stubs)
cargo build --release    # Release build
cargo run                # TUI mode (demo mode on macOS, live on Windows)
cargo run -- --dump      # One-shot CLI dump mode
```

## Code Quality
```bash
cargo clippy             # Lint
cargo fmt --check        # Check formatting
cargo fmt                # Auto-format
```

## Notes
- No tests exist yet
- macOS builds use stub implementations for Windows APIs (`#[cfg(windows)]`)
- TUI runs on macOS with demo data from `src/tui/run.rs:load_demo_data`

## System Utils (Darwin/macOS)
- `git` — version control
- `ls`, `find`, `grep` — standard file system tools (Darwin variants)
