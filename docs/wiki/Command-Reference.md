# Command Reference

This page covers both CLI commands and TUI `:` commands.

## CLI Commands

Defined in `textquest/src/main.rs` and implemented in `textquest/src/cli.rs`.

| Command | Purpose |
| --- | --- |
| `cargo run` or `textquest.exe tui` | Launch the TUI dashboard |
| `textquest.exe inject [--pid <pid>]` | Inject the DLL into one or all EQ clients |
| `textquest.exe login <account> [--server <name>] [--character <name>] [--pid <pid>]` | Start automated login |
| `textquest.exe cmd <pid> "/slash command"` | Send a slash command through authenticated IPC |
| `textquest.exe cmd <pid> "/bc ..."` | Relay a box-chat command through the EQBC-style network runtime |
| `textquest.exe nav <pid> <x> <y> <z>` | Navigate one client to coordinates |
| `textquest.exe nav-all <x> <y> <z>` | Navigate all connected clients |
| `textquest.exe nav-path <zone> <x1> <y1> <z1> <x2> <y2> <z2>` | Compute or test a route in one zone |
| `textquest.exe navmesh reload [<zone>] [--pid <pid>]` | Redownload and validate a cached zone navmesh |
| `textquest.exe navmesh diagnostics [<zone>] [--pid <pid>]` | Print cache health and optional live navigator diagnostics |
| `textquest.exe client-status <pid>` | Print one client's live state |
| `textquest.exe client-status-all` | Print a summary table for all clients |
| `textquest.exe zones <pid>` | Query the live zone graph from an injected client |
| `textquest.exe calibrate` | Dump login calibration info |
| `cargo run -- --dump` | Original one-shot dump mode |

Example:

```powershell
target\release\textquest.exe cmd 12345 "/sit"
target\release\textquest.exe navmesh reload gfaydark
target\release\textquest.exe navmesh diagnostics --pid 12345
target\release\textquest.exe client-status-all
target\release\textquest.exe zones 12345
```

## TUI Command Bar

Top-level commands come from `KNOWN_COMMANDS` and the parser in `textquest/src/tui/app.rs`.

### Help and inspection

- `:help`
- `:help <command>`
- `:commands`
- `:cmds`
- `:status`
- `:status overview`

### Camp and navigation

- `:camp start <name>`
- `:camp stop`
- `:camp status`
- `:camp list`
- `:camp add <name>`
- `:camp remove <name>`
- `:camp next`
- `:camp prev`
- `:nav <camp_name>`
- `:nav <x y z>`
- `:nav <zone>`
- `:mode camp`
- `:mode hunt`
- `:loot`

### Combat and group control

- `:ma <character_name>`
- `:mt <character_name>`
- `:engage [target_id]`
- `:disengage`
- `:invite <character_name>`
- `:accept`
- `:heal cancel`

### CH chain

- `:ch start <pid1,pid2,...> <interval_secs> [target_id] [spell_slot]`
- `:ch stop`
- `:ch status`
- `:ch add <pid>`
- `:ch remove <pid>` (`:ch rm <pid>` remains supported)
- `:ch interval <seconds>`
- `:ch adaptive <on|off>`
- `:chui`
- `:chui open`
- `:chui close`
- `:chui toggle`

### Login and lifecycle

- `:login`
- `:login all`
- `:login G<n>`
- `:login <account_name>`
- `:launch ...` as an alias for `:login`
- `:stop <name|all>`
- `:restart <name|all>`

### Tracking and utility

- `:track <spawn name>`
- `:track list`
- `:untrack <spawn name>`
- `:all /slash command`
- `:bc /slash command`
- `:bca //slash command`
- `:bcaa //slash command`
- `:bct <character_name> //slash command`
- `:config`
- `:cfg`
- `:wizard`
- `:theme`
- `:privacy`
- `:quit`
- `:q`

## Aliases and Notes

- `:h` is normalized to `:help`
- `:s` is normalized to `:status`
- `:cfg` is normalized to `:config`
- `:cmds` is normalized to `:commands`
- `:launch` routes to the login flow
- `:bc`, `:bca`, `:bcaa`, and `:bct` use the local `[box_chat]` relay in `config/textquest.toml`

Important current caveat:

- `:inject` is present in the TUI command list, but it is still a placeholder status message, not a full injection action. Use the CLI injection command for live work.

## Internals

- TUI command completion is context-aware and includes camps, tracked names, account names, zone names, and group targets.
- IPC-backed commands eventually flow through `textquest_common::ipc::Command`.
- CLI authenticated commands load `%TEMP%/textquest/login_token_<pid>.bin`, derive the session ID, then connect to the matching named pipe.

## Current Behavior vs Roadmap

### Current behavior

- The command bar is real operational surface area, not a stub.
- CH chain, tracking, status, and camp control are all first-class command paths.

### Roadmap or partial wiring

- Login launch from the TUI is fully stubbed on non-Windows and logs what would have happened.
- Some post-login automation steps are scaffolded but still called out in code comments as future hardening work.
