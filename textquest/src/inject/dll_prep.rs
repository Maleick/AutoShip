use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};

const MAX_PREP_ATTEMPTS: u32 = 24;
const DLL_EXTENSION: &str = "dll";
#[cfg(windows)]
const FILE_SHARE_READ: u32 = 0x0000_0001;

/// A prepared on-disk DLL plus a live file-handle guard that prevents
/// replacement while injection is in-flight.
pub struct StagedDll {
    path: PathBuf,
    _guard: std::fs::File,
}

impl StagedDll {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Legitimate Microsoft DLL names used for stealth staging.
///
/// These are real DLLs found alongside .NET, CLR, WPF, diagnostics, and other
/// Microsoft runtime components. A staged DLL named after one of these blends
/// into normal process module lists.
const LEGITIMATE_DLL_NAMES: &[&str] = &[
    // .NET / CLR runtime
    "mscorlib.ni.dll",
    "clrjit.dll",
    "mscoree.dll",
    "clr.dll",
    "mscoreei.dll",
    "mscorwks.dll",
    "mscordbi.dll",
    "mscordacwks.dll",
    "clretwrc.dll",
    "nlssorting.dll",
    // Diagnostics / debugging
    "dbghelp.dll",
    "dbgcore.dll",
    "diagtrack.dll",
    "dxdiagn.dll",
    "perftrack.dll",
    "wer.dll",
    "faultrep.dll",
    // WPF / presentation
    "wpfgfx_cor3.dll",
    "presentationfontcache.dll",
    "presentationnative_cor3.dll",
    "milcore.dll",
    "dwmapi.dll",
    "dcomp.dll",
    // Crypto / security
    "bcrypt.dll",
    "ncryptsslp.dll",
    "rsaenh.dll",
    "schannel.dll",
    "mssign32.dll",
    // Networking
    "httpapi.dll",
    "winhttp.dll",
    "webio.dll",
    "dhcpcsvc.dll",
    "dnsapi.dll",
    "winnsi.dll",
    // DirectX / graphics
    "d3d11.dll",
    "dxgi.dll",
    "d3dcompiler_47.dll",
    "d2d1.dll",
    "dwrite.dll",
    // System utilities
    "devobj.dll",
    "cfgmgr32.dll",
    "dpapi.dll",
    "wintrust.dll",
    "mspatcha.dll",
    "cabinet.dll",
    "msi.dll",
    "sxs.dll",
    "apphelp.dll",
    "uxtheme.dll",
];

/// A pool that hands out unique legitimate DLL names for staging.
///
/// Ensures no two clients in the same session receive the same name,
/// supporting up to [`LEGITIMATE_DLL_NAMES`].len() concurrent clients.
pub struct StagingNamePool {
    used: Mutex<Vec<usize>>,
}

impl StagingNamePool {
    /// Create a new empty pool.
    pub fn new() -> Self {
        Self {
            used: Mutex::new(Vec::new()),
        }
    }

    /// Return the total number of names available in the pool.
    #[must_use]
    pub fn capacity(&self) -> usize {
        LEGITIMATE_DLL_NAMES.len()
    }

    /// Return the number of names already claimed.
    #[must_use]
    pub fn used_count(&self) -> usize {
        self.used.lock().expect("pool lock poisoned").len()
    }

    /// Pick a random unused name from the pool.
    ///
    /// Returns `Err` if all names have been claimed.
    pub fn next_name(&self) -> Result<&'static str> {
        use rand::seq::SliceRandom;
        let mut used = self.used.lock().expect("pool lock poisoned");

        if used.len() >= LEGITIMATE_DLL_NAMES.len() {
            anyhow::bail!(
                "All {} legitimate DLL names exhausted",
                LEGITIMATE_DLL_NAMES.len()
            );
        }

        let mut rng = rand::thread_rng();
        let available: Vec<usize> = (0..LEGITIMATE_DLL_NAMES.len())
            .filter(|i| !used.contains(i))
            .collect();

        let &idx = available
            .as_slice()
            .choose(&mut rng)
            .expect("available is non-empty");

        used.push(idx);
        Ok(LEGITIMATE_DLL_NAMES[idx])
    }

    /// Release a previously claimed name back to the pool.
    pub fn release(&self, name: &str) {
        let mut used = self.used.lock().expect("pool lock poisoned");
        if let Some(idx) = LEGITIMATE_DLL_NAMES.iter().position(|&n| n == name) {
            used.retain(|&i| i != idx);
        }
    }

    /// Stage a DLL using the next available legitimate name.
    ///
    /// Combines name allocation with the standard staging pipeline.
    pub fn prepare_dll(&self, source_dll: &Path) -> Result<PathBuf> {
        validate_source_dll(source_dll)?;

        let target_dir = prepare_payload_dir()?;
        std::fs::create_dir_all(&target_dir)?;

        let name = self.next_name()?;
        let target_path = target_dir.join(name);

        stage_dll_copy(source_dll, &target_path).inspect_err(|_err| {
            // Return the name on failure so the caller can retry.
            self.release(name);
        })?;

        tracing::info!(name, "Prepared DLL payload with legitimate name");
        Ok(target_path)
    }
}

impl Default for StagingNamePool {
    fn default() -> Self {
        Self::new()
    }
}

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

/// Prepare a DLL and keep a restrictive file handle open so other local
/// processes cannot swap or rewrite the staged payload before `LoadLibraryW`.
pub fn prepare_dll_locked(source_dll: &Path) -> Result<StagedDll> {
    validate_source_dll(source_dll)?;

    let target_dir = prepare_payload_dir()?;
    std::fs::create_dir_all(&target_dir)?;

    for _ in 0..MAX_PREP_ATTEMPTS {
        let random_name = generate_random_dll_name();
        let target_path = target_dir.join(&random_name);

        match stage_dll_copy_locked(source_dll, &target_path) {
            Ok(guard) => {
                tracing::info!(name = %random_name, "Prepared locked DLL payload");
                return Ok(StagedDll {
                    path: target_path,
                    _guard: guard,
                });
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
    let target_dir = std::env::temp_dir().join("textquest_payloads");
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
    let mut target_file = stage_target_file(target)?;
    std::io::copy(&mut source_file, &mut target_file)
        .with_context(|| format!("Failed to copy DLL to {}", target.display()))?;
    target_file.flush().ok();
    Ok(())
}

fn stage_dll_copy_locked(source: &Path, target: &Path) -> Result<std::fs::File> {
    let mut source_file = std::fs::File::open(source)
        .with_context(|| format!("Failed to open source DLL {}", source.display()))?;
    let mut target_file = stage_target_file(target)?;
    std::io::copy(&mut source_file, &mut target_file)
        .with_context(|| format!("Failed to copy DLL to {}", target.display()))?;
    target_file.flush().ok();
    Ok(target_file)
}

fn stage_target_file(target: &Path) -> Result<std::fs::File> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(target)
            .with_context(|| format!("Failed to open staging path {}", target.display()))
    }
    #[cfg(not(windows))]
    {
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .read(true)
            .open(target)
            .with_context(|| format!("Failed to open staging path {}", target.display()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn pool_has_enough_names_for_36_clients() {
        assert!(
            LEGITIMATE_DLL_NAMES.len() >= 40,
            "Pool must have at least 40 names, got {}",
            LEGITIMATE_DLL_NAMES.len()
        );
    }

    #[test]
    fn all_names_end_with_dll() {
        for name in LEGITIMATE_DLL_NAMES {
            assert!(name.ends_with(".dll"), "Name {name} does not end with .dll");
        }
    }

    #[test]
    fn no_duplicate_names_in_pool() {
        let unique: HashSet<&str> = LEGITIMATE_DLL_NAMES.iter().copied().collect();
        assert_eq!(
            unique.len(),
            LEGITIMATE_DLL_NAMES.len(),
            "Duplicate names in LEGITIMATE_DLL_NAMES"
        );
    }

    #[test]
    fn pool_hands_out_unique_names() {
        let pool = StagingNamePool::new();
        let mut seen = HashSet::new();
        for _ in 0..36 {
            let name = pool.next_name().expect("should not exhaust for 36 clients");
            assert!(seen.insert(name), "Duplicate name handed out: {name}");
        }
    }

    #[test]
    fn pool_exhaustion_returns_error() {
        let pool = StagingNamePool::new();
        for _ in 0..LEGITIMATE_DLL_NAMES.len() {
            pool.next_name().expect("should succeed");
        }
        assert!(pool.next_name().is_err(), "Should error when exhausted");
    }

    #[test]
    fn pool_release_allows_reuse() {
        let pool = StagingNamePool::new();
        let name = pool.next_name().expect("first name");
        pool.release(name);
        assert_eq!(pool.used_count(), 0);
    }

    #[test]
    fn pool_capacity_matches_const() {
        let pool = StagingNamePool::new();
        assert_eq!(pool.capacity(), LEGITIMATE_DLL_NAMES.len());
    }

    #[test]
    fn pool_default_is_empty() {
        let pool = StagingNamePool::default();
        assert_eq!(pool.used_count(), 0);
    }

    #[test]
    fn pool_used_count_tracks_claims() {
        let pool = StagingNamePool::new();
        assert_eq!(pool.used_count(), 0);
        let _n1 = pool.next_name().unwrap();
        assert_eq!(pool.used_count(), 1);
        let _n2 = pool.next_name().unwrap();
        assert_eq!(pool.used_count(), 2);
    }

    #[test]
    fn prepare_dll_rejects_non_dll_extension() {
        let pool = StagingNamePool::new();
        let bad = Path::new("foo.exe");
        assert!(pool.prepare_dll(bad).is_err());
    }
}
