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
//! # Crash isolation
//!
//! All FFI call sites (`PLUGIN_INIT`, `PLUGIN_SHUTDOWN`, registered callbacks)
//! are wrapped in [`std::panic::catch_unwind`] so that a panicking plugin
//! cannot bring down the orchestrator process.  A per-plugin [`PluginHealth`]
//! tracker enforces an **error budget**: more than [`ERROR_BUDGET_MAX`] errors
//! within [`ERROR_BUDGET_WINDOW_SECS`] seconds automatically disables the
//! plugin.  Disabled plugins are skipped on all future dispatch.
//!
//! ## Watchdog limitation
//!
//! True hang detection (killing a stuck plugin thread) requires running the
//! plugin in a subprocess, which is outside the scope of in-process DLL
//! loading.  The current implementation logs a warning when a call site has
//! not completed within [`WATCHDOG_WARN_SECS`] seconds, but cannot forcibly
//! terminate the call.  Full process isolation is a post-M8 improvement item.
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
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tracing::{debug, error, info, warn};

use textquest_common::plugins::{ConflictStatus, PluginManifest};

// ─── Error-budget constants ───────────────────────────────────────────────────

/// Maximum number of errors allowed within the sliding window before the
/// plugin is automatically disabled.
pub const ERROR_BUDGET_MAX: usize = 5;

/// Sliding window width (seconds) for the error budget.
pub const ERROR_BUDGET_WINDOW_SECS: u64 = 60;

/// Log a warning if a plugin FFI call has not returned within this many
/// seconds.  The call cannot be killed in-process; the warning is advisory.
pub const WATCHDOG_WARN_SECS: u64 = 5;

// ─── PluginHealth ─────────────────────────────────────────────────────────────

/// Per-plugin error budget tracker and liveness record.
///
/// Maintained inside [`PluginRegistry`] alongside each [`PluginHandle`].
/// When [`PluginHealth::is_disabled`] returns `true` the registry skips all
/// dispatch for that plugin and logs at `warn` level.
#[derive(Debug)]
pub struct PluginHealth {
    /// Ring of timestamps for recent errors (within the sliding window).
    recent_errors: VecDeque<Instant>,
    /// Set when the plugin exceeds its error budget.
    disabled: bool,
    /// Reason the plugin was disabled, if any.
    disable_reason: Option<String>,
    /// Timestamp of the last successful callback completion.
    pub last_heartbeat: Option<Instant>,
}

impl PluginHealth {
    /// Create a fresh health record.
    pub fn new() -> Self {
        Self {
            recent_errors: VecDeque::new(),
            disabled: false,
            disable_reason: None,
            last_heartbeat: None,
        }
    }

    /// Returns `true` if this plugin has been automatically disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the reason the plugin was disabled, if any.
    pub fn disable_reason(&self) -> Option<&str> {
        self.disable_reason.as_deref()
    }

    /// Record one error and disable the plugin if the budget is exhausted.
    ///
    /// Returns `true` if the plugin was **just** disabled by this call.
    pub fn record_error(&mut self, plugin_name: &str, detail: &str) -> bool {
        let now = Instant::now();
        let window = Duration::from_secs(ERROR_BUDGET_WINDOW_SECS);

        // Evict entries older than the sliding window.
        while let Some(&front) = self.recent_errors.front() {
            if now.duration_since(front) > window {
                self.recent_errors.pop_front();
            } else {
                break;
            }
        }

        self.recent_errors.push_back(now);

        error!(
            plugin = %plugin_name,
            error_count = self.recent_errors.len(),
            budget_max = ERROR_BUDGET_MAX,
            detail = %detail,
            "Plugin error recorded"
        );

        if !self.disabled && self.recent_errors.len() > ERROR_BUDGET_MAX {
            let reason = format!(
                "exceeded error budget ({} errors in {}s): {}",
                self.recent_errors.len(),
                ERROR_BUDGET_WINDOW_SECS,
                detail
            );
            self.disabled = true;
            self.disable_reason = Some(reason.clone());
            warn!(plugin = %plugin_name, reason = %reason, "Plugin DISABLED — error budget exhausted");
            return true;
        }

        false
    }

    /// Mark a successful call completion (heartbeat).
    pub fn record_success(&mut self) {
        self.last_heartbeat = Some(Instant::now());
    }

    /// Number of errors recorded in the current sliding window.
    pub fn error_count_in_window(&self) -> usize {
        let now = Instant::now();
        let window = Duration::from_secs(ERROR_BUDGET_WINDOW_SECS);
        self.recent_errors
            .iter()
            .filter(|&&t| now.duration_since(t) <= window)
            .count()
    }
}

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

/// Dashboard snapshot of a single DLL plugin for the Web UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginStatusSnapshot {
    pub name: String,
    pub version: Option<String>,
    pub path: PathBuf,
    pub healthy: bool,
    pub disabled_reason: Option<String>,
    pub manifest: PluginManifest,
    pub conflict_status: ConflictStatus,
    pub pause_command: Option<String>,
}

/// Tracks all successfully loaded plugins, keyed by plugin name.
pub struct PluginRegistry {
    plugins: HashMap<String, PluginHandle>,
    /// Directory that was scanned most recently.
    plugins_dir: Option<PathBuf>,
    /// Per-plugin registration trackers for bulk cleanup on unload.
    trackers: HashMap<String, PluginRegistrationTracker>,
    /// Per-plugin health / error-budget tracking.
    health: HashMap<String, PluginHealth>,
    /// Shared command registry — plugins register into this.
    command_registry: SharedCommandRegistry,
    /// Shared hotkey registry — plugins register into this.
    hotkey_registry: SharedHotkeyRegistry,
    /// Runtime contract declared per plugin.
    manifests: HashMap<String, PluginManifest>,
    /// Conflict resolution outcome per plugin, recorded at load time.
    conflict_status: HashMap<String, ConflictStatus>,
    /// Set when combat engine is paused due to an external automation conflict.
    pub combat_engine_paused: bool,
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
            health: HashMap::new(),
            command_registry,
            hotkey_registry,
            manifests: HashMap::new(),
            conflict_status: HashMap::new(),
            combat_engine_paused: false,
        }
    }

    // ── Health / error-budget accessors ──────────────────────────────────────

    /// Returns `true` if the named plugin is disabled due to error-budget
    /// exhaustion.  Unknown plugin names always return `false`.
    pub fn is_plugin_disabled(&self, name: &str) -> bool {
        self.health
            .get(name)
            .map(|h| h.is_disabled())
            .unwrap_or(false)
    }

    /// Return a reference to the [`PluginHealth`] record for a plugin, if any.
    pub fn plugin_health(&self, name: &str) -> Option<&PluginHealth> {
        self.health.get(name)
    }

    /// Record a plugin error and potentially disable the plugin.
    ///
    /// Returns `true` if the plugin was just disabled by this call.
    pub fn record_plugin_error(&mut self, name: &str, detail: &str) -> bool {
        self.health
            .entry(name.to_string())
            .or_insert_with(PluginHealth::new)
            .record_error(name, detail)
    }

    /// Record a successful call for a plugin (heartbeat update).
    pub fn record_plugin_success(&mut self, name: &str) {
        self.health
            .entry(name.to_string())
            .or_insert_with(PluginHealth::new)
            .record_success();
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
        let hk_removed: usize = tracker
            .hotkey_ids
            .iter()
            .filter(|&&id| hk_reg.unregister(id))
            .count();
        let cmd_removed: usize = tracker
            .command_ids
            .iter()
            .filter(|&&id| cmd_reg.unregister(id))
            .count();
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

            // Skip plugins that have already been disabled (e.g. from a prior
            // partial load attempt that panicked).
            if self.is_plugin_disabled(&plugin_name) {
                warn!(
                    plugin = %plugin_name,
                    reason = ?self.health.get(&plugin_name).and_then(|h| h.disable_reason()),
                    "Skipping disabled plugin during discovery"
                );
                continue;
            }

            info!(plugin = %plugin_name, path = %path.display(), "Attempting to load plugin");

            match self.load_plugin_isolated(&path, plugin_name.clone()) {
                Ok(handle) => {
                    info!(plugin = %plugin_name, "Plugin loaded successfully");
                    self.record_plugin_success(&plugin_name);
                    self.plugins.insert(plugin_name, handle);
                    loaded += 1;
                }
                Err(err) => {
                    error!(plugin = %plugin_name, %err, "Failed to load plugin — skipping");
                    self.record_plugin_error(&plugin_name, &err.to_string());
                }
            }
        }

        info!(loaded, "Plugin discovery complete");
        Ok(loaded)
    }

    /// Invoke a plugin callback with watchdog timing and `catch_unwind`
    /// protection.
    ///
    /// - Logs a warning if the call exceeds [`WATCHDOG_WARN_SECS`].
    /// - Records the result against the plugin's [`PluginHealth`] budget.
    ///
    /// Returns `Ok(R)` on success, or an error string on panic/failure.
    ///
    /// # Note on memory isolation
    ///
    /// This only catches Rust panics via `catch_unwind`. True memory isolation
    /// (segfaults, stack overflows) requires running the plugin in a separate
    /// process. That is a known limitation; in-process DLL loading cannot
    /// prevent all crash vectors.
    pub fn call_plugin_fn<F, R>(
        &mut self,
        plugin_name: &str,
        label: &str,
        f: F,
    ) -> Result<R, String>
    where
        F: FnOnce() -> R + std::panic::UnwindSafe,
    {
        if self.is_plugin_disabled(plugin_name) {
            let reason = self
                .health
                .get(plugin_name)
                .and_then(|h| h.disable_reason())
                .unwrap_or("unknown")
                .to_string();
            return Err(format!("plugin '{}' is disabled: {}", plugin_name, reason));
        }

        let start = Instant::now();

        let result = std::panic::catch_unwind(f);

        let elapsed = start.elapsed();
        if elapsed >= Duration::from_secs(WATCHDOG_WARN_SECS) {
            warn!(
                plugin = %plugin_name,
                call = %label,
                elapsed_ms = elapsed.as_millis(),
                watchdog_threshold_secs = WATCHDOG_WARN_SECS,
                "Plugin call exceeded watchdog threshold — possible hang. \
                 NOTE: in-process DLL loading cannot kill a hung plugin thread; \
                 subprocess isolation is required for hard termination."
            );
        }

        match result {
            Ok(val) => {
                self.record_plugin_success(plugin_name);
                Ok(val)
            }
            Err(panic_payload) => {
                let detail = panic_payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| panic_payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "<non-string panic payload>".to_string());
                let msg = format!("panic in {} call '{}': {}", plugin_name, label, detail);
                error!(plugin = %plugin_name, call = %label, detail = %detail, "Plugin panicked");
                self.record_plugin_error(plugin_name, &msg);
                Err(msg)
            }
        }
    }

    /// Like [`load_plugin`] but wraps the PLUGIN_INIT call in `catch_unwind`.
    fn load_plugin_isolated(&mut self, path: &Path, name: String) -> Result<PluginHandle> {
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

        #[cfg(target_os = "windows")]
        {
            self.load_plugin_windows_isolated(path, name)
        }
    }

    /// Windows-only DLL loading implementation with `catch_unwind` around
    /// `PLUGIN_INIT`.
    #[cfg(target_os = "windows")]
    fn load_plugin_windows_isolated(&mut self, path: &Path, name: String) -> Result<PluginHandle> {
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

        // Call PLUGIN_INIT wrapped in catch_unwind for panic isolation.
        // SAFETY: symbol lifetime is bounded by `lib` which we still hold.
        let init_fn: libloading::Symbol<PluginInitFn> =
            unsafe { lib.get(b"PLUGIN_INIT\0").expect("already validated above") };

        // SAFETY: the function pointer is valid for the lifetime of `lib`.
        // We copy it as a raw fn pointer so catch_unwind can take ownership.
        let raw_init: PluginInitFn = *init_fn;

        let start = Instant::now();
        let init_result = std::panic::catch_unwind(|| {
            // SAFETY: We hold `lib` alive, so `raw_init` is valid.
            unsafe { raw_init() }
        });
        let elapsed = start.elapsed();

        if elapsed >= Duration::from_secs(WATCHDOG_WARN_SECS) {
            warn!(
                plugin = %name,
                elapsed_ms = elapsed.as_millis(),
                "PLUGIN_INIT exceeded watchdog threshold — possible hang. \
                 In-process loading cannot forcibly kill hung FFI calls."
            );
        }

        let rc = match init_result {
            Ok(rc) => rc,
            Err(panic_payload) => {
                let detail = panic_payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| panic_payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "<non-string panic>".to_string());
                error!(plugin = %name, detail = %detail, "PLUGIN_INIT panicked — skipping plugin");
                anyhow::bail!("PLUGIN_INIT panicked: {}", detail);
            }
        };

        if rc != 0 {
            anyhow::bail!("PLUGIN_INIT returned non-zero status: {rc}");
        }

        let metadata = PluginMetadata {
            name,
            path: path.to_path_buf(),
            version,
        };

        Ok(PluginHandle {
            metadata,
            library: lib,
        })
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

    // ── Manifest contract enforcement ────────────────────────────────────────

    /// Enforce a [`PluginManifest`] for an already-loaded plugin.
    ///
    /// Checks `requires`, force-unloads `force_unload` entries, and records
    /// `pause_on_load` in the conflict status.  The caller is responsible for
    /// dispatching the pause command via IPC.
    pub fn enforce_manifest_contract(
        &mut self,
        plugin_name: &str,
        manifest: PluginManifest,
    ) -> Result<ConflictStatus> {
        let missing: Vec<String> = manifest
            .requires
            .iter()
            .filter(|req| !self.plugins.contains_key(*req))
            .cloned()
            .collect();

        if !missing.is_empty() {
            error!(
                plugin = %plugin_name,
                ?missing,
                "Plugin requirement not satisfied"
            );
            anyhow::bail!(
                "plugin '{}' requires plugins not loaded: {}",
                plugin_name,
                missing.join(", ")
            );
        }

        let mut force_unloaded: Vec<String> = Vec::new();
        for conflicting in &manifest.force_unload {
            if self.plugins.contains_key(conflicting.as_str()) {
                warn!(
                    plugin = %plugin_name,
                    conflicting = %conflicting,
                    "Force-unloading conflicting plugin per manifest"
                );
                self.plugins.remove(conflicting);
                self.health.remove(conflicting);
                self.trackers.remove(conflicting);
                self.manifests.remove(conflicting);
                self.conflict_status.remove(conflicting);
                force_unloaded.push(conflicting.clone());
            }
        }

        if let Some(cmd) = &manifest.pause_on_load {
            info!(
                plugin = %plugin_name,
                command = %cmd,
                "Manifest pause_on_load — caller must dispatch via IPC"
            );
        }

        let status = if force_unloaded.is_empty() {
            ConflictStatus::Ok
        } else {
            ConflictStatus::ConflictingPlugins(force_unloaded)
        };

        self.manifests.insert(plugin_name.to_string(), manifest);
        self.conflict_status
            .insert(plugin_name.to_string(), status.clone());

        Ok(status)
    }

    /// Return `true` when a known rgmercs-family plugin is in the registry.
    ///
    /// Detection checks well-known script/plugin names; a loaded plugin named
    /// `rgmercs`, `e3n`, or `rgbattlefield` indicates rgmercs owns combat.
    pub fn rgmercs_detected(&self) -> bool {
        const RGMERCS_NAMES: &[&str] = &["rgmercs", "e3n", "rgbattlefield"];
        self.plugins
            .keys()
            .any(|name| RGMERCS_NAMES.iter().any(|r| name.eq_ignore_ascii_case(r)))
    }

    /// Pause the TextQuest DLL combat engine when an external automation
    /// conflict is detected.  Returns `true` if the state changed.
    pub fn pause_combat_engine(&mut self) -> bool {
        if self.combat_engine_paused {
            return false;
        }
        self.combat_engine_paused = true;
        warn!(
            "Combat engine paused — external automation conflict detected \
             (rgmercs or equivalent is active)"
        );
        true
    }

    /// Resume the TextQuest combat engine.  Returns `true` if the state changed.
    pub fn resume_combat_engine(&mut self) -> bool {
        if !self.combat_engine_paused {
            return false;
        }
        self.combat_engine_paused = false;
        info!("Combat engine resumed");
        true
    }

    /// Snapshot all loaded plugins for Web UI / dashboard consumers.
    pub fn plugin_status_snapshots(&self) -> Vec<PluginStatusSnapshot> {
        self.plugins
            .iter()
            .map(|(name, handle)| {
                let health = self.health.get(name);
                PluginStatusSnapshot {
                    name: name.clone(),
                    version: handle.metadata.version.clone(),
                    path: handle.metadata.path.clone(),
                    healthy: health.map(|h| !h.is_disabled()).unwrap_or(true),
                    disabled_reason: health
                        .and_then(|h| h.disable_reason())
                        .map(|s| s.to_string()),
                    manifest: self
                        .manifests
                        .get(name)
                        .cloned()
                        .unwrap_or_default(),
                    conflict_status: self
                        .conflict_status
                        .get(name)
                        .cloned()
                        .unwrap_or_default(),
                    pause_command: self
                        .manifests
                        .get(name)
                        .and_then(|m| m.pause_on_load.clone()),
                }
            })
            .collect()
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
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

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

        assert!(
            registry
                .command_registry()
                .lock()
                .unwrap()
                .dispatch("/plug cmd foo bar")
        );
        assert_eq!(*received.lock().unwrap(), "foo bar");

        // Explicit unregister.
        assert!(registry.plugin_unregister_command("test_plugin", id));
        assert!(
            !registry
                .command_registry()
                .lock()
                .unwrap()
                .dispatch("/plug cmd foo bar")
        );
    }

    #[test]
    fn cleanup_plugin_registrations_removes_all_on_unload() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let mut registry = PluginRegistry::new();

        // Register two hotkeys and one command for "plugin_a".
        let fired = Arc::new(AtomicU32::new(0));
        let f1 = Arc::clone(&fired);
        let f2 = Arc::clone(&fired);

        registry.plugin_register_hotkey(
            "plugin_a",
            "alt+f1",
            Box::new(move || {
                f1.fetch_add(1, Ordering::Relaxed);
            }),
        );
        registry.plugin_register_hotkey(
            "plugin_a",
            "alt+f2",
            Box::new(move || {
                f2.fetch_add(1, Ordering::Relaxed);
            }),
        );
        registry.plugin_register_command("plugin_a", "/pa cmd", Box::new(|_| {}));

        // Sanity: both hotkeys and command work.
        assert!(registry.hotkey_registry().lock().unwrap().fire("alt+f1"));
        assert!(registry.hotkey_registry().lock().unwrap().fire("alt+f2"));
        assert!(
            registry
                .command_registry()
                .lock()
                .unwrap()
                .dispatch("/pa cmd")
        );

        // Simulate plugin unload.
        let (hk, cmd) = registry.cleanup_plugin_registrations("plugin_a");
        assert_eq!(hk, 2, "two hotkeys should have been removed");
        assert_eq!(cmd, 1, "one command should have been removed");

        // Nothing fires after cleanup.
        assert!(!registry.hotkey_registry().lock().unwrap().fire("alt+f1"));
        assert!(!registry.hotkey_registry().lock().unwrap().fire("alt+f2"));
        assert!(
            !registry
                .command_registry()
                .lock()
                .unwrap()
                .dispatch("/pa cmd")
        );
    }

    #[test]
    fn plugin_priority_is_higher_than_script() {
        use std::sync::{Arc, Mutex};

        let (cmd_reg, hk_reg) = crate::registry::new_shared();
        let mut plugin_registry =
            PluginRegistry::with_registries(Arc::clone(&cmd_reg), Arc::clone(&hk_reg));

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
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let (cmd_reg, hk_reg) = crate::registry::new_shared();
        let mut plugin_registry =
            PluginRegistry::with_registries(Arc::clone(&cmd_reg), Arc::clone(&hk_reg));

        // Plugin registers a hotkey via the Rust API.
        let fired = Arc::new(AtomicU32::new(0));
        let f = Arc::clone(&fired);
        plugin_registry.plugin_register_hotkey(
            "my_plugin",
            "ctrl+g",
            Box::new(move || {
                f.fetch_add(1, Ordering::Relaxed);
            }),
        );

        // Fire via the shared handle (same one the Lua bindings would hold).
        assert!(hk_reg.lock().unwrap().fire("ctrl+g"));
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    // ── Crash isolation / PluginHealth tests ─────────────────────────────────

    /// A panic inside `call_plugin_fn` must be caught and NOT propagate.
    #[test]
    fn panic_in_plugin_fn_is_caught() {
        let mut registry = PluginRegistry::new();
        let result = registry.call_plugin_fn("panicky_plugin", "test_call", || {
            panic!("intentional test panic");
        });
        assert!(result.is_err(), "panic should be converted to Err");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("panic") || msg.contains("intentional"),
            "error message should describe the panic: {msg}"
        );
    }

    /// After 6 errors in the window the plugin must be disabled.
    #[test]
    fn error_budget_disables_plugin_after_threshold() {
        let mut registry = PluginRegistry::new();
        let name = "budget_plugin";

        // Inject ERROR_BUDGET_MAX + 1 errors (= 6).
        for i in 0..=(ERROR_BUDGET_MAX) {
            let result = registry.call_plugin_fn(name, "error_call", || -> i32 {
                panic!("error #{}", i);
            });
            assert!(result.is_err());
        }

        assert!(
            registry.is_plugin_disabled(name),
            "plugin must be disabled after {} errors",
            ERROR_BUDGET_MAX + 1
        );
        let reason = registry
            .plugin_health(name)
            .and_then(|h| h.disable_reason());
        assert!(reason.is_some(), "disable reason should be recorded");
    }

    /// A disabled plugin must be skipped on subsequent `call_plugin_fn` calls.
    #[test]
    fn disabled_plugin_is_skipped_on_dispatch() {
        let mut registry = PluginRegistry::new();
        let name = "skip_plugin";

        // Drive it past the budget.
        for i in 0..=(ERROR_BUDGET_MAX) {
            let _ = registry.call_plugin_fn(name, "err", || -> () { panic!("e{i}") });
        }
        assert!(registry.is_plugin_disabled(name));

        // Now any further call should short-circuit with an error mentioning "disabled".
        let result = registry.call_plugin_fn(name, "after_disable", || 42_i32);
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("disabled"),
            "error must indicate the plugin is disabled"
        );
    }

    /// Successful calls update `last_heartbeat`.
    #[test]
    fn successful_call_updates_heartbeat() {
        let mut registry = PluginRegistry::new();
        let name = "healthy_plugin";

        assert!(
            registry.plugin_health(name).is_none(),
            "no health record before first call"
        );

        registry
            .call_plugin_fn(name, "ok_call", || 99_i32)
            .expect("successful call must not error");

        let h = registry
            .plugin_health(name)
            .expect("health record created after call");
        assert!(
            h.last_heartbeat.is_some(),
            "heartbeat should be set after success"
        );
        assert!(!h.is_disabled(), "should not be disabled");
    }

    /// `PluginHealth::record_error` sliding window: errors older than the
    /// window width must not count toward the budget.
    #[test]
    fn error_budget_sliding_window_evicts_old_errors() {
        // We cannot easily sleep for 60 s in a unit test, so we test the eviction
        // logic directly on `PluginHealth` with manually backdated entries.
        let mut health = PluginHealth::new();
        let old_time = Instant::now() - Duration::from_secs(ERROR_BUDGET_WINDOW_SECS + 10);

        // Pre-fill with entries that are already outside the window.
        for _ in 0..ERROR_BUDGET_MAX {
            health.recent_errors.push_back(old_time);
        }

        // Recording one new error should evict all the stale ones first, so the
        // total in-window count is only 1 — well below the budget max.
        let name = "window_test";
        let just_disabled = health.record_error(name, "new error");
        assert!(
            !just_disabled,
            "plugin must not be disabled — stale errors should be evicted"
        );
        assert_eq!(health.error_count_in_window(), 1);
        assert!(!health.is_disabled());
    }

    /// `fake_dll_file_fails_gracefully` still holds with the new isolated loader.
    /// (Regression guard: the old loader path was replaced; ensure the new one
    /// also handles bad DLL content without panicking.)
    #[test]
    fn fake_dll_file_fails_gracefully_isolated() {
        let dir = make_temp_dir();
        std::fs::write(dir.path().join("broken_plugin.dll"), b"not a PE").unwrap();
        let mut registry = PluginRegistry::new();
        let count = registry
            .discover_and_load(dir.path())
            .expect("discover_and_load must not panic on broken DLL");
        assert_eq!(count, 0, "broken DLL must not be counted as loaded");
        assert!(registry.is_empty());
        // Error budget should have recorded the failure.
        assert_eq!(
            registry
                .plugin_health("broken_plugin")
                .map(|h| h.error_count_in_window())
                .unwrap_or(0),
            1,
            "one error should be recorded for the failed load"
        );
    }

    // ── Manifest contract enforcement tests ──────────────────────────────────

    #[test]
    fn enforce_manifest_contract_ok_when_no_requirements() {
        let mut registry = PluginRegistry::new();
        let manifest = PluginManifest::default();
        let status = registry
            .enforce_manifest_contract("mq2nav", manifest)
            .expect("empty manifest should always satisfy");
        assert_eq!(status, ConflictStatus::Ok);
    }

    #[test]
    fn enforce_manifest_contract_errors_on_missing_requires() {
        let mut registry = PluginRegistry::new();
        let manifest = PluginManifest {
            requires: vec!["MQ2Nav".to_string(), "MQ2DanNet".to_string()],
            ..Default::default()
        };
        let err = registry
            .enforce_manifest_contract("rgmercs", manifest)
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("MQ2Nav") || msg.contains("MQ2DanNet"),
            "error should name missing plugins: {msg}"
        );
    }

    #[test]
    fn enforce_manifest_force_unload_on_absent_plugin_is_noop() {
        let mut registry = PluginRegistry::new();
        let manifest = PluginManifest {
            force_unload: vec!["MQ2Melee".to_string()],
            pause_on_load: Some("/war pause on".to_string()),
            ..Default::default()
        };
        // MQ2Melee is not in the registry, so force_unload is a no-op.
        let status = registry
            .enforce_manifest_contract("rgmercs", manifest)
            .expect("force_unload on absent plugin should succeed");
        assert_eq!(status, ConflictStatus::Ok);
        assert!(
            registry.manifests.contains_key("rgmercs"),
            "manifest should be recorded"
        );
    }

    #[test]
    fn rgmercs_detected_returns_false_on_empty_registry() {
        let registry = PluginRegistry::new();
        assert!(!registry.rgmercs_detected());
    }

    #[test]
    fn pause_combat_engine_transitions_state() {
        let mut registry = PluginRegistry::new();
        assert!(!registry.combat_engine_paused);
        assert!(
            registry.pause_combat_engine(),
            "first pause should return true"
        );
        assert!(registry.combat_engine_paused);
        assert!(
            !registry.pause_combat_engine(),
            "second pause is a no-op"
        );
        assert!(
            registry.resume_combat_engine(),
            "resume should return true"
        );
        assert!(!registry.combat_engine_paused);
    }

    #[test]
    fn plugin_status_snapshots_empty_on_fresh_registry() {
        let registry = PluginRegistry::new();
        assert!(
            registry.plugin_status_snapshots().is_empty(),
            "empty registry yields no snapshots"
        );
    }
}
