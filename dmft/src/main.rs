use dmft::cli;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing_appender::rolling;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
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
    /// Launch the TUI dashboard (default)
    Tui,
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
    /// Send a slash command to a PID
    Cmd {
        /// Target PID
        pid: u32,
        /// Slash command (e.g., "/sit")
        command: String,
    },
    /// Navigate to coordinates
    Nav {
        /// Target PID
        pid: u32,
        x: f32,
        y: f32,
        z: f32,
    },
    /// Navigate ALL clients to coordinates
    NavAll {
        x: f32,
        y: f32,
        z: f32,
    },
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
    /// Query player status for a PID
    Status {
        /// Target PID
        pid: u32,
    },
    /// Summary table for all EQ clients
    StatusAll,
    /// Query the zone adjacency graph from an injected client
    Zones {
        /// Target PID
        pid: u32,
    },
    /// Calibrate login addresses for all processes
    Calibrate,
}

fn main() -> Result<()> {
    // Set up file logging — must be done before anything else.
    let log_dir = std::env::current_dir().unwrap_or_default().join("logs");
    std::fs::create_dir_all(&log_dir).ok();
    let file_appender = rolling::RollingFileAppender::builder()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix("dmft.log")
        .max_log_files(7) // Keep 1 week of logs
        .build(&log_dir)
        .unwrap_or_else(|_| rolling::daily(&log_dir, "dmft.log"));
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
        Some(Commands::Tui) => cli::run_tui_mode(),
        Some(Commands::Inject { pid: Some(pid) }) => cli::run_inject_pid_mode(pid),
        Some(Commands::Inject { pid: None }) => cli::run_inject_mode(),
        Some(Commands::Login {
            account,
            server,
            character,
            pid,
        }) => {
            let password =
                rpassword::prompt_password("Password: ").context("Failed to read password")?;
            let character = character.unwrap_or_default();
            if let Some(pid) = pid {
                cli::run_login_pid_mode(pid, &account, &password, &server, &character)
            } else {
                cli::run_login_mode(&account, &password, &server, &character)
            }
        }
        Some(Commands::Cmd { pid, command }) => cli::run_cmd_mode(pid, &command),
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
        Some(Commands::Status { pid }) => cli::run_status_mode(pid),
        Some(Commands::StatusAll) => cli::run_statusall_mode(),
        Some(Commands::Zones { pid }) => cli::run_zones_mode(pid),
        Some(Commands::Calibrate) => cli::run_calibrate_mode(),
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
