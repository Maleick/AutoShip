# Quick Start

This page is the fastest path to a useful TextQuest session.

## Current Behavior

### 1. Optional: initialize the reference submodules

You can skip this for normal build, test, and runtime work.

Run this only before offset, struct, eqlib, or MacroQuest investigation:

```bash
git submodule update --init --recursive
```

These submodules are reference-only and are not part of the normal Cargo inner loop.

### 2. Choose the runtime you are actually in

#### Demo mode

Use this when:

- you are on macOS or Linux
- you are on Windows without a live EQ client
- you are iterating on the TUI or operator workflow

Commands:

```bash
cargo build
cargo run
```

Expected result:

- TextQuest opens the TUI
- the UI shows simulated characters and activity
- the four main screens are usable without EQ running

#### Live Windows mode

Use this when:

- you are on Windows
- `eqgame.exe` clients are running
- you want real injection, IPC, login, nav, or combat behavior

Commands:

```powershell
cargo build --release
target\release\textquest.exe inject
target\release\textquest.exe
```

Useful follow-up commands:

```powershell
target\release\textquest.exe cmd <pid> "/sit"
target\release\textquest.exe status <pid>
target\release\textquest.exe status-all
```

### 3. Learn the four TUI screens

- `1`: Characters
- `2`: Map
- `3`: Navigation
- `4`: Debug

High-value keys:

- `:` open command mode
- `?` toggle help overlay
- `T` cycle theme
- `p` toggle privacy mode
- `[` and `]` change the selected client
- `q` quit

### 4. Use command mode for actual work

Common starting commands:

```text
:status overview
:camp list
:nav <camp|x y z|zone>
:login all
:ch status
:help camp
```

## Internals That Matter Early

- The TUI command parser and help text live in `textquest/src/tui/app.rs`.
- CLI subcommands live in `textquest/src/main.rs` and call into `textquest/src/cli.rs`.
- Pipe and shared-memory naming come from `textquest-common/src/ipc.rs`.
- Demo mode is driven by `textquest/src/tui/demo_data.rs` and non-Windows stubs.

## Current Behavior vs Roadmap

### Current behavior

- Demo mode is the normal non-Windows development path.
- The TUI, map overlays, command bar, command history, and CH panel all work in repo state today.
- The `:inject` TUI command is still a placeholder message, so use the CLI injection path for real injection work.

### Needs live validation

- Multi-client Windows launch, login, and post-login grouping still need real EQ validation after upstream patches.
- Navigation and combat behavior should be treated as "implemented and testable" rather than "universally validated in every zone."
