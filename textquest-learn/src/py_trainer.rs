use anyhow::Result;
use std::path::Path;
use std::process::Command;
use tracing::info;

pub async fn train(
    algo: &str,
    warm_start: &Path,
    ledger: &Path,
    reward: &str,
    class: &str,
    epochs: usize,
    out: &Path,
) -> Result<()> {
    info!(
        algo = algo,
        warm_start = ?warm_start,
        ledger = ?ledger,
        reward = reward,
        class = class,
        epochs = epochs,
        out = ?out,
        "Starting RL training"
    );

    // Spawn Python sidecar trainer
    let python_script = find_python_trainer()?;

    let output = Command::new("python3")
        .arg(&python_script)
        .arg("--algo")
        .arg(algo)
        .arg("--warm-start")
        .arg(warm_start)
        .arg("--ledger")
        .arg(ledger)
        .arg("--reward")
        .arg(reward)
        .arg("--class")
        .arg(class)
        .arg("--epochs")
        .arg(epochs.to_string())
        .arg("--out")
        .arg(out)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "Python training failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    info!("Training completed successfully");
    Ok(())
}

fn find_python_trainer() -> Result<String> {
    // Look for trainer script in standard locations
    let candidates = [
        "textquest-learn/python/trainer.py",
        "../textquest-learn/python/trainer.py",
        "./trainer.py",
    ];

    for candidate in &candidates {
        if std::path::Path::new(candidate).exists() {
            return Ok(candidate.to_string());
        }
    }

    anyhow::bail!("Could not find Python trainer script")
}
