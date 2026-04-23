#[cfg(windows)]
use textquest::{cli, log_retention, paths};

#[cfg(windows)]
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
#[cfg(windows)]
use std::path::Path;
#[cfg(windows)]
use textquest::config::LogConfig;
#[cfg(windows)]
use tracing_appender::rolling;
#[cfg(windows)]
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "TextQuest — EverQuest multibox controller",
    long_about = "TextQuest is an EverQuest multibox controller with TUI dashboard, DLL \
                  injection,\nnavigation, combat automation, and web dashboard support.\n\nRun \
                  without arguments to launch the TUI dashboard."
)]
#[command(propagate_version = true)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// One-shot CLI dump mode
    #[arg(short, long)]
    dump: bool,

    /// Inject into eqgame.exe processes
    #[arg(short, long)]
    inject: bool,

    /// Log output format: "text" (human-readable, default) or "json" (structured JSONL)
    #[arg(long, default_value = "text", value_parser = ["text", "json"])]
    log_format: String,
}

fn parse_duration_hours(arg: &str) -> Result<f64, String> {
    let duration = arg.parse::<f64>().map_err(|e| e.to_string())?;
    if !duration.is_finite() {
        return Err("duration must be a finite number".to_string());
    }
    if duration < 0.0 {
        return Err("duration must be >= 0".to_string());
    }
    Ok(duration)
}

#[derive(Subcommand, Debug)]
enum Commands {
    // ── Daemon lifecycle ──────────────────────────────────────────────
    /// Start the TextQuest daemon (TUI + background services)
    Start {
        /// Run in foreground instead of daemonizing
        #[arg(long)]
        foreground: bool,
    },
    /// Stop a running TextQuest daemon gracefully
    Stop,
    /// Show the running daemon's status
    #[command(name = "status")]
    DaemonStatus,

    // ── Dashboard ─────────────────────────────────────────────────────
    /// Launch the web dashboard (Axum + React)
    Dashboard {
        /// Port for the web server
        #[arg(long, default_value = "3001")]
        port: u16,
        /// Open browser automatically
        #[arg(long)]
        open: bool,
    },

    // ── TUI ───────────────────────────────────────────────────────────
    /// Launch the TUI dashboard (default)
    Tui,

    // ── Injection & login ─────────────────────────────────────────────
    /// Inject into eqgame.exe processes
    Inject {
        /// Optional PID to inject into a specific process only
        #[arg(long)]
        pid: Option<u32>,
    },
    /// Automated login
    Login {
        /// Account name from config/accounts.toml
        account: String,
        /// Server name (default: "Firiona Vie")
        #[arg(long, default_value = "Firiona Vie")]
        server: String,
        /// Character name
        #[arg(long)]
        character: Option<String>,
        /// Optional PID to target a specific process only
        #[arg(long)]
        pid: Option<u32>,
    },
    /// End-to-end autologin: spawn → inject → login (reads accounts.toml)
    Autologin {
        /// Login only this account (from accounts.toml). Omit for all accounts.
        #[arg(long)]
        account: Option<String>,
        /// Login only accounts in this group number.
        #[arg(long)]
        group: Option<u32>,
        /// EQ password (same for all accounts). Prefer the `TEXTQUEST_PASSWORD`
        /// env var — CLI flags are visible in process listings (ps/Task
        /// Manager) and shell history.
        #[arg(long, hide = true)]
        password: Option<String>,
        /// Master password for encrypted credential store. Prefer the
        /// `TEXTQUEST_MASTER_PASSWORD` env var — CLI flags are visible in
        /// process listings (ps/Task Manager) and shell history.
        #[arg(long, hide = true)]
        master_password: Option<String>,
        /// Spawn new EQ processes (default: use existing eqgame.exe processes)
        #[arg(long)]
        spawn: bool,
        /// Seconds to wait after injection before sending login (default: 3)
        #[arg(long, default_value = "3")]
        inject_delay: u64,
    },

    // ── Client commands ───────────────────────────────────────────────
    /// Send a slash command to a PID
    Cmd {
        /// Target PID
        pid: u32,
        /// Slash command (e.g., "/sit")
        command: String,
    },

    /// Right-click interact with current target (NPC, door, or object)
    Interact {
        /// Target PID
        pid: u32,
    },

    // ── Navigation ────────────────────────────────────────────────────
    /// Navigate to coordinates
    Nav {
        /// Target PID
        pid: u32,
        x: f32,
        y: f32,
        z: f32,
    },
    /// Navigate ALL clients to coordinates
    NavAll { x: f32, y: f32, z: f32 },
    /// Navigate between zones
    NavPath {
        zone: String,
        x1: f32,
        y1: f32,
        z1: f32,
        x2: f32,
        y2: f32,
        z2: f32,
    },

    // ── Rendering ─────────────────────────────────────────────────────
    /// Set render mode for a client (normal, strobe, null)
    Render {
        /// Target PID
        pid: u32,
        /// Render mode: normal, strobe, or null
        mode: String,
    },
    /// Set render mode for ALL injected clients
    #[command(name = "render-all")]
    RenderAll {
        /// Render mode: normal, strobe, or null
        mode: String,
    },

    // ── Status & diagnostics ──────────────────────────────────────────
    /// Query player status for a PID
    #[command(name = "client-status")]
    ClientStatus {
        /// Target PID
        pid: u32,
    },
    /// Summary table for all EQ clients
    #[command(name = "client-status-all")]
    ClientStatusAll,
    /// Query the zone adjacency graph from an injected client
    Zones {
        /// Target PID
        pid: u32,
    },
    /// Inspect or refresh cached navmesh data
    Navmesh {
        #[command(subcommand)]
        action: NavMeshAction,
    },
    /// Calibrate login addresses for all processes
    Calibrate,

    // ── Orchestration ─────────────────────────────────────────────────
    /// Run the orchestrator event loop (health checks, launch coordinator, camp
    /// loop)
    Orchestrate {
        /// Parse config and print what the orchestrator would do without
        /// executing any IPC writes, DLL injections, or process launches.
        #[arg(long)]
        dry_run: bool,
    },

    // ── Testing ───────────────────────────────────────────────────────
    /// Run the orchestrator in test mode for N hours with progress reporting
    #[command(name = "overnight-test")]
    OvernightTest {
        /// Duration to run in hours (default: 8)
        #[arg(long, default_value = "8", value_parser = parse_duration_hours)]
        duration: f64,
        /// Account profile name to test (omit for all accounts)
        #[arg(long)]
        accounts: Option<String>,
        /// Test all accounts regardless of profile
        #[arg(long)]
        all_accounts: bool,
        /// Comma-separated list of scenarios to run
        #[arg(long, value_delimiter = ',')]
        scenarios: Vec<String>,
        /// Directory to write progress and final JSON report
        #[arg(long, default_value = "./overnight-test-runs")]
        output_dir: std::path::PathBuf,
        /// Log level (default: info)
        #[arg(long, default_value = "info")]
        log_level: String,
    },

    // ── Report generation ─────────────────────────────────────────────
    /// Generate an HTML report from a session JSON export
    Report {
        /// Path to the session JSON export file (produced by overnight-test)
        #[arg(long, short)]
        input: std::path::PathBuf,
        /// Output path for the HTML report (default: report.html)
        #[arg(long, short, default_value = "report.html")]
        output: std::path::PathBuf,
    },

    // ── Configuration ─────────────────────────────────────────────────
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    // ── Credentials ──────────────────────────────────────────────────
    /// Manage encrypted Daybreak credentials
    Credential {
        #[command(subcommand)]
        action: CredentialAction,
    },
}

#[derive(Subcommand, Debug)]
enum NavMeshAction {
    /// Redownload and validate the zone navmesh cache
    Reload {
        /// Zone short name (for example `gfaydark`). Omit with `--pid` to use
        /// the live client's zone.
        zone: Option<String>,
        /// Resolve the zone from a live injected client
        #[arg(long)]
        pid: Option<u32>,
    },
    /// Print cache and live navigator diagnostics for a zone
    Diagnostics {
        /// Zone short name (for example `gfaydark`). Omit with `--pid` to use
        /// the live client's zone.
        zone: Option<String>,
        /// Resolve the zone from a live injected client and query its navigator
        /// state
        #[arg(long)]
        pid: Option<u32>,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Validate the TOML configuration file
    Check {
        /// Path to config file (default: config/textquest.toml)
        #[arg(long)]
        path: Option<String>,
    },
    /// Print the resolved configuration
    Show,
}

#[derive(Subcommand, Debug)]
enum CredentialAction {
    /// Add or update an account credential
    Add {
        /// Account name (e.g., "Frostreaver01")
        account: String,
        /// Account password (non-interactive mode; omit to be prompted).
        /// WARNING: CLI flags are visible in process listings — prefer
        /// interactive prompt.
        #[arg(long, hide = true)]
        password: Option<String>,
        /// Master password (non-interactive mode; omit to be prompted).
        /// WARNING: CLI flags are visible in process listings — prefer
        /// `TEXTQUEST_MASTER_PASSWORD` env var or interactive prompt.
        #[arg(long, hide = true)]
        master_password: Option<String>,
    },
    /// List all stored account names
    List,
    /// Remove an account credential
    Remove {
        /// Account name to remove
        account: String,
    },
}

#[cfg(not(windows))]
fn main() {
    eprintln!("textquest is only supported on Windows");
}

#[cfg(windows)]
fn main() -> Result<()> {
    let args = Args::parse();
    let (log_prefix, default_filter) = if args.dump {
        ("textquest-dump.log", "debug")
    } else {
        ("textquest.log", "info")
    };

    // Load log config for rotation/retention settings; fall back to defaults
    // if the config file is not yet present (first run, fresh install, etc.).
    let log_config = cli::load_config().map(|c| c.log).unwrap_or_default();

    let log_dir = paths::resolve_log_dir();
    let _tracing_guard = init_tracing(
        &log_dir,
        log_prefix,
        default_filter,
        &args.log_format,
        &log_config,
    );

    tracing::info!(
        log_prefix,
        log_dir = %log_dir.display(),
        "TextQuest orchestrator starting"
    );

    match args.command {
        // Daemon lifecycle
        Some(Commands::Start { foreground }) => cli::run_start_mode(foreground),
        Some(Commands::Stop) => cli::run_stop_mode(),
        Some(Commands::DaemonStatus) => cli::run_daemon_status_mode(),

        // Dashboard
        Some(Commands::Dashboard { port, open }) => cli::run_dashboard_mode(port, open),

        // TUI
        Some(Commands::Tui) => cli::run_tui_mode(),

        // Injection & login
        Some(Commands::Inject { pid: Some(pid) }) => cli::run_inject_pid_mode(pid),
        Some(Commands::Inject { pid: None }) => cli::run_inject_mode(),
        Some(Commands::Login {
            account,
            server,
            character,
            pid,
        }) => {
            let password = textquest::credentials::prompt::prompt_password("Password: ")
                .context("Failed to read password")?;
            let character = character.unwrap_or_default();
            if let Some(pid) = pid {
                cli::run_login_pid_mode(pid, &account, password, &server, &character)
            } else {
                cli::run_login_mode(&account, password, &server, &character)
            }
        }

        Some(Commands::Autologin {
            account,
            group,
            password,
            master_password,
            spawn,
            inject_delay,
        }) => cli::run_autologin_mode(
            account,
            group,
            password,
            master_password,
            spawn,
            inject_delay,
        ),

        // Client commands
        Some(Commands::Cmd { pid, command }) => cli::run_cmd_mode(pid, &command),
        Some(Commands::Interact { pid }) => cli::run_interact_mode(pid),

        // Rendering
        Some(Commands::Render { pid, mode }) => cli::run_render_mode(pid, &mode),
        Some(Commands::RenderAll { mode }) => cli::run_renderall_mode(&mode),

        // Navigation
        Some(Commands::Nav { pid, x, y, z }) => cli::run_nav_mode(pid, x, y, z),
        Some(Commands::NavAll { x, y, z }) => cli::run_navall_mode(x, y, z),
        Some(Commands::NavPath {
            zone,
            x1,
            y1,
            z1,
            x2,
            y2,
            z2,
        }) => cli::run_navpath_mode(&zone, (x1, y1, z1), (x2, y2, z2)),

        // Status & diagnostics
        Some(Commands::ClientStatus { pid }) => cli::run_status_mode(pid),
        Some(Commands::ClientStatusAll) => cli::run_statusall_mode(),
        Some(Commands::Zones { pid }) => cli::run_zones_mode(pid),
        Some(Commands::Navmesh { action }) => match action {
            NavMeshAction::Reload { zone, pid } => {
                cli::run_navmesh_reload_mode(zone.as_deref(), pid)
            }
            NavMeshAction::Diagnostics { zone, pid } => {
                cli::run_navmesh_diagnostics_mode(zone.as_deref(), pid)
            }
        },
        Some(Commands::Calibrate) => cli::run_calibrate_mode(),

        // Orchestration
        Some(Commands::Orchestrate { dry_run }) => cli::run_orchestrate_mode(dry_run),

        // Testing
        Some(Commands::OvernightTest {
            duration,
            accounts,
            all_accounts,
            scenarios,
            output_dir,
            log_level,
        }) => cli::run_overnight_test_mode(
            duration,
            accounts.as_deref(),
            all_accounts,
            &scenarios,
            &output_dir,
            &log_level,
        ),

        // Report generation
        Some(Commands::Report { input, output }) => cli::run_report_mode(&input, &output),

        // Configuration
        Some(Commands::Config { action }) => match action {
            ConfigAction::Check { path } => cli::run_config_check_mode(path.as_deref()),
            ConfigAction::Show => cli::run_config_show_mode(),
        },

        // Credentials
        Some(Commands::Credential { action }) => match action {
            CredentialAction::Add {
                account,
                password: acct_pw,
                master_password: master_pw,
            } => {
                let master = match master_pw {
                    Some(pw) => zeroize::Zeroizing::new(pw),
                    None => textquest::credentials::prompt::prompt_password("Master password: ")
                        .context("Failed to read master password")?,
                };
                match acct_pw {
                    Some(pw) => {
                        let store = cli::open_credential_store(&master)?;
                        store.add_account(&account, &pw)?;
                        eprintln!("Account '{account}' added/updated.");
                        Ok(())
                    }
                    None => cli::run_credential_add_mode(&account, master),
                }
            }
            CredentialAction::List => {
                let password = textquest::credentials::prompt::prompt_password("Master password: ")
                    .context("Failed to read master password")?;
                cli::run_credential_list_mode(password)
            }
            CredentialAction::Remove { account } => {
                let password = textquest::credentials::prompt::prompt_password("Master password: ")
                    .context("Failed to read master password")?;
                cli::run_credential_remove_mode(&account, password)
            }
        },

        None => {
            // Check top-level flags for backward compatibility
            if args.dump {
                cli::run_dump_mode()
            } else if args.inject {
                cli::run_inject_mode()
            } else {
                cli::run_tui_mode()
            }
        }
    }
}

#[cfg(windows)]
fn init_tracing(
    log_dir: &Path,
    filename_prefix: &str,
    default_filter: &str,
    log_format: &str,
    log_config: &LogConfig,
) -> tracing_appender::non_blocking::WorkerGuard {
    if let Err(err) = std::fs::create_dir_all(log_dir) {
        eprintln!(
            "warning: unable to create log directory '{}': {err}",
            log_dir.display()
        );
    }

    let mut builder = rolling::RollingFileAppender::builder()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix(filename_prefix);

    if log_config.max_files > 0 {
        builder = builder.max_log_files(log_config.max_files);
    }

    let file_appender = builder
        .build(log_dir)
        .unwrap_or_else(|_| rolling::daily(log_dir, filename_prefix));

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    if log_format == "json" {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().json().with_writer(non_blocking))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
            .init();
    }

    guard
}

#[cfg(test)]
mod tests {
    use super::{Args, Commands, NavMeshAction};
    use clap::Parser;
    use std::path::PathBuf;

    #[test]
    fn parses_navmesh_reload_with_zone() {
        let args = Args::parse_from(["textquest", "navmesh", "reload", "gfaydark"]);
        match args.command {
            Some(Commands::Navmesh {
                action: NavMeshAction::Reload { zone, pid },
            }) => {
                assert_eq!(zone.as_deref(), Some("gfaydark"));
                assert_eq!(pid, None);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_navmesh_diagnostics_with_pid() {
        let args = Args::parse_from(["textquest", "navmesh", "diagnostics", "--pid", "12345"]);
        match args.command {
            Some(Commands::Navmesh {
                action: NavMeshAction::Diagnostics { zone, pid },
            }) => {
                assert_eq!(zone, None);
                assert_eq!(pid, Some(12_345));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_overnight_test_defaults() {
        let args = Args::parse_from(["textquest", "overnight-test"]);
        match args.command {
            Some(Commands::OvernightTest {
                duration,
                accounts,
                all_accounts,
                scenarios,
                output_dir,
                log_level,
            }) => {
                assert!((duration - 8.0_f64).abs() < f64::EPSILON);
                assert!(accounts.is_none());
                assert!(!all_accounts);
                assert!(scenarios.is_empty());
                assert_eq!(output_dir, PathBuf::from("./overnight-test-runs"));
                assert_eq!(log_level, "info");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_overnight_test_custom_flags() {
        let args = Args::parse_from([
            "textquest",
            "overnight-test",
            "--duration",
            "4",
            "--accounts",
            "frostreaver",
            "--all-accounts",
            "--scenarios",
            "combat,nav",
            "--output-dir",
            "/tmp/runs",
            "--log-level",
            "debug",
        ]);
        match args.command {
            Some(Commands::OvernightTest {
                duration,
                accounts,
                all_accounts,
                scenarios,
                output_dir,
                log_level,
            }) => {
                assert!((duration - 4.0_f64).abs() < f64::EPSILON);
                assert_eq!(accounts.as_deref(), Some("frostreaver"));
                assert!(all_accounts);
                assert_eq!(scenarios, vec!["combat", "nav"]);
                assert_eq!(output_dir, PathBuf::from("/tmp/runs"));
                assert_eq!(log_level, "debug");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn overnight_test_invalid_duration_rejected() {
        let result =
            Args::try_parse_from(["textquest", "overnight-test", "--duration", "notanumber"]);
        assert!(result.is_err());
    }

    #[test]
    fn overnight_test_negative_duration_rejected() {
        let result = Args::try_parse_from(["textquest", "overnight-test", "--duration", "-1"]);
        assert!(result.is_err());
    }

    #[test]
    fn overnight_test_nan_duration_rejected() {
        let result = Args::try_parse_from(["textquest", "overnight-test", "--duration", "NaN"]);
        assert!(result.is_err());
    }

    #[test]
    fn overnight_test_infinite_duration_rejected() {
        let result = Args::try_parse_from(["textquest", "overnight-test", "--duration", "inf"]);
        assert!(result.is_err());
    }

    #[test]
    fn log_format_defaults_to_text() {
        let args = Args::parse_from(["textquest"]);
        assert_eq!(args.log_format, "text");
    }

    #[test]
    fn log_format_accepts_json() {
        let args = Args::parse_from(["textquest", "--log-format", "json"]);
        assert_eq!(args.log_format, "json");
    }

    #[test]
    fn log_format_accepts_text() {
        let args = Args::parse_from(["textquest", "--log-format", "text"]);
        assert_eq!(args.log_format, "text");
    }

    #[test]
    fn log_format_rejects_invalid_value() {
        let result = Args::try_parse_from(["textquest", "--log-format", "yaml"]);
        assert!(result.is_err());
    }
}
