use clap::{Parser, Subcommand};
use std::path::PathBuf;
use textquest_learn::{
    bookmarks::BookmarkStore, ledger::ExperienceLedger, quality_gates::QualityGates,
    onnx_export::OnnxExporter, FlaggedSegmentDataset, PolicyMetadata,
    training::BehaviorCloningTrainer,
};
use tracing::{info, error};

#[derive(Parser)]
#[command(name = "textquest-learn")]
#[command(about = "TextQuest behavior cloning training harness", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Behavior cloning training pipeline")]
    Bc {
        #[command(subcommand)]
        subcommand: BcCommand,
    },
}

#[derive(Subcommand)]
enum BcCommand {
    #[command(about = "Train a behavior cloning policy from flagged segments")]
    Train {
        #[arg(long, help = "Path to experience ledger directory")]
        ledger: PathBuf,

        #[arg(long, help = "Path to bookmarks SQLite database")]
        flags: PathBuf,

        #[arg(long, help = "Character class (e.g., cleric, warrior)")]
        class: String,

        #[arg(long, help = "Context schema version")]
        context_schema: String,

        #[arg(long, help = "Training epochs", default_value = "40")]
        epochs: u32,

        #[arg(long, help = "Hidden units per layer", default_value = "32")]
        hidden_units: usize,

        #[arg(long, help = "Learning rate", default_value = "0.01")]
        learning_rate: f32,

        #[arg(long, help = "Output path for trained model")]
        out: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Bc { subcommand } => match subcommand {
            BcCommand::Train {
                ledger,
                flags,
                class,
                context_schema,
                epochs,
                hidden_units,
                learning_rate,
                out,
            } => {
                train_behavior_cloning(
                    ledger,
                    flags,
                    class,
                    context_schema,
                    epochs,
                    hidden_units,
                    learning_rate,
                    out,
                )
                .await?;
            }
        },
    }

    Ok(())
}

async fn train_behavior_cloning(
    ledger_path: PathBuf,
    flags_path: PathBuf,
    class: String,
    context_schema: String,
    epochs: u32,
    hidden_units: usize,
    learning_rate: f32,
    output_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Loading experience ledger from {:?}", ledger_path);
    let ledger = ExperienceLedger::load_from_dir(&ledger_path)?;
    info!("Loaded {} experience entries", ledger.all_entries().len());

    info!("Loading bookmarks from {:?}", flags_path);
    let bookmarks = BookmarkStore::load_from_sqlite(&flags_path)?;
    info!("Loaded {} bookmarks", bookmarks.get_bookmarks().len());

    info!("Building flagged segment dataset for class '{}'", class);
    let dataset = FlaggedSegmentDataset::build(&ledger, &bookmarks, &class, &context_schema)?;
    info!(
        "Built dataset with {} {}-dimensional context-action pairs",
        dataset.pairs.len(),
        dataset.context_dim()
    );

    let (train_pairs, heldout_pairs) = dataset.split_train_heldout(0.2);
    info!(
        "Split into {} training and {} held-out samples",
        train_pairs.len(),
        heldout_pairs.len()
    );

    info!("Training behavior cloning model ({} epochs, {} hidden units)", epochs, hidden_units);
    let trainer = BehaviorCloningTrainer::new(learning_rate, epochs, hidden_units);
    let model = trainer.train(&dataset)?;
    info!("Model trained successfully");

    info!("Evaluating held-out action match rate");
    let heldout_metrics = QualityGates::evaluate_heldout(&model, &heldout_pairs)?;
    info!(
        "Held-out action match rate: {:.2}% ({}/{})",
        heldout_metrics.action_match_rate * 100.0,
        heldout_metrics.correct_predictions,
        heldout_metrics.total_samples
    );

    QualityGates::check_action_match(&heldout_metrics)?;

    info!("Checking coverage of context clusters");
    let coverage = QualityGates::check_coverage(&train_pairs, &heldout_pairs)?;
    info!(
        "Coverage: {} well-covered clusters, {} underfitted",
        coverage.well_covered_clusters,
        coverage.underfitted_clusters.len()
    );

    if !coverage.coverage_ok {
        error!(
            "Coverage check failed. Underfitted clusters: {:?}",
            coverage.underfitted_clusters
        );
        return Err("Coverage quality gate failed".into());
    }

    let git_sha = std::process::Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string())
        .trim()
        .to_string();

    let metadata = PolicyMetadata {
        class: class.clone(),
        training_data_manifest: bookmarks
            .get_bookmarks()
            .iter()
            .map(|b| (b.session_id.clone(), (b.start_flag_idx, b.end_flag_idx)))
            .collect(),
        context_schema_version: context_schema,
        exporter_git_sha: git_sha,
        held_out_action_match_rate: heldout_metrics.action_match_rate,
        total_training_samples: train_pairs.len(),
        total_heldout_samples: heldout_pairs.len(),
        underfitted_context_clusters: coverage.underfitted_clusters,
    };

    info!("Exporting model to {:?}", output_path);
    OnnxExporter::export(&model, &metadata, &output_path)?;

    info!("Training complete. Exported policy artifact: {:?}", output_path);
    Ok(())
}
