//! Plugin discovery and loading system — MQ2-compatible plugin infrastructure.
//!
//! Scans a configurable directory for `.dll` files, attempts to load each one
//! via [`libloading`], resolves required entry points (`PLUGIN_INIT`,
//! `PLUGIN_SHUTDOWN`) and an optional version symbol (`PLUGIN_VERSION`),
//! validates compatibility, and tracks loaded plugins in a [`PluginRegistry`].
//!
//! Load failures are logged but never panic — the orchestrator keeps running
//! with whatever plugins loaded successfully.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::{debug, error, info, warn};

// ─── ABI function signatures ─────────────────────────────────────────────────

/// Signature for `PLUGIN_INIT` — called once after the plugin is loaded.
///
/// # Safety
/// The callee must be well-behaved; this is an FFI boundary.
#[cfg(target_os = "windows")]
type PluginInitFn = unsafe extern "C" fn() -> i32;

/// Signature for `PLUGIN_SHUTDOWN` — called before unloading the plugin.
///
/// # Safety
/// The callee must be well-behaved; this is an FFI boundary.
#[cfg(target_os = "windows")]
type PluginShutdownFn = unsafe extern "C" fn();

/// Signature for the optional `PLUGIN_VERSION` symbol (null-terminated C string).
#[cfg(target_os = "windows")]
type PluginVersionPtr = *const std::os::raw::c_char;

/// Minimum plugin API version accepted by this loader.
pub const MIN_PLUGIN_VERSION: u32 = 1;

// ─── PluginMetadata ───────────────────────────────────────────────────────────

/// Static metadata extracted from a loaded plugin.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    /// Friendly name derived from the DLL filename (without extension).
    pub name: String,
    /// Absolute path to the DLL on disk.
    pub path: PathBuf,
    /// Version string exported by the plugin, if present.
    pub version: Option<String>,
}

// ─── PluginHandle ─────────────────────────────────────────────────────────────

/// A successfully loaded plugin. Holds the `libloading::Library` so the DLL
/// stays mapped for the process lifetime (until the handle is dropped).
pub struct PluginHandle {
    /// Extracted metadata.
    pub metadata: PluginMetadata,
    /// The underlying loaded library. Kept alive so symbols remain valid.
    #[cfg(target_os = "windows")]
    #[allow(dead_code)]
    library: libloading::Library,
}

impl std::fmt::Debug for PluginHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginHandle")
            .field("metadata", &self.metadata)
            .finish_non_exhaustive()
    }
}

// ─── PluginRegistry ──────────────────────────────────────────────────────────

/// Tracks all successfully loaded plugins, keyed by plugin name.
#[derive(Debug, Default)]
pub struct PluginRegistry {
    plugins: HashMap<String, PluginHandle>,
    /// Directory that was scanned most recently.
    plugins_dir: Option<PathBuf>,
}

impl PluginRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Scan `dir` for `*.dll` files, attempt to load each one, and register
    /// those that pass validation.
    ///
    /// Returns the number of plugins successfully loaded.
    ///
    /// # Errors
    ///
    /// Returns an error only if `dir` cannot be read at all. Individual plugin
    /// load failures are logged and skipped.
    pub fn discover_and_load(&mut self, dir: &Path) -> Result<usize> {
        info!(directory = %dir.display(), "Starting plugin discovery");
        self.plugins_dir = Some(dir.to_path_buf());

        let entries = std::fs::read_dir(dir)
            .with_context(|| format!("Cannot read plugins directory: {}", dir.display()))?;

        let mut loaded = 0usize;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    warn!(%err, "Skipping unreadable directory entry");
                    continue;
                }
            };

            let path = entry.path();

            // Only consider .dll files.
            let is_dll = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("dll"))
                .unwrap_or(false);

            if !is_dll {
                debug!(path = %path.display(), "Skipping non-DLL file");
                continue;
            }

            let plugin_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            info!(plugin = %plugin_name, path = %path.display(), "Attempting to load plugin");

            match self.load_plugin(&path, plugin_name.clone()) {
                Ok(handle) => {
                    info!(plugin = %plugin_name, "Plugin loaded successfully");
                    self.plugins.insert(plugin_name, handle);
                    loaded += 1;
                }
                Err(err) => {
                    error!(plugin = %plugin_name, %err, "Failed to load plugin — skipping");
                }
            }
        }

        info!(loaded, "Plugin discovery complete");
        Ok(loaded)
    }

    /// Load a single plugin from `path`.
    ///
    /// On Windows: opens the DLL, resolves `PLUGIN_INIT` and `PLUGIN_SHUTDOWN`,
    /// reads the optional `PLUGIN_VERSION`, calls `PLUGIN_INIT`, and returns a
    /// handle.
    ///
    /// On non-Windows: always returns an error (DLL loading is Windows-only).
    ///
    /// # Errors
    ///
    /// Returns an error if the DLL cannot be opened, required symbols are
    /// missing, or `PLUGIN_INIT` returns a non-zero status.
    fn load_plugin(&self, path: &Path, name: String) -> Result<PluginHandle> {
        #[cfg(target_os = "windows")]
        {
            self.load_plugin_windows(path, name)
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Non-Windows: DLL loading not supported; log and bail gracefully.
            warn!(
                plugin = %name,
                path = %path.display(),
                "DLL loading is only supported on Windows — skipping"
            );
            anyhow::bail!("DLL loading not supported on this platform")
        }
    }

    /// Windows-only DLL loading implementation.
    #[cfg(target_os = "windows")]
    fn load_plugin_windows(&self, path: &Path, name: String) -> Result<PluginHandle> {
        // SAFETY: We own the path and it's a valid filesystem path.
        let lib = unsafe { libloading::Library::new(path) }
            .with_context(|| format!("Failed to open DLL: {}", path.display()))?;

        // Resolve required entry points.
        // SAFETY: We trust that the symbol name is correct and the function
        // signature matches. Any mismatch is an author error in the plugin.
        let _init_fn: libloading::Symbol<PluginInitFn> = unsafe {
            lib.get(b"PLUGIN_INIT\0")
                .context("Missing required symbol: PLUGIN_INIT")?
        };

        let _shutdown_fn: libloading::Symbol<PluginShutdownFn> = unsafe {
            lib.get(b"PLUGIN_SHUTDOWN\0")
                .context("Missing required symbol: PLUGIN_SHUTDOWN")?
        };

        // Read optional version string.
        let version = self.read_plugin_version(&lib);

        // Call PLUGIN_INIT and check the return code.
        // SAFETY: symbol lifetime is bounded by `lib` which we still hold.
        let init_fn: libloading::Symbol<PluginInitFn> = unsafe {
            lib.get(b"PLUGIN_INIT\0").expect("already validated above")
        };

        let rc = unsafe { init_fn() };
        if rc != 0 {
            anyhow::bail!("PLUGIN_INIT returned non-zero status: {rc}");
        }

        let metadata = PluginMetadata {
            name,
            path: path.to_path_buf(),
            version,
        };

        Ok(PluginHandle { metadata, library: lib })
    }

    /// Read the `PLUGIN_VERSION` export as a UTF-8 string if present.
    #[cfg(target_os = "windows")]
    fn read_plugin_version(&self, lib: &libloading::Library) -> Option<String> {
        // SAFETY: The symbol, if present, must point to a valid null-terminated
        // C string. We copy it immediately and drop the reference.
        let sym: Result<libloading::Symbol<PluginVersionPtr>, _> =
            unsafe { lib.get(b"PLUGIN_VERSION\0") };

        match sym {
            Err(_) => {
                debug!("PLUGIN_VERSION not exported; skipping version check");
                None
            }
            Ok(ptr_sym) => {
                let raw = *ptr_sym;
                if raw.is_null() {
                    warn!("PLUGIN_VERSION exported but pointer is null");
                    return None;
                }
                // SAFETY: we checked for null; trusting the plugin to have a
                // valid C string at this address.
                let cstr = unsafe { std::ffi::CStr::from_ptr(raw) };
                match cstr.to_str() {
                    Ok(s) => {
                        debug!(version = %s, "Plugin version found");
                        Some(s.to_string())
                    }
                    Err(err) => {
                        warn!(%err, "PLUGIN_VERSION is not valid UTF-8");
                        None
                    }
                }
            }
        }
    }

    /// Return a reference to the handle for a plugin by name, if loaded.
    pub fn get(&self, name: &str) -> Option<&PluginHandle> {
        self.plugins.get(name)
    }

    /// Iterate over all loaded plugins.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &PluginHandle)> {
        self.plugins.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Number of loaded plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Returns `true` if no plugins are loaded.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// The plugins directory last passed to [`discover_and_load`], if any.
    pub fn plugins_dir(&self) -> Option<&Path> {
        self.plugins_dir.as_deref()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_temp_dir() -> TempDir {
        tempfile::tempdir().expect("tempdir creation should not fail")
    }

    #[test]
    fn empty_directory_yields_zero_plugins() {
        let dir = make_temp_dir();
        let mut registry = PluginRegistry::new();
        let count = registry
            .discover_and_load(dir.path())
            .expect("discover_and_load on empty dir should not error");
        assert_eq!(count, 0);
        assert!(registry.is_empty());
    }

    #[test]
    fn non_dll_files_are_ignored() {
        let dir = make_temp_dir();
        // Create a non-DLL file.
        std::fs::write(dir.path().join("not_a_plugin.txt"), b"hello").unwrap();
        let mut registry = PluginRegistry::new();
        let count = registry
            .discover_and_load(dir.path())
            .expect("discover_and_load should not error for non-DLL files");
        assert_eq!(count, 0);
        assert!(registry.is_empty());
    }

    /// On non-Windows: fake .dll files should fail gracefully (no panic).
    /// On Windows: fake .dll files should fail with a load error (not a valid PE).
    #[test]
    fn fake_dll_file_fails_gracefully() {
        let dir = make_temp_dir();
        // Write a file with .dll extension but invalid content.
        std::fs::write(dir.path().join("fake_plugin.dll"), b"this is not a DLL").unwrap();
        let mut registry = PluginRegistry::new();
        // discover_and_load itself should succeed (no panic) even though the
        // individual plugin load fails.
        let count = registry
            .discover_and_load(dir.path())
            .expect("discover_and_load should not panic on bad DLL");
        assert_eq!(count, 0, "Fake DLL should not be counted as loaded");
        assert!(registry.is_empty());
    }

    #[test]
    fn registry_len_and_is_empty_are_consistent() {
        let registry = PluginRegistry::new();
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
    }

    #[test]
    fn plugins_dir_is_recorded() {
        let dir = make_temp_dir();
        let mut registry = PluginRegistry::new();
        registry.discover_and_load(dir.path()).unwrap();
        assert_eq!(registry.plugins_dir(), Some(dir.path()));
    }

    #[test]
    fn missing_directory_returns_error() {
        let mut registry = PluginRegistry::new();
        let result = registry.discover_and_load(Path::new("/nonexistent/path/to/plugins"));
        assert!(result.is_err(), "Missing directory should return Err");
    }

    /// Windows-only: verify that a real DLL can be loaded if it exports the
    /// required symbols. This test is skipped on non-Windows.
    #[cfg(target_os = "windows")]
    #[test]
    fn windows_valid_dll_loads_successfully() {
        // This test requires a real plugin DLL to be present. In CI, the
        // `textquest-dll` crate is built and placed in a known location.
        // Skip if the DLL is not present.
        let dll_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/debug/textquest_dll.dll");
        if !dll_path.exists() {
            return;
        }
        let dir = make_temp_dir();
        let dest = dir.path().join("textquest_dll.dll");
        std::fs::copy(&dll_path, &dest).unwrap();
        let mut registry = PluginRegistry::new();
        let count = registry.discover_and_load(dir.path()).unwrap();
        assert_eq!(count, 1);
        assert!(!registry.is_empty());
    }
}
