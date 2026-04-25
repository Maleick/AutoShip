//! `textquest-learn` — training CLI for the L-4 contextual-bandit layer.
//!
//! # Usage
//!
//! ```
//! textquest-learn bandits train \
//!     --ledger decisions.jsonl \
//!     --reward reward \
//!     --scope cleric.heal_picker \
//!     --out model.bin \
//!     [--algorithm linucb|thompson] \
//!     [--alpha 1.0] \
//!     [--arms CH,Heal,Cure] \
//!     [--context-dim 12]
//! ```

use std::{
    fs,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;

use textquest_learn::{
    bandit::{
        context::ContextSpec,
        linucb::LinUcbModel,
        model::{AlgorithmTag, BanditScope, ModelFile, MODEL_FILE_VERSION},
        thompson::ThompsonModel,
    },
};

// ─── CLI types ───────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "textquest-learn", about = "TextQuest L-4 bandit training tools")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Bandits(BanditsArgs),
}

#[derive(Parser)]
struct BanditsArgs {
    #[command(subcommand)]
    subcommand: BanditsCommand,
}

#[derive(Subcommand)]
enum BanditsCommand {
    Train(TrainArgs),
}

#[derive(ValueEnum, Clone, Debug)]
enum AlgorithmArg {
    Linucb,
    Thompson,
}

#[derive(Parser, Debug)]
struct TrainArgs {
    /// Path to L-1 decision ledger (JSONL, one record per line).
    #[arg(long)]
    ledger: PathBuf,

    /// JSON field name in each ledger record to use as the reward signal.
    #[arg(long, default_value = "reward")]
    reward: String,

    /// Scope: "<class>.<decision_point>" (e.g. "cleric.heal_picker").
    #[arg(long)]
    scope: String,

    /// Output path for the trained model file.
    #[arg(long, short)]
    out: PathBuf,

    /// Bandit algorithm.
    #[arg(long, value_enum, default_value = "linucb")]
    algorithm: AlgorithmArg,

    /// Exploration parameter (alpha for LinUCB, v for Thompson).
    #[arg(long, default_value_t = 1.0)]
    alpha: f32,

    /// Comma-separated list of arm labels (e.g. "CH,Heal,Cure").
    #[arg(long, value_delimiter = ',')]
    arms: Vec<String>,

    /// Context vector dimension (must match spec used during data collection).
    #[arg(long, default_value_t = 12)]
    context_dim: u32,

    /// Optional path to a TOML context-spec file for validation.
    #[arg(long)]
    context_spec: Option<PathBuf>,
}

// ─── Ledger record schema ────────────────────────────────────────────────────

/// One record from the L-1 decision ledger JSONL file.
#[derive(Debug, Deserialize)]
struct LedgerRecord {
    /// Scope key (must match --scope).
    #[serde(default)]
    scope: Option<String>,
    /// Which arm was taken (0-based index).
    arm_taken: usize,
    /// Context vector as a flat array of f32.
    context_vec: Vec<f32>,
    /// Reward signal (field name matched by --reward flag).
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

// ─── Main ────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Bandits(b) => match b.subcommand {
            BanditsCommand::Train(args) => run_train(args),
        },
    }
}

fn run_train(args: TrainArgs) -> Result<()> {
    // Destructure to avoid partial-move issues
    let TrainArgs {
        ledger,
        reward,
        scope: scope_str,
        out,
        algorithm,
        alpha,
        arms: arm_names,
        context_dim,
        context_spec: context_spec_path,
    } = args;

    // Parse scope
    let (class, decision_point) = parse_scope(&scope_str)?;
    let scope = BanditScope::new(class, decision_point);
    let scope_key = scope.as_key();

    // Resolve arm labels
    let arm_label_strings: Vec<String> = if arm_names.is_empty() {
        bail!("--arms is required: provide comma-separated arm labels");
    } else {
        arm_names
    };
    let arm_labels: Vec<&str> = arm_label_strings.iter().map(String::as_str).collect();
    let d = context_dim as usize;

    // Build context spec
    let ctx_spec = if let Some(spec_path) = context_spec_path {
        let text = fs::read_to_string(&spec_path)
            .with_context(|| format!("reading context spec: {}", spec_path.display()))?;
        toml::from_str::<ContextSpec>(&text)
            .with_context(|| "parsing context spec TOML")?
    } else {
        // Synthetic spec with generic field names
        ContextSpec {
            dim: context_dim,
            field_names: (0..d).map(|i| format!("dim_{i}")).collect(),
        }
    };
    let spec_hash = ctx_spec.hash_fingerprint();

    eprintln!(
        "Training {scope_key} with {} arms, d={d}, spec_hash={spec_hash:#x}",
        arm_labels.len()
    );

    // Train
    let (arms, algo_tag) = match algorithm {
        AlgorithmArg::Linucb => {
            let mut model = LinUcbModel::new(&arm_labels, d, alpha);
            let (processed, skipped) = train_linucb(&mut model, &ledger, &reward, &scope_key, d)?;
            eprintln!("LinUCB: {processed} records processed, {skipped} skipped");
            let arms = model.arms.iter().map(|a| a.to_serialized()).collect();
            (arms, AlgorithmTag::LinUcb)
        }
        AlgorithmArg::Thompson => {
            let mut model = ThompsonModel::new(&arm_labels, d, alpha);
            let (processed, skipped) = train_thompson(&mut model, &ledger, &reward, &scope_key, d)?;
            eprintln!("Thompson: {processed} records processed, {skipped} skipped");
            for arm in &mut model.arms {
                arm.finalize();
            }
            let arms = model.arms.iter().map(|a| a.to_serialized()).collect();
            (arms, AlgorithmTag::Thompson)
        }
    };

    let model_file = ModelFile {
        version: MODEL_FILE_VERSION,
        context_spec_hash: spec_hash,
        scope,
        algorithm: algo_tag,
        exploration_param: alpha,
        context_dim,
        arms,
    };

    let bytes = model_file
        .to_bytes()
        .context("serializing model to bincode")?;
    fs::write(&out, &bytes)
        .with_context(|| format!("writing model to {}", out.display()))?;

    eprintln!("Wrote {} bytes → {}", bytes.len(), out.display());
    Ok(())
}

// ─── Training loops ──────────────────────────────────────────────────────────

fn train_linucb(
    model: &mut LinUcbModel,
    ledger: &std::path::Path,
    reward_field: &str,
    scope_key: &str,
    d: usize,
) -> Result<(usize, usize)> {
    let file = fs::File::open(ledger)
        .with_context(|| format!("opening ledger {}", ledger.display()))?;
    let mut processed = 0usize;
    let mut skipped = 0usize;

    for (line_no, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match serde_json::from_str::<LedgerRecord>(line) {
            Ok(rec) => {
                if let Some(ref s) = rec.scope {
                    if s != scope_key {
                        continue;
                    }
                }
                let reward = extract_reward(&rec, reward_field);
                if let Some((ctx, r)) = validate_record(&rec, reward, d, line_no) {
                    if rec.arm_taken < model.arms.len() {
                        model.arms[rec.arm_taken].update(&ctx, r);
                        processed += 1;
                    } else {
                        skipped += 1;
                    }
                } else {
                    skipped += 1;
                }
            }
            Err(e) => {
                eprintln!("line {line_no}: parse error: {e}");
                skipped += 1;
            }
        }
    }
    Ok((processed, skipped))
}

fn train_thompson(
    model: &mut ThompsonModel,
    ledger: &std::path::Path,
    reward_field: &str,
    scope_key: &str,
    d: usize,
) -> Result<(usize, usize)> {
    let file = fs::File::open(ledger)
        .with_context(|| format!("opening ledger {}", ledger.display()))?;
    let mut processed = 0usize;
    let mut skipped = 0usize;

    for (line_no, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match serde_json::from_str::<LedgerRecord>(line) {
            Ok(rec) => {
                if let Some(ref s) = rec.scope {
                    if s != scope_key {
                        continue;
                    }
                }
                let reward = extract_reward(&rec, reward_field);
                if let Some((ctx, r)) = validate_record(&rec, reward, d, line_no) {
                    if rec.arm_taken < model.arms.len() {
                        model.arms[rec.arm_taken].update(&ctx, r);
                        processed += 1;
                    } else {
                        skipped += 1;
                    }
                } else {
                    skipped += 1;
                }
            }
            Err(e) => {
                eprintln!("line {line_no}: parse error: {e}");
                skipped += 1;
            }
        }
    }
    Ok((processed, skipped))
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn parse_scope(s: &str) -> Result<(&str, &str)> {
    let (a, b) = s
        .split_once('.')
        .with_context(|| format!("scope must be 'class.decision_point', got: {s}"))?;
    Ok((a, b))
}

fn extract_reward(rec: &LedgerRecord, field: &str) -> Option<f32> {
    rec.extra
        .get(field)
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)
}

fn validate_record(
    rec: &LedgerRecord,
    reward: Option<f32>,
    d: usize,
    line_no: usize,
) -> Option<([f32; 32], f32)> {
    let r = reward?;
    if rec.context_vec.len() < d {
        eprintln!(
            "line {line_no}: context_vec len {} < required {d}",
            rec.context_vec.len()
        );
        return None;
    }
    let mut ctx = [0.0f32; 32];
    ctx[..rec.context_vec.len().min(32)].copy_from_slice(&rec.context_vec[..rec.context_vec.len().min(32)]);
    Some((ctx, r))
}
