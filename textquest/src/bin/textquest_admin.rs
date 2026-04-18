//! TextQuest Admin CLI — operator tooling for session diagnostics and management.

#![allow(dead_code)]

mod admin_client;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "textquest-admin")]
#[command(about = "TextQuest administrative CLI for multibox session management")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, default_value = "http://127.0.0.1:3001")]
    api_url: String,
}

#[derive(Subcommand)]
enum Commands {
    Diagnose {
        session_id: u32,
    },
    Logs {
        #[command(subcommand)]
        action: LogAction,
    },
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
}

#[derive(Subcommand)]
enum LogAction {
    Tail {
        session_id: u32,
        #[arg(default_value = "50")]
        lines: u32,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    Audit { session_id: u32 },
}

#[derive(Subcommand)]
enum SessionAction {
    List,
    Start { profile: u32 },
    Stop { session_id: u32 },
    Restart { session_id: u32 },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let client = admin_client::AdminClient::new(&cli.api_url);

    match cli.command {
        Commands::Diagnose { session_id } => match client.get_diagnostics(session_id) {
            Ok(diag) => {
                println!("=== Diagnostics for Session {} ===", session_id);
                println!("Memory: {} MB", diag.memory_mb);
                println!("CPU %: {}", diag.cpu_percent);
                println!("IPC Latency (ms):");
                println!("  p50: {}", diag.ipc_latency_p50);
                println!("  p95: {}", diag.ipc_latency_p95);
                println!("  p99: {}", diag.ipc_latency_p99);
                println!("Status: {}", diag.status);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                ExitCode::FAILURE
            }
        },
        Commands::Logs { action } => match action {
            LogAction::Tail { session_id, lines } => match client.get_logs(session_id, lines) {
                Ok(logs) => {
                    if logs.is_empty() {
                        println!("No logs available for session {}", session_id);
                    } else {
                        for line in logs {
                            println!("{}", line);
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    ExitCode::FAILURE
                }
            },
        },
        Commands::Config { action } => match action {
            ConfigAction::Audit { session_id } => match client.get_config_audit(session_id) {
                Ok(audit) => {
                    println!("=== Configuration Audit for Session {} ===", session_id);
                    println!("Character: {}", audit.character_name);
                    println!("Class: {}", audit.class_name);
                    println!("Group: {}", audit.group_id);
                    println!("\nConfiguration Status:");
                    for item in audit.items {
                        let status = if item.consistent { "✓" } else { "✗" };
                        println!("  {} {}: {}", status, item.name, item.detail);
                    }
                    if audit.issues.is_empty() {
                        println!("\nNo configuration issues detected.");
                    } else {
                        println!("\nIssues:");
                        for issue in audit.issues {
                            println!("  - {}", issue);
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    ExitCode::FAILURE
                }
            },
        },
        Commands::Session { action } => match action {
            SessionAction::List => match client.list_sessions() {
                Ok(sessions) => {
                    if sessions.is_empty() {
                        println!("No active sessions.");
                    } else {
                        println!("=== Managed Sessions ===");
                        for session in sessions {
                            println!(
                                "Session {}: {} ({}) - {} [{}]",
                                session.session_id,
                                session
                                    .character_name
                                    .unwrap_or_else(|| "Unknown".to_string()),
                                session.class_name.unwrap_or_else(|| "Unknown".to_string()),
                                session.routing_scope.label,
                                format!("{:?}", session.lifecycle_state).to_lowercase()
                            );
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    ExitCode::FAILURE
                }
            },
            SessionAction::Start { profile } => match client.start_session(profile) {
                Ok(resp) => {
                    println!("{}", resp.message);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    ExitCode::FAILURE
                }
            },
            SessionAction::Stop { session_id } => match client.stop_session(session_id) {
                Ok(resp) => {
                    println!("{}", resp.message);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    ExitCode::FAILURE
                }
            },
            SessionAction::Restart { session_id } => match client.restart_session(session_id) {
                Ok(resp) => {
                    println!("{}", resp.message);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    ExitCode::FAILURE
                }
            },
        },
    }
}
