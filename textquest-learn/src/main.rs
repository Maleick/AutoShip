use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod evaluator;
mod policy;
mod py_trainer;

#[derive(Parser)]
#[command(name = "textquest-learn")]
#[command(about = "Offline RL training harness for TextQuest policies")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Train offline RL policy (CQL/IQL)
    Rl {
        #[command(subcommand)]
        subcommand: RlSubcommand,
    },
}

#[derive(Subcommand)]
enum RlSubcommand {
    /// Train a policy using CQL or IQL
    Train {
        /// Algorithm: cql or iql
        #[arg(long)]
        algo: String,

        /// Warm-start BC policy path (ONNX)
        #[arg(long)]
        warm_start: PathBuf,

        /// Ledger data directory
        #[arg(long)]
        ledger: PathBuf,

        /// Reward specification key (e.g., combat.heal.cleric.v1)
        #[arg(long)]
        reward: String,

        /// Character class (cleric, rogue, etc.)
        #[arg(long)]
        class: String,

        /// Number of training epochs
        #[arg(long, default_value = "200")]
        epochs: usize,

        /// Output ONNX path
        #[arg(long)]
        out: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Rl { subcommand } => match subcommand {
            RlSubcommand::Train {
                algo,
                warm_start,
                ledger,
                reward,
                class,
                epochs,
                out,
            } => {
                py_trainer::train(&algo, &warm_start, &ledger, &reward, &class, epochs, &out)
                    .await?;
            }
        },
    }

    Ok(())
}
