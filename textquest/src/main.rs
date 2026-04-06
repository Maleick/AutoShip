use textquest::cli;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing_appender::rolling;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "DMFT — EverQuest multibox controller",
    long_about = "DMFT is an EverQuest multibox controller with TUI dashboard, DLL injection,\n\
                   navigation, combat automation, and web dashboard support.\n\n\
                   Run without arguments to launch the TUI dashboard."
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
}

#[derive(Subcommand)]
enum Commands {
    // ── Daemon lifecycle ──────────────────────────────────────────────
    /// Start the DMFT daemon (TUI + background services)
    Start {
        /// Run in foreground instead of daemonizing
        #[arg(long)]
        foreground: bool,
    },
    /// Stop a running DMFT daemon gracefully
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
        /// EQ password (same for all accounts). Also reads DMFT_PASSWORD env var.
        #[arg(long)]
        password: Option<String>,
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

    /// Right-click interact with current target (open bank/merchant/quest window)
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
    /// Calibrate login addresses for all processes
    Calibrate,

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

#[derive(Subcommand)]
enum ConfigAction {
    /// Validate the TOML configuration file
    Check {
        /// Path to config file (default: config/frostreaver.toml)
        #[arg(long)]
        path: Option<String>,
    },
    /// Print the resolved configuration
    Show,
}

#[derive(Subcommand)]
enum CredentialAction {
    /// Add or update an account credential
    Add {
        /// Account name (e.g., "Frostreaver01")
        account: String,
        /// Account password (non-interactive mode; omit to be prompted)
        #[arg(long)]
        password: Option<String>,
        /// Master password (non-interactive mode; omit to be prompted)
        #[arg(long)]
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

fn main() -> Result<()> {
    // Set up file logging — must be done before anything else.
    let log_dir = std::env::current_dir().unwrap_or_default().join("logs");
    std::fs::create_dir_all(&log_dir).ok();
    let file_appender = rolling::RollingFileAppender::builder()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix("textquest.log")
        .max_log_files(7) // Keep 1 week of logs
        .build(&log_dir)
        .unwrap_or_else(|_| rolling::daily(&log_dir, "textquest.log"));
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();

    tracing::info!("DMFT orchestrator starting");

    let args = Args::parse();

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
            spawn,
            inject_delay,
        }) => cli::run_autologin_mode(account, group, password, spawn, inject_delay),

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
        Some(Commands::Calibrate) => cli::run_calibrate_mode(),

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
