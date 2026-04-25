//! MacroQuest plugin discovery and loader scaffold.
//!
//! This module is intentionally small: it establishes the host API contract,
//! discovers candidate plugins, and owns the DLL lifecycle handle on Windows.
//! Full MQ2 binary compatibility needs more exported APIs and out-of-process
//! crash isolation, so callers should treat this as the first compatibility
//! layer rather than a complete MQ2 runtime.

use std::{
    collections::BTreeMap,
    ffi::{c_char, c_void},
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use super::bridge::MQ2Bridge;

/// Version of the C-compatible TextQuest MQ2 host API.
pub const TEXTQUEST_MQ2_API_VERSION: u32 = 1;

/// Minimal MQ2-like host surface exposed to plugin code.
///
/// This starts with command execution because it is the common denominator for
/// plugin init, reload, and simple command handler tests. Additional MQ2 APIs
/// should be added here only when TextQuest has a stable backing implementation.
pub trait MacroQuestPluginApi {
    /// Execute an EverQuest slash command through TextQuest.
    fn cmd(&self, slash_command: &str);
}

impl<'tick> MacroQuestPluginApi for MQ2Bridge<'tick> {
    fn cmd(&self, slash_command: &str) {
        MQ2Bridge::cmd(self, slash_command);
    }
}

/// C ABI surface passed to TextQuest-aware MQ2 compatibility plugins.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TextQuestMq2Api {
    /// Must match [`TEXTQUEST_MQ2_API_VERSION`].
    pub version: u32,
    /// Opaque host-owned pointer passed back to callbacks.
    pub context: *mut c_void,
    /// Execute an EQ slash command. Returns 0 on success.
    pub cmd: Option<unsafe extern "C" fn(context: *mut c_void, command: *const c_char) -> i32>,
}

impl TextQuestMq2Api {
    /// Return an empty API table with the current version marker.
    ///
    /// The caller may populate callbacks when a stable host context is
    /// available. Keeping this constructor callback-free avoids leaking
    /// game-tick scoped bridge pointers into long-lived plugins.
    pub const fn empty() -> Self {
        Self {
            version: TEXTQUEST_MQ2_API_VERSION,
            context: std::ptr::null_mut(),
            cmd: None,
        }
    }
}

/// Runtime contract declared for an MQ2 plugin by the operator config.
///
/// Since external MQ2 DLLs cannot export TextQuest manifests, operators declare
/// these in their plugin config (e.g. to model rgmercs' `init.lua` contract).
/// The loader enforces this before calling `InitializePlugin`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeManifest {
    /// Plugin names (without extension) that must be loaded before this plugin.
    pub requires: Vec<String>,
    /// Plugin names (without extension) that must be unloaded before this plugin.
    pub force_unload: Vec<String>,
    /// Optional EQ slash command issued after this plugin initialises.
    pub pause_on_load: Option<String>,
}

/// Per-plugin configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginConfig {
    /// Disabled plugins are discovered but skipped by bulk loading.
    pub enabled: bool,
    /// Free-form string settings owned by the specific plugin.
    pub settings: BTreeMap<String, String>,
    /// Plugins that must be loaded before this one can start.
    pub required_plugins: Vec<String>,
    /// Plugins that conflict with this one and must be unloaded when this loads.
    pub conflicts_with: Vec<String>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            settings: BTreeMap::new(),
            required_plugins: Vec::new(),
            conflicts_with: Vec::new(),
        }
    }
}

/// Discovered plugin file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCandidate {
    pub name: String,
    pub path: PathBuf,
}

impl PluginCandidate {
    fn from_path(path: PathBuf) -> Option<Self> {
        let extension = path.extension()?.to_string_lossy();
        if !is_plugin_extension(&extension) {
            return None;
        }

        let name = path.file_stem()?.to_string_lossy().into_owned();
        Some(Self { name, path })
    }
}

/// Loader for MacroQuest-compatible plugin candidates.
pub struct MacroQuestPluginLoader {
    plugins_dir: PathBuf,
    configs: BTreeMap<String, PluginConfig>,
    loaded: std::collections::BTreeSet<String>,
}

impl std::fmt::Debug for MacroQuestPluginLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MacroQuestPluginLoader")
            .field("plugins_dir", &self.plugins_dir)
            .field("configs", &self.configs)
            .field("loaded", &self.loaded)
            .finish()
    }
}

impl MacroQuestPluginLoader {
    /// Create a loader rooted at a plugin directory.
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugins_dir: plugins_dir.into(),
            configs: BTreeMap::new(),
            loaded: std::collections::BTreeSet::new(),
        }
    }

    /// Create a loader with pre-parsed per-plugin configuration.
    pub fn with_configs(
        plugins_dir: impl Into<PathBuf>,
        configs: BTreeMap<String, PluginConfig>,
    ) -> Self {
        Self {
            plugins_dir: plugins_dir.into(),
            configs,
            loaded: std::collections::BTreeSet::new(),
        }
    }

    /// Return the plugin directory scanned by this loader.
    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    /// Discover `.dll` and `.mq2` plugin candidates.
    ///
    /// Missing plugin directories are treated as empty so TextQuest can start
    /// without requiring users to create the directory first.
    pub fn discover(&self) -> io::Result<Vec<PluginCandidate>> {
        if !self.plugins_dir.exists() {
            return Ok(Vec::new());
        }

        let mut candidates = Vec::new();
        for entry in fs::read_dir(&self.plugins_dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_file() {
                continue;
            }

            if let Some(candidate) = PluginCandidate::from_path(entry.path()) {
                candidates.push(candidate);
            }
        }

        candidates.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(candidates)
    }

    /// Return config for a plugin name, defaulting to enabled.
    pub fn config_for(&self, plugin_name: &str) -> PluginConfig {
        self.configs.get(plugin_name).cloned().unwrap_or_default()
    }

    /// Check if a plugin is currently loaded.
    pub fn is_loaded(&self, plugin_name: &str) -> bool {
        self.loaded.contains(plugin_name)
    }

    /// Get list of currently loaded plugins.
    pub fn loaded_plugins(&self) -> Vec<String> {
        self.loaded.iter().cloned().collect()
    }

    /// Validate that all required plugins for the given plugin are loaded.
    fn validate_required_plugins(&self, plugin_name: &str) -> Result<(), PluginLoadError> {
        let config = self.config_for(plugin_name);
        let missing: Vec<String> = config
            .required_plugins
            .into_iter()
            .filter(|req| !self.is_loaded(req))
            .collect();

        if !missing.is_empty() {
            return Err(PluginLoadError::MissingRequiredPlugins {
                plugin: plugin_name.to_string(),
                missing,
            });
        }
        Ok(())
    }

    /// Unload plugins that conflict with the given plugin.
    fn unload_conflicts(&mut self, plugin_name: &str) -> Result<(), PluginLoadError> {
        let config = self.config_for(plugin_name);
        let conflicts = config.conflicts_with.clone();

        for conflict in &conflicts {
            if self.is_loaded(conflict) {
                self.loaded.remove(conflict);
                tracing::info!(
                    plugin = %plugin_name,
                    unloaded = %conflict,
                    "auto-unloaded conflicting plugin"
                );
            }
        }
        Ok(())
    }

    /// Load a single discovered plugin.
    ///
    /// Before loading, validates that:
    /// 1. Plugin is enabled
    /// 2. All required plugins are already loaded (runtime contract)
    /// 3. Any conflicting plugins are unloaded (force-unload contract)
    ///
    /// TextQuest-aware plugins may export `TextQuestInitializePlugin` accepting
    /// [`TextQuestMq2Api`]. Legacy-style plugins may export `InitializePlugin`;
    /// they can be loaded but will not receive host callbacks until a wrapper
    /// exports the MQ2 globals they expect.
    pub fn load(
        &mut self,
        candidate: &PluginCandidate,
        api: &TextQuestMq2Api,
    ) -> Result<LoadedMacroQuestPlugin, PluginLoadError> {
        self.load_with_context(candidate, api, &std::collections::BTreeSet::new(), &mut std::collections::BTreeSet::new())
    }

    fn load_with_context(
        &self,
        candidate: &PluginCandidate,
        api: &TextQuestMq2Api,
        loaded_names: &std::collections::BTreeSet<String>,
        force_unloaded: &mut std::collections::BTreeSet<String>,
    ) -> Result<LoadedMacroQuestPlugin, PluginLoadError> {
        let config = self.config_for(&candidate.name);
        if !config.enabled {
            return Err(PluginLoadError::Disabled(candidate.name.clone()));
        }

        // Validate required-load contract
        self.validate_required_plugins(&candidate.name)?;

        // Enforce force-unload contract
        self.unload_conflicts(&candidate.name)?;

        // Load the plugin
        let loaded = load_platform(candidate, api)?;

        // Track the loaded plugin by name
        self.loaded.insert(candidate.name.clone());

        tracing::info!(
            plugin = %candidate.name,
            required = ?config.required_plugins,
            "plugin loaded with runtime contract"
        );

        Ok(loaded)
    }

    /// Discover and load every enabled plugin, enforcing manifest contracts.
    ///
    /// Plugins are loaded in discovery order (alphabetical). If a plugin's
    /// `requires` names something not yet loaded, loading fails with
    /// [`PluginLoadError::UnsatisfiedRequirement`]. Operators should ensure
    /// dependency order via `requires` declarations.
    pub fn load_enabled(
        &mut self,
        api: &TextQuestMq2Api,
    ) -> Result<Vec<LoadedMacroQuestPlugin>, PluginLoadError> {
        let candidates = self
            .discover()
            .map_err(|source| PluginLoadError::Discover {
                path: self.plugins_dir.clone(),
                source,
            })?;

        let mut loaded = Vec::new();
        let mut loaded_names = std::collections::BTreeSet::new();
        let mut force_unloaded: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        for candidate in candidates {
            if !self.config_for(&candidate.name).enabled {
                continue;
            }
            // Skip plugins that a prior manifest declared must be unloaded.
            if force_unloaded.contains(&candidate.name) {
                tracing::info!(
                    plugin = %candidate.name,
                    "skipping plugin: force-unloaded by a prior manifest"
                );
                continue;
            }
            let plugin = self.load_with_context(&candidate, api, &loaded_names, &mut force_unloaded)?;
            loaded_names.insert(candidate.name.clone());
            loaded.push(plugin);
        }
        Ok(loaded)
    }
}

/// Loaded plugin handle. Dropping the handle shuts the plugin down on Windows.
#[derive(Debug)]
pub struct LoadedMacroQuestPlugin {
    candidate: PluginCandidate,
    /// EQ slash command declared by the plugin's manifest, if any.
    pub pause_command: Option<String>,
    #[cfg(windows)]
    library: windows::Win32::Foundation::HMODULE,
    #[cfg(windows)]
    shutdown: Option<unsafe extern "system" fn()>,
}

impl LoadedMacroQuestPlugin {
    pub fn candidate(&self) -> &PluginCandidate {
        &self.candidate
    }
}

#[derive(Debug, Error)]
pub enum PluginLoadError {
    #[error("failed to discover plugins in {path}: {source}")]
    Discover { path: PathBuf, source: io::Error },
    #[error("plugin {0} is disabled")]
    Disabled(String),
    #[error("MacroQuest plugin DLL loading is only supported on Windows")]
    UnsupportedPlatform,
    #[cfg(windows)]
    #[error("failed to load plugin DLL {path}: {source}")]
    LoadLibrary {
        path: PathBuf,
        source: windows::core::Error,
    },
    #[error("plugin {0} does not export TextQuestInitializePlugin or InitializePlugin")]
    MissingInitializeSymbol(PathBuf),
    #[error("plugin {path} returned init status {status}")]
    InitFailed { path: PathBuf, status: i32 },
    #[error("plugin {plugin} requires {missing:?} to be loaded first")]
    MissingRequiredPlugins { plugin: String, missing: Vec<String> },
    #[error("plugin {plugin} requires {required:?} which failed to unload: {reason}")]
    ConflictUnloadFailed {
        plugin: String,
        required: Vec<String>,
        reason: String,
    },
}

fn is_plugin_extension(extension: &str) -> bool {
    extension.eq_ignore_ascii_case("dll") || extension.eq_ignore_ascii_case("mq2")
}

#[cfg(not(windows))]
fn load_platform(
    candidate: &PluginCandidate,
    _api: &TextQuestMq2Api,
) -> Result<LoadedMacroQuestPlugin, PluginLoadError> {
    tracing::debug!(
        plugin = %candidate.name,
        path = %candidate.path.display(),
        "MQ2 plugin loading skipped on non-Windows host"
    );
    Err(PluginLoadError::UnsupportedPlatform)
}

#[cfg(not(windows))]
#[allow(dead_code)]
fn make_loaded_non_windows(candidate: PluginCandidate) -> LoadedMacroQuestPlugin {
    LoadedMacroQuestPlugin {
        candidate,
        pause_command: None,
    }
}

#[cfg(windows)]
fn load_platform(
    candidate: &PluginCandidate,
    api: &TextQuestMq2Api,
) -> Result<LoadedMacroQuestPlugin, PluginLoadError> {
    use std::os::windows::ffi::OsStrExt;

    use windows::{
        Win32::System::LibraryLoader::{FreeLibrary, GetProcAddress, LoadLibraryW},
        core::{PCSTR, PCWSTR},
    };

    type TextQuestInitializePlugin = unsafe extern "system" fn(*const TextQuestMq2Api) -> i32;
    type LegacyInitializePlugin = unsafe extern "system" fn();
    type ShutdownPlugin = unsafe extern "system" fn();

    unsafe fn get_textquest_initialize(
        library: windows::Win32::Foundation::HMODULE,
    ) -> Option<TextQuestInitializePlugin> {
        let proc =
            unsafe { GetProcAddress(library, PCSTR(b"TextQuestInitializePlugin\0".as_ptr())) }?;
        Some(unsafe { std::mem::transmute(proc) })
    }

    unsafe fn get_legacy_initialize(
        library: windows::Win32::Foundation::HMODULE,
    ) -> Option<LegacyInitializePlugin> {
        let proc = unsafe { GetProcAddress(library, PCSTR(b"InitializePlugin\0".as_ptr())) }?;
        Some(unsafe { std::mem::transmute(proc) })
    }

    unsafe fn get_shutdown(library: windows::Win32::Foundation::HMODULE) -> Option<ShutdownPlugin> {
        let proc = unsafe { GetProcAddress(library, PCSTR(b"ShutdownPlugin\0".as_ptr())) }?;
        Some(unsafe { std::mem::transmute(proc) })
    }

    let wide_path: Vec<u16> = candidate
        .path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let library = unsafe { LoadLibraryW(PCWSTR(wide_path.as_ptr())) }.map_err(|source| {
        PluginLoadError::LoadLibrary {
            path: candidate.path.clone(),
            source,
        }
    })?;

    let shutdown = unsafe { get_shutdown(library) };

    if let Some(init) = unsafe { get_textquest_initialize(library) } {
        let status = unsafe { init(api as *const TextQuestMq2Api) };
        if status != 0 {
            unsafe {
                FreeLibrary(library);
            }
            return Err(PluginLoadError::InitFailed {
                path: candidate.path.clone(),
                status,
            });
        }
    } else if let Some(init) = unsafe { get_legacy_initialize(library) } {
        unsafe {
            init();
        }
    } else {
        unsafe {
            FreeLibrary(library);
        }
        return Err(PluginLoadError::MissingInitializeSymbol(
            candidate.path.clone(),
        ));
    }

    Ok(LoadedMacroQuestPlugin {
        candidate: candidate.clone(),
        pause_command: None, // set by load_with_context after platform load
        library,
        shutdown,
    })
}

#[cfg(windows)]
impl Drop for LoadedMacroQuestPlugin {
    fn drop(&mut self) {
        use windows::Win32::System::LibraryLoader::FreeLibrary;

        unsafe {
            if let Some(shutdown) = self.shutdown {
                shutdown();
            }
            FreeLibrary(self.library);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_returns_empty_for_missing_plugin_dir() {
        let loader = MacroQuestPluginLoader::new("target/definitely-missing-plugin-dir");

        let discovered = loader.discover().expect("missing directory is not fatal");

        assert!(discovered.is_empty());
    }

    #[test]
    fn discover_filters_plugin_extensions_and_sorts_by_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("zeta.txt"), b"skip").expect("write txt");
        fs::write(dir.path().join("beta.mq2"), b"plugin").expect("write mq2");
        fs::write(dir.path().join("alpha.DLL"), b"plugin").expect("write dll");
        fs::create_dir(dir.path().join("nested.dll")).expect("nested dir");

        let loader = MacroQuestPluginLoader::new(dir.path());
        let names: Vec<_> = loader
            .discover()
            .expect("discover")
            .into_iter()
            .map(|candidate| candidate.name)
            .collect();

        assert_eq!(names, ["alpha", "beta"]);
    }

    #[test]
    fn config_for_defaults_to_enabled_and_respects_overrides() {
        let mut configs = BTreeMap::new();
        configs.insert(
            "mq2eqbc".to_string(),
            PluginConfig {
                enabled: false,
                settings: BTreeMap::from([("server".to_string(), "127.0.0.1".to_string())]),
                required_plugins: Vec::new(),
                conflicts_with: Vec::new(),
            },
        );

        let loader = MacroQuestPluginLoader::with_configs("plugins", configs);

        assert!(loader.config_for("mq2lua").enabled);
        assert!(!loader.config_for("mq2eqbc").enabled);
    }
}
