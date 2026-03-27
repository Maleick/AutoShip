use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Copy the compiled DLL to a temp directory with a randomized name
/// that looks like a plausible system component.
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

/// Generate a random name that blends in with system DLLs.
fn generate_random_dll_name() -> String {
    let prefixes = [
        "msvc", "dx", "d3d", "win", "sys", "rt", "api", "cfg", "net", "sec",
    ];
    let suffixes = [
        "rt", "cfg", "hlp", "svc", "ext", "lib", "mod", "core", "base", "util",
    ];

    let r1: u32 = rand::random::<u32>();
    let r2: u32 = rand::random::<u32>();
    let prefix = prefixes[(r1 as usize) % prefixes.len()];
    let suffix = suffixes[(r2 as usize) % suffixes.len()];
    let num: u32 = rand::random::<u32>() % 10000;

    format!("{}_{}{}.dll", prefix, suffix, num)
}
