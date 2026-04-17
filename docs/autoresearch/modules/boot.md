# Boot & Cross-Cutting Modules

Files: `main.rs`, `lib.rs`, `cli.rs`, `config.rs`, `paths.rs`, `command_dispatch.rs`, `crash_reporter.rs`, `alerts.rs`, `box_chat.rs`, `chat_log.rs` + `chat_log/`, `say_detection.rs`, `timestamp_runtime.rs`, `orchestrator_loop.rs`.

---

## `main.rs`

- **Purpose.** Entry point: initialize tracing, resolve log dir, dispatch to a `cli::run_*_mode` handler.
- **Public API.** `fn main() -> Result<()>`.
- **Invariants.**
  - `_tracing_guard` must stay in scope for the whole program (non-blocking `tracing-appender` guard).
  - Daily log rotation, 7-file retention.
  - Env filter defaults: `debug` when dumping, `info` otherwise.
  - Failure to resolve log dir logs a warning but does not abort.
- **Depends on.** `textquest::cli`, `textquest::paths`.

## `lib.rs`

- **Purpose.** Crate root — re-exports submodules, exposes runtime path constants, and supplies a Windows module-base resolver.
- **Public API.**
  - `pub mod cli, config, chat_log, orchestrator_loop, box_chat, …` (19+ submodules).
  - `pub const SOUL_DB_PATH`, `GHIDRA_DB_PATH`, `OPCODES_CONFIG_PATH`.
  - `pub fn get_module_base(proc: &ProcessHandle) -> Result<u64>`.
- **Invariants.**
  - `get_module_base` is Windows-only; non-Windows builds fall back to `textquest_common::offsets::EQ_PREFERRED_BASE`.
  - Soul Engine is an independent workspace crate (`textquest-soul`); `SoulConfig` is nested into `AppConfig`.
  - Several modules carry `#[allow(dead_code)]` — not every feature is active in every build.

## `cli.rs` (2458 LOC)

- **Purpose.** Implements every subcommand `main.rs` can dispatch. Each `run_*_mode` is a standalone handler that may spawn threads or async tasks.
- **Public API (run-mode handlers).**
  `run_tui_mode`, `run_inject_mode`, `run_login_mode`, `run_autologin_mode`, `run_cmd_mode`, `run_nav_mode`, `run_navall_mode`, `run_render_mode`, `run_status_mode`, `run_zones_mode`, `run_calibrate_mode`, `run_orchestrate_mode`, `run_start_mode`, `run_stop_mode`, `run_dashboard_mode`, `run_config_check_mode`, `run_config_show_mode`, `run_credential_add_mode`, `run_credential_list_mode`, `run_credential_remove_mode`, `open_credential_store`, `load_config`.
- **Invariants.**
  - Credential operations prompt for the master password via `rpassword` (hidden from `ps`).
  - PID-based commands connect to injected DLLs via named pipes; commands fail if the DLL is not injected.
  - Navigation subcommands touch the zone-adjacency graph and navmesh cache.
- **Depends on.** `config`, `command_dispatch`, `ipc`, `credentials`, plus `nav`, `process`, `eq`, `client`, `launcher`, `orchestrator`.

## `config.rs` (1277 LOC)

- **Purpose.** Defines the full TOML schema (`AppConfig` et al) and reads `config/textquest.toml` (primary) or `config/frostreaver.toml` (legacy fallback).
- **Public API (top-level structs/enums).**
  - Accounts: `AccountEntry`, `ProfileGroup`, `AccountsConfig`.
  - App: `AppConfig`, `DiscordConfig`, `GroupConfig`, `LaunchConfig`, `ServerConfig`, `RetryConfig`, `OrchestratorConfig`, `PeerDiscoveryConfig`, `SpawnWatchConfig`, `KillTrackerConfig`, `SayDetectionConfig`, `SayRuleConfig`, `BoxChatConfig`.
  - Enums: `PlayerFilterMode`, `SayRuleAction`, `SayPatternType`.
- **Invariants.**
  - Every top-level struct implements `Default`.
  - Empty Discord webhook URLs disable the feature, they are never treated as malformed.
  - Say-detection rules support substring, exact, and regex matchers.
- **Depends on.** `serde`, `toml`, `textquest_common::login::AccountInfo`, `textquest_soul::config::SoulConfig`.

## `paths.rs`

- **Purpose.** Cross-platform runtime path resolution for logs and state.
- **Public API.** `resolve_log_dir`, `orchestrator_log_path`, `dump_log_path`, `dll_log_path`, `rolling_log_path_pattern`, `dump_command_label`, `ensure_writable_dir`, `probe_path`.
- **Invariants.**
  - Search order on Windows: `%AppData%`, exe parent, current dir, temp.
  - Each candidate is probed with a writability check before it is returned.
  - DLL logs are always in the temp dir (Windows only).

## `command_dispatch.rs`

- **Purpose.** Thin dispatcher that converts a "(pid, slash command)" tuple into either a local nav interception or an IPC pipe send.
- **Public API.** `dispatch_local_command(pid: u32, command: &str) -> Result<()>`.
- **Invariants.**
  - First delegate to `nav::try_handle_local_slash_command`.
  - If unhandled, connect to the DLL pipe using the active session token — fails if the DLL is not injected.

## `crash_reporter.rs`

- **Purpose.** Per-character crash context snapshots and recovery command generation.
- **Public API.** `CrashContext`, `CrashReporter::{new, record_context, last_context, on_crash, recovery_commands, format_report}`.
- **Invariants.**
  - One active context per character; overwritten on the next crash.
  - Default recovery is `["/camp desktop"]` when a context exists, otherwise empty.
  - Reports include Unix timestamp + last 20 log lines.

## `alerts.rs` (1068 LOC)

- **Purpose.** Central alert event model used by all subsystems that can raise user-facing notifications.
- **Public API.** Event and severity types plus the dispatch plumbing between emitters (metrics, combat, zoning) and sinks (TUI toast, webhook, audio SFX).
- **Invariants.**
  - `AlertManager::publish` always inserts a fresh row via `store.insert` and dispatches by severity — there is **no** alert-ID dedupe or escalation suppression. Callers are responsible for avoiding duplicate raises.
  - Audio sinks can be muted globally; text/visual sinks stay live.

## `box_chat.rs` (931 LOC)

- **Purpose.** EQBC-style TCP relay — cross-machine slash-command broadcast and targeted sends, so a TUI on one box can steer clients on another.
- **Public API.** `BoxChatManager`, `HubState`, `ListenerHandle`, `ConnectorHandle`, `RuntimeState`, `RuntimeMode`, `configure`, `configure_connector_only`, `stop`, `reload_from_disk`, `update_local_clients`, `dispatch_if_box_chat`, `start_listener`, `start_connector`, `node_name`, `on_off`.
- **Invariants.**
  - Listener + connector are independent roles; either can run alone.
  - Config reload hot-swaps state without dropping existing peers.
  - Route syntax: `character@server:cmd`.
  - IO poll interval 200 ms; connector reconnect every 2 s.

## `chat_log.rs` + `chat_log/{mod,config,writer}.rs`

- **Purpose.** MQ2Log-parity output: per-character log files with rotation + channel filtering.
- **Public API.** `ChatLogConfig`, `ChatChannel` (Say, Tell, Group, Raid, Guild, Ooc, Shout, Auction, …), `LogLevel`, `RotationStrategy`, `ChatLogWriter` (`new`, `reconfigure`, `write_event`).
- **Invariants.**
  - One file per `(server, character)`: `logs/server_charname.log`.
  - Rotation strategies: daily or size.
  - Some channels are always logged regardless of filters (moderation).
  - Writes go through a `BufWriter` keyed per file.

## `say_detection.rs`

- **Purpose.** Rule engine that matches incoming chat text against configured `SayRuleConfig` entries and emits actions (react, alert, script).
- **Invariants.**
  - Pattern types: substring, exact, regex (precompiled at config load).
  - Actions are fire-and-forget; any side effects must be idempotent.

## `timestamp_runtime.rs`

- **Purpose.** Per-character timestamp-format config with filesystem polling, dispatched to the DLL via IPC.
- **Public API.** `TimestampConfig`, `TimestampRuntime::{new, tick, get_config}`, `default_config_path`, `load_from_disk`.
- **Invariants.**
  - Reads `config/timestamp.toml`; polls the mtime every 2 s.
  - Per-character format stored in a `HashMap`; defaults to disabled.
  - Returns `None` from `tick` when the file is unchanged (cheap call).

## `orchestrator_loop.rs` (861 LOC)

- **Purpose.** Top-level async tick loop wiring `ClientManager`, `LaunchCoordinator`, and `Orchestrator` together; runs health checks, death-camp recovery, and crash relogging.
- **Public API.** `OrchestratorLoop::{new, from_config}` + internal tick, `LoopEvent` (`ClientCamped`, `ClientRequeued`, `ClientRegistered`, `PeerDiscovered`, `PeerExpired`, `HealthCheckDone`, `ShuttingDown`).
- **Invariants.**
  - Death stand state = 111 → if per-toon auto-camp is enabled, dispatch `/camp desktop`.
  - Relog dispatch is retryable; failures re-queue.
  - Each tick checks every client's health; unhealthy clients get `/camp desktop`.
  - Timestamp config reloaded every tick (mtime-filtered).
  - Shutdown flows through a tokio `watch` channel; exit is graceful.
  - Auto-camp-on-death is keyed by lowercase character name.
- **Depends on.** `client::manager`, `launcher::coordinator`, `orchestrator`, `timestamp_runtime`, `textquest_common::{combat, ipc}`, tokio.
