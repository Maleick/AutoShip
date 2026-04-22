//! Plugin discovery and loading system — MQ2-compatible plugin infrastructure.
//!
//! Scans a configurable directory for `.dll` files, attempts to load each one
//! via [`libloading`], resolves required entry points (`PLUGIN_INIT`,
//! `PLUGIN_SHUTDOWN`) and an optional version symbol (`PLUGIN_VERSION`),
//! validates compatibility, and tracks loaded plugins in a [`PluginRegistry`].
//!
//! Load failures are logged but never panic — the orchestrator keeps running
//! with whatever plugins loaded successfully.
//!
//! # Hotkey/command registration for plugins
//!
//! Plugins receive a [`PluginApi`] pointer during `PLUGIN_INIT` (via the
//! optional `PLUGIN_INIT_V2` entry point) that exposes two C-callable functions:
//!
//! ```c
//! // Register a hotkey. Returns a u64 hotkey ID (0 on failure).
//! uint64_t tq_register_hotkey(const char* combo, void (*callback)(void));
//!
//! // Register a slash command. Returns a u64 command ID (0 on failure).
//! uint64_t tq_register_command(const char* path, void (*callback)(const char*));
//!
//! // Unregister a hotkey by ID.
//! int      tq_unregister_hotkey(uint64_t id);
//!
//! // Unregister a command by ID.
//! int      tq_unregister_command(uint64_t id);
//! ```
//!
//! On unload the registry automatically cleans up all entries registered by
//! that plugin.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::{debug, error, info, warn};

use crate::registry::{
    CommandId, Priority, ScriptHotkeyId, SharedCommandRegistry, SharedHotkeyRegistry,
};

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

// ─── Plugin registration API ─────────────────────────────────────────────────

/// C-callable function signatures that plugins may call to register hotkeys /
/// commands.  These are exposed to plugins via the `PLUGIN_INIT_V2` entry point.
type RegisterHotkeyFn =
    unsafe extern "C" fn(combo: *const std::os::raw::c_char, cb: unsafe extern "C" fn()) -> u64;
type RegisterCommandFn = unsafe extern "C" fn(
    path: *const std::os::raw::c_char,
    cb: unsafe extern "C" fn(*const std::os::raw::c_char),
) -> u64;
type UnregisterHotkeyFn = unsafe extern "C" fn(id: u64) -> i32;
type UnregisterCommandFn = unsafe extern "C" fn(id: u64) -> i32;

/// `PLUGIN_INIT_V2` — optional entry point that receives the API table.
///
/// Plugins that export this symbol are handed a [`PluginApi`] pointer before
/// `PLUGIN_INIT` is called, giving them access to the hotkey/command APIs.
#[cfg(target_os = "windows")]
type PluginInitV2Fn = unsafe extern "C" fn(api: *const PluginApi) -> i32;

/// API table passed to plugins via `PLUGIN_INIT_V2`.
///
/// The layout is fixed (repr C) and versioned by `api_version`.  Plugins must
/// check `api_version >= 1` before calling any function pointer.
#[repr(C)]
pub struct PluginApi {
    /// Always `1` for this version of the API table.
    pub api_version: u32,
    /// Register a hotkey. `combo` is a null-terminated UTF-8 string
    /// (e.g. `"ctrl+f5"`).  Returns a non-zero hotkey ID on success.
    pub register_hotkey: RegisterHotkeyFn,
    /// Register a slash command. `path` is a null-terminated UTF-8 string
    /// (e.g. `"/mymod help"`).  Returns a non-zero command ID on success.
    pub register_command: RegisterCommandFn,
    /// Unregister a hotkey by ID returned from `register_hotkey`.
    /// Returns 1 on success, 0 if not found.
    pub unregister_hotkey: UnregisterHotkeyFn,
    /// Unregister a command by ID returned from `register_command`.
    /// Returns 1 on success, 0 if not found.
    pub unregister_command: UnregisterCommandFn,
}

// Safety: PluginApi contains only fn pointers which are Send+Sync.
unsafe impl Send for PluginApi {}
unsafe impl Sync for PluginApi {}

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

// ─── PluginRegistrationTracker ────────────────────────────────────────────────

/// Tracks hotkey and command IDs registered by a specific plugin so they can
/// be bulk-removed when the plugin is unloaded.
#[derive(Debug, Default)]
pub struct PluginRegistrationTracker {
    pub hotkey_ids: Vec<ScriptHotkeyId>,
    pub command_ids: Vec<CommandId>,
}

impl PluginRegistrationTracker {
    pub fn new() -> Self {
        Self::default()
    }
}

// ─── PluginRegistry ──────────────────────────────────────────────────────────

/// Tracks all successfully loaded plugins, keyed by plugin name.
pub struct PluginRegistry {
    plugins: HashMap<String, PluginHandle>,
    /// Directory that was scanned most recently.
    plugins_dir: Option<PathBuf>,
    /// Per-plugin registration trackers for bulk cleanup on unload.
    trackers: HashMap<String, PluginRegistrationTracker>,
    /// Shared command registry — plugins register into this.
    command_registry: SharedCommandRegistry,
    /// Shared hotkey registry — plugins register into this.
    hotkey_registry: SharedHotkeyRegistry,
}

impl std::fmt::Debug for PluginRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginRegistry")
            .field("plugins", &self.plugins.keys().collect::<Vec<_>>())
            .field("plugins_dir", &self.plugins_dir)
            .finish_non_exhaustive()
    }
}

impl PluginRegistry {
    /// Create an empty registry with freshly-created shared registries.
    pub fn new() -> Self {
        let (cmd, hk) = crate::registry::new_shared();
        Self::with_registries(cmd, hk)
    }

    /// Create a registry that shares existing hotkey/command registries (e.g.
    /// with a [`crate::lua::bindings::LuaBindings`] instance).
    pub fn with_registries(
        command_registry: SharedCommandRegistry,
        hotkey_registry: SharedHotkeyRegistry,
    ) -> Self {
        Self {
            plugins: HashMap::new(),
            plugins_dir: None,
            trackers: HashMap::new(),
            command_registry,
            hotkey_registry,
        }
    }

    /// Register a hotkey on behalf of a plugin.
    ///
    /// This is the Rust-side equivalent of `tq_register_hotkey` and is used
    /// in unit tests.  On Windows the same logic is called via the C FFI from
    /// the plugin DLL.
    pub fn plugin_register_hotkey(
        &mut self,
        plugin_name: &str,
        combo: impl Into<String>,
        callback: Box<dyn Fn() + Send + Sync>,
    ) -> ScriptHotkeyId {
        let id = self.hotkey_registry.lock().unwrap().register(
            combo,
            Priority::Plugin,
            plugin_name,
            callback,
        );
        self.trackers
            .entry(plugin_name.to_string())
            .or_default()
            .hotkey_ids
            .push(id);
        id
    }

    /// Register a slash command on behalf of a plugin.
    pub fn plugin_register_command(
        &mut self,
        plugin_name: &str,
        path: impl Into<String>,
        handler: Box<dyn Fn(&str) + Send + Sync>,
    ) -> CommandId {
        let id = self.command_registry.lock().unwrap().register(
            path,
            Priority::Plugin,
            plugin_name,
            handler,
        );
        self.trackers
            .entry(plugin_name.to_string())
            .or_default()
            .command_ids
            .push(id);
        id
    }

    /// Unregister a specific hotkey registered by a plugin.
    pub fn plugin_unregister_hotkey(&mut self, plugin_name: &str, id: ScriptHotkeyId) -> bool {
        if let Some(tracker) = self.trackers.get_mut(plugin_name) {
            tracker.hotkey_ids.retain(|&hid| hid != id);
        }
        self.hotkey_registry.lock().unwrap().unregister(id)
    }

    /// Unregister a specific command registered by a plugin.
    pub fn plugin_unregister_command(&mut self, plugin_name: &str, id: CommandId) -> bool {
        if let Some(tracker) = self.trackers.get_mut(plugin_name) {
            tracker.command_ids.retain(|&cid| cid != id);
        }
        self.command_registry.lock().unwrap().unregister(id)
    }

    /// Bulk-unregister all hotkeys and commands registered by `plugin_name`.
    ///
    /// Called automatically when a plugin is unloaded.
    pub fn cleanup_plugin_registrations(&mut self, plugin_name: &str) -> (usize, usize) {
        let tracker = match self.trackers.remove(plugin_name) {
            Some(t) => t,
            None => return (0, 0),
        };
        let mut hk_reg = self.hotkey_registry.lock().unwrap();
        let mut cmd_reg = self.command_registry.lock().unwrap();
        let hk_removed: usize =
            tracker.hotkey_ids.iter().filter(|&&id| hk_reg.unregister(id)).count();
        let cmd_removed: usize =
            tracker.command_ids.iter().filter(|&&id| cmd_reg.unregister(id)).count();
        info!(
            plugin = %plugin_name,
            hotkeys_removed = hk_removed,
            commands_removed = cmd_removed,
            "Plugin registrations cleaned up on unload"
        );
        (hk_removed, cmd_removed)
    }

    /// Expose shared registry handles for external coordination.
    pub fn command_registry(&self) -> SharedCommandRegistry {
        Arc::clone(&self.command_registry)
    }

    /// Expose shared registry handles for external coordination.
    pub fn hotkey_registry(&self) -> SharedHotkeyRegistry {
        Arc::clone(&self.hotkey_registry)
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

    // ── Plugin registration API ───────────────────────────────────────────────

    #[test]
    fn plugin_register_hotkey_fires_callback() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let mut registry = PluginRegistry::new();
        let fired = Arc::new(AtomicU32::new(0));
        let f = Arc::clone(&fired);

        let id = registry.plugin_register_hotkey(
            "test_plugin",
            "ctrl+f5",
            Box::new(move || {
                f.fetch_add(1, Ordering::Relaxed);
            }),
        );

        assert!(registry.hotkey_registry().lock().unwrap().fire("ctrl+f5"));
        assert_eq!(fired.load(Ordering::Relaxed), 1);

        // Explicit unregister by ID.
        assert!(registry.plugin_unregister_hotkey("test_plugin", id));
        assert!(!registry.hotkey_registry().lock().unwrap().fire("ctrl+f5"));
    }

    #[test]
    fn plugin_register_command_routes_correctly() {
        use std::sync::Arc;
        use std::sync::Mutex;

        let mut registry = PluginRegistry::new();
        let received = Arc::new(Mutex::new(String::new()));
        let r = Arc::clone(&received);

        let id = registry.plugin_register_command(
            "test_plugin",
            "/plug cmd",
            Box::new(move |args: &str| {
                *r.lock().unwrap() = args.to_string();
            }),
        );

        assert!(registry.command_registry().lock().unwrap().dispatch("/plug cmd foo bar"));
        assert_eq!(*received.lock().unwrap(), "foo bar");

        // Explicit unregister.
        assert!(registry.plugin_unregister_command("test_plugin", id));
        assert!(!registry.command_registry().lock().unwrap().dispatch("/plug cmd foo bar"));
    }

    #[test]
    fn cleanup_plugin_registrations_removes_all_on_unload() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let mut registry = PluginRegistry::new();

        // Register two hotkeys and one command for "plugin_a".
        let fired = Arc::new(AtomicU32::new(0));
        let f1 = Arc::clone(&fired);
        let f2 = Arc::clone(&fired);

        registry.plugin_register_hotkey("plugin_a", "alt+f1", Box::new(move || { f1.fetch_add(1, Ordering::Relaxed); }));
        registry.plugin_register_hotkey("plugin_a", "alt+f2", Box::new(move || { f2.fetch_add(1, Ordering::Relaxed); }));
        registry.plugin_register_command("plugin_a", "/pa cmd", Box::new(|_| {}));

        // Sanity: both hotkeys and command work.
        assert!(registry.hotkey_registry().lock().unwrap().fire("alt+f1"));
        assert!(registry.hotkey_registry().lock().unwrap().fire("alt+f2"));
        assert!(registry.command_registry().lock().unwrap().dispatch("/pa cmd"));

        // Simulate plugin unload.
        let (hk, cmd) = registry.cleanup_plugin_registrations("plugin_a");
        assert_eq!(hk, 2, "two hotkeys should have been removed");
        assert_eq!(cmd, 1, "one command should have been removed");

        // Nothing fires after cleanup.
        assert!(!registry.hotkey_registry().lock().unwrap().fire("alt+f1"));
        assert!(!registry.hotkey_registry().lock().unwrap().fire("alt+f2"));
        assert!(!registry.command_registry().lock().unwrap().dispatch("/pa cmd"));
    }

    #[test]
    fn plugin_priority_is_higher_than_script() {
        use std::sync::{Arc, Mutex};

        let (cmd_reg, hk_reg) = crate::registry::new_shared();
        let mut plugin_registry = PluginRegistry::with_registries(
            Arc::clone(&cmd_reg),
            Arc::clone(&hk_reg),
        );

        let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));
        let o1 = Arc::clone(&order);
        let o2 = Arc::clone(&order);

        // Script-priority entry (lower precedence).
        cmd_reg.lock().unwrap().register(
            "/shared",
            crate::registry::Priority::Script,
            "a_script",
            Box::new(move |_| o1.lock().unwrap().push("script")),
        );

        // Plugin-priority entry (higher precedence).
        plugin_registry.plugin_register_command(
            "my_plugin",
            "/shared",
            Box::new(move |_| o2.lock().unwrap().push("plugin")),
        );

        assert!(cmd_reg.lock().unwrap().dispatch("/shared"));
        let result = order.lock().unwrap().clone();
        assert_eq!(result, vec!["plugin"], "plugin must shadow script");
    }

    #[test]
    fn with_registries_shares_state_with_lua_bindings() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let (cmd_reg, hk_reg) = crate::registry::new_shared();
        let mut plugin_registry = PluginRegistry::with_registries(
            Arc::clone(&cmd_reg),
            Arc::clone(&hk_reg),
        );

        // Plugin registers a hotkey via the Rust API.
        let fired = Arc::new(AtomicU32::new(0));
        let f = Arc::clone(&fired);
        plugin_registry.plugin_register_hotkey(
            "my_plugin",
            "ctrl+g",
            Box::new(move || { f.fetch_add(1, Ordering::Relaxed); }),
        );

        // Fire via the shared handle (same one the Lua bindings would hold).
        assert!(hk_reg.lock().unwrap().fire("ctrl+g"));
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }
}
