use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Copy the compiled DLL to a temp directory with a randomized name
/// that looks like a plausible system component.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn prepare_dll(source_dll: &Path) -> Result<PathBuf> {
    let target_dir = std::env::temp_dir().join("dmft_payloads");
    std::fs::create_dir_all(&target_dir)?;

    let random_name = generate_random_dll_name();
    let target_path = target_dir.join(&random_name);

    std::fs::copy(source_dll, &target_path)
        .with_context(|| format!("Failed to copy DLL to {}", target_path.display()))?;

    tracing::info!(name = %random_name, "Prepared DLL payload");
    Ok(target_path)
}

/// Clean up a previously prepared DLL.
pub fn cleanup_dll(path: &Path) {
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
}

/// Generate a random hex DLL name with no guessable pattern.
///
/// Uses `OsRng` to produce 16 random bytes formatted as a 32-char hex string,
/// e.g. `a3f2c891b4d7e05f1234567890abcdef.dll`. This removes the predictable
/// prefix/suffix/number pattern that a forensic scan could match against.
fn generate_random_dll_name() -> String {
    use rand::RngCore;
    let mut rng = rand::rngs::OsRng;
    let hi = rng.next_u64();
    let lo = rng.next_u64();
    format!("{hi:016x}{lo:016x}.dll")
}
