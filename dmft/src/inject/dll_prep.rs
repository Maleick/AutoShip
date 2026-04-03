use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

const MAX_PREP_ATTEMPTS: u32 = 24;
const DLL_EXTENSION: &str = "dll";

/// Copy the compiled DLL to a temp directory with a randomized name
/// that looks like a plausible system component.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn prepare_dll(source_dll: &Path) -> Result<PathBuf> {
    validate_source_dll(source_dll)?;

    let target_dir = prepare_payload_dir()?;
    std::fs::create_dir_all(&target_dir)?;

    for _ in 0..MAX_PREP_ATTEMPTS {
        let random_name = generate_random_dll_name();
        let target_path = target_dir.join(&random_name);

        match stage_dll_copy(source_dll, &target_path) {
            Ok(()) => {
                tracing::info!(name = %random_name, "Prepared DLL payload");
                return Ok(target_path);
            }
            Err(err) => {
                let should_retry = err
                    .root_cause()
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|io_err| io_err.kind() == std::io::ErrorKind::AlreadyExists);

                if should_retry {
                    continue;
                }
                return Err(err);
            }
        }
    }

    Err(anyhow!(
        "Failed to prepare DLL after {MAX_PREP_ATTEMPTS} randomized path attempts"
    ))
}

/// Clean up a previously prepared DLL.
pub fn cleanup_dll(path: &Path) {
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
}

fn validate_source_dll(path: &Path) -> Result<()> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case(DLL_EXTENSION) {
        anyhow::bail!("Source DLL must have a .dll extension: {}", path.display());
    }

    let meta = std::fs::symlink_metadata(path)
        .with_context(|| format!("Failed to inspect source DLL metadata: {}", path.display()))?;
    if meta.is_dir() || meta.file_type().is_symlink() {
        anyhow::bail!("Source DLL is not a regular file: {}", path.display());
    }
    Ok(())
}

fn prepare_payload_dir() -> Result<PathBuf> {
    let target_dir = std::env::temp_dir().join("dmft_payloads");
    if let Ok(meta) = std::fs::symlink_metadata(&target_dir) {
        if meta.file_type().is_symlink() {
            anyhow::bail!("Payload directory is a symlink: {}", target_dir.display());
        }
        if !meta.is_dir() {
            anyhow::bail!(
                "Payload directory is not a directory: {}",
                target_dir.display()
            );
        }
    }
    Ok(target_dir)
}

/// Write a DLL payload file to a unique staging path.
///
/// Uses `create_new` so path collisions are rejected instead of truncating.
fn stage_dll_copy(source: &Path, target: &Path) -> Result<()> {
    let mut source_file = std::fs::File::open(source)
        .with_context(|| format!("Failed to open source DLL {}", source.display()))?;
    let mut target_file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(target)
        .with_context(|| format!("Failed to open staging path {}", target.display()))?;
    std::io::copy(&mut source_file, &mut target_file)
        .with_context(|| format!("Failed to copy DLL to {}", target.display()))?;
    target_file.flush().ok();
    Ok(())
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
