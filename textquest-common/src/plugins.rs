//! Plugin API contracts shared by TextQuest runtimes and external extensions.
//!
//! This module intentionally starts with an in-process registry and Rust trait
//! contract. Dynamic library discovery can layer on top of the `PluginFactory`
//! entry point once the ABI boundary is finalized.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Runtime contract declared by a plugin at registration time.
///
/// The loader enforces this before the plugin's `on_load` is called:
/// - All `requires` names must already be loaded and enabled.
/// - All `force_unload` names are disabled before this plugin loads.
/// - `pause_on_load` is an EQ slash command the host should issue after load.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin names that must be loaded and enabled before this plugin activates.
    pub requires: Vec<String>,
    /// Plugin names that must be disabled before this plugin activates.
    /// The loader unloads them, and may restore on unload.
    pub force_unload: Vec<String>,
    /// Optional EQ slash command issued after this plugin loads (e.g. `/enc pause on`).
    pub pause_on_load: Option<String>,
}

impl PluginManifest {
    pub fn is_empty(&self) -> bool {
        self.requires.is_empty()
            && self.force_unload.is_empty()
            && self.pause_on_load.is_none()
    }
}

/// Conflict resolution status for a loaded plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ConflictStatus {
    /// No manifest constraints or all constraints satisfied.
    #[default]
    Ok,
    /// One or more required plugins are absent or disabled.
    MissingRequirements(Vec<String>),
    /// One or more conflicting plugins could not be unloaded.
    ConflictingPlugins(Vec<String>),
}

/// Domain surfaces a plugin can extend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PluginDomain {
    /// Combat rotations, assist logic, targeting, and class strategy hooks.
    Combat,
    /// Navigation, pathing, camp movement, and zone-transition hooks.
    Navigation,
    /// Inventory, loot, banking, vendor, and item-management hooks.
    Inventory,
    /// Runtime-level extensions that do not fit a gameplay domain.
    System,
}

/// A single operation or hook exposed by a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginCapability {
    pub domain: PluginDomain,
    pub name: String,
    pub description: String,
}

impl PluginCapability {
    pub fn new(
        domain: PluginDomain,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            domain,
            name: name.into(),
            description: description.into(),
        }
    }
}

/// Static plugin identity and advertised capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub capabilities: Vec<PluginCapability>,
    /// Runtime contract this plugin declares. Enforced by `PluginRegistry::register`.
    pub manifest: PluginManifest,
}

impl PluginMetadata {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            description: description.into(),
            capabilities: Vec::new(),
            manifest: PluginManifest::default(),
        }
    }

    pub fn with_manifest(mut self, manifest: PluginManifest) -> Self {
        self.manifest = manifest;
        self
    }

    pub fn with_capability(mut self, capability: PluginCapability) -> Self {
        self.add_capability(capability);
        self
    }

    pub fn add_capability(&mut self, capability: PluginCapability) {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
        }
    }
}

/// Context passed to lifecycle hooks for runtime capability registration.
#[derive(Debug, Default)]
pub struct PluginContext {
    capabilities: Vec<PluginCapability>,
}

impl PluginContext {
    pub fn register_capability(&mut self, capability: PluginCapability) {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
        }
    }

    pub fn capabilities(&self) -> &[PluginCapability] {
        &self.capabilities
    }

    fn into_capabilities(self) -> Vec<PluginCapability> {
        self.capabilities
    }
}

/// Standard TextQuest plugin contract.
pub trait TextQuestPlugin: Send {
    fn metadata(&self) -> PluginMetadata;

    fn on_load(&mut self, _context: &mut PluginContext) -> PluginResult<()> {
        Ok(())
    }

    fn on_unload(&mut self) -> PluginResult<()> {
        Ok(())
    }
}

/// Factory signature used by static registrars and future dynamic loaders.
pub type PluginFactory = fn() -> Box<dyn TextQuestPlugin>;

pub type PluginResult<T> = Result<T, PluginError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    EmptyName,
    DuplicatePlugin(String),
    NotFound(String),
    Lifecycle(String),
    /// One or more `requires` entries are absent or disabled.
    UnsatisfiedRequirement { plugin: String, missing: Vec<String> },
    /// One or more `force_unload` entries could not be disabled.
    ConflictingPlugin { plugin: String, conflicts: Vec<String> },
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "plugin name must not be empty"),
            Self::DuplicatePlugin(name) => write!(f, "plugin already registered: {name}"),
            Self::NotFound(name) => write!(f, "plugin not found: {name}"),
            Self::Lifecycle(message) => write!(f, "plugin lifecycle failed: {message}"),
            Self::UnsatisfiedRequirement { plugin, missing } => write!(
                f,
                "plugin {plugin} requires [{deps}] but they are not loaded",
                deps = missing.join(", ")
            ),
            Self::ConflictingPlugin { plugin, conflicts } => write!(
                f,
                "plugin {plugin} could not unload conflicts [{deps}]",
                deps = conflicts.join(", ")
            ),
        }
    }
}

impl std::error::Error for PluginError {}

/// Enabled/disabled status snapshot for registry consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginStatus {
    pub metadata: PluginMetadata,
    pub enabled: bool,
    /// Whether this plugin's manifest constraints are satisfied.
    pub conflict: ConflictStatus,
    /// EQ slash command emitted at load time from manifest, if any.
    pub pause_command: Option<String>,
}

struct RegisteredPlugin {
    plugin: Box<dyn TextQuestPlugin>,
    metadata: PluginMetadata,
    enabled: bool,
    /// Recorded at load time so `status()` can report conflict resolution later.
    conflict: ConflictStatus,
    /// Slash command recorded from manifest.pause_on_load at load time.
    pause_command: Option<String>,
}

/// Runtime registry for plugin lifecycle and capability lookup.
#[derive(Default)]
pub struct PluginRegistry {
    plugins: BTreeMap<String, RegisteredPlugin>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_factory(&mut self, factory: PluginFactory) -> PluginResult<PluginStatus> {
        self.register(factory())
    }

    pub fn register(&mut self, mut plugin: Box<dyn TextQuestPlugin>) -> PluginResult<PluginStatus> {
        let mut metadata = plugin.metadata();
        validate_metadata(&metadata)?;

        if self.plugins.contains_key(&metadata.name) {
            return Err(PluginError::DuplicatePlugin(metadata.name));
        }

        // Enforce requires — all named plugins must be present and enabled.
        if !metadata.manifest.requires.is_empty() {
            let missing: Vec<String> = metadata
                .manifest
                .requires
                .iter()
                .filter(|dep| {
                    !self
                        .plugins
                        .get(dep.as_str())
                        .map(|r| r.enabled)
                        .unwrap_or(false)
                })
                .cloned()
                .collect();
            if !missing.is_empty() {
                return Err(PluginError::UnsatisfiedRequirement {
                    plugin: metadata.name.clone(),
                    missing,
                });
            }
        }

        // Enforce force_unload — disable conflicting plugins before loading.
        if !metadata.manifest.force_unload.is_empty() {
            let mut failed: Vec<String> = Vec::new();
            for conflict in &metadata.manifest.force_unload.clone() {
                if let Some(registered) = self.plugins.get_mut(conflict.as_str())
                    && registered.enabled {
                        match registered.plugin.on_unload() {
                            Ok(()) => registered.enabled = false,
                            Err(_) => failed.push(conflict.clone()),
                        }
                    }
            }
            if !failed.is_empty() {
                return Err(PluginError::ConflictingPlugin {
                    plugin: metadata.name.clone(),
                    conflicts: failed,
                });
            }
        }

        let pause_command = metadata.manifest.pause_on_load.clone();

        let mut context = PluginContext::default();
        plugin.on_load(&mut context)?;
        for capability in context.into_capabilities() {
            metadata.add_capability(capability);
        }

        let status = PluginStatus {
            metadata: metadata.clone(),
            enabled: true,
            conflict: ConflictStatus::Ok,
            pause_command,
        };
        self.plugins.insert(
            metadata.name.clone(),
            RegisteredPlugin {
                plugin,
                metadata,
                enabled: true,
                conflict: ConflictStatus::Ok,
                pause_command: status.pause_command.clone(),
            },
        );
        Ok(status)
    }

    pub fn enable(&mut self, name: &str) -> PluginResult<PluginStatus> {
        let registered = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| PluginError::NotFound(name.to_string()))?;

        if !registered.enabled {
            let mut context = PluginContext::default();
            registered.plugin.on_load(&mut context)?;
            for capability in context.into_capabilities() {
                registered.metadata.add_capability(capability);
            }
            registered.enabled = true;
        }

        Ok(registered.status())
    }

    pub fn disable(&mut self, name: &str) -> PluginResult<PluginStatus> {
        let registered = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| PluginError::NotFound(name.to_string()))?;

        if registered.enabled {
            registered.plugin.on_unload()?;
            registered.enabled = false;
        }

        Ok(registered.status())
    }

    pub fn status(&self, name: &str) -> Option<PluginStatus> {
        self.plugins.get(name).map(RegisteredPlugin::status)
    }

    pub fn statuses(&self) -> Vec<PluginStatus> {
        self.plugins
            .values()
            .map(RegisteredPlugin::status)
            .collect()
    }

    pub fn capabilities(&self, domain: PluginDomain) -> Vec<PluginCapability> {
        self.plugins
            .values()
            .filter(|plugin| plugin.enabled)
            .flat_map(|plugin| plugin.metadata.capabilities.iter())
            .filter(|capability| capability.domain == domain)
            .cloned()
            .collect()
    }
}

impl RegisteredPlugin {
    fn status(&self) -> PluginStatus {
        PluginStatus {
            metadata: self.metadata.clone(),
            enabled: self.enabled,
            conflict: self.conflict.clone(),
            pause_command: self.pause_command.clone(),
        }
    }
}

fn validate_metadata(metadata: &PluginMetadata) -> PluginResult<()> {
    if metadata.name.trim().is_empty() {
        return Err(PluginError::EmptyName);
    }
    Ok(())
}

// ─── MQ Actors mailbox interop ────────────────────────────────────────────────

/// Peer address in the MQ Actors addressing scheme: `<server>/<character>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorAddress {
    pub server: String,
    pub character: String,
}

impl ActorAddress {
    pub fn new(server: impl Into<String>, character: impl Into<String>) -> Self {
        Self {
            server: server.into(),
            character: character.into(),
        }
    }
}

impl fmt::Display for ActorAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.server, self.character)
    }
}

/// An MQ Actors-compatible message with JSON payload.
///
/// Payload format mirrors the Lua table serialization used by MQ2 Actors so
/// that rgmercs peer messages (corpse drag, comms heartbeats) can round-trip
/// through TextQuest's IPC without re-encoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorsMessage {
    pub from: ActorAddress,
    pub to: ActorAddress,
    /// JSON-encoded payload (MQ2 Actors Lua table format).
    pub payload: String,
}

impl ActorsMessage {
    pub fn new(from: ActorAddress, to: ActorAddress, payload: impl Into<String>) -> Self {
        Self {
            from,
            to,
            payload: payload.into(),
        }
    }
}

/// Trait for MQ Actors-compatible mailbox endpoints.
///
/// Implement this to let TextQuest components participate in rgmercs' peer
/// messaging protocol (corpse drag, comms heartbeats, etc.).
pub trait ActorsMailbox: Send + Sync {
    fn send(&self, message: ActorsMessage) -> PluginResult<()>;
    fn drain(&self) -> Vec<ActorsMessage>;
    fn address(&self) -> &ActorAddress;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct DemoInventoryPlugin;

    impl TextQuestPlugin for DemoInventoryPlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new("demo-inventory", "0.1.0", "demo")
        }

        fn on_load(&mut self, context: &mut PluginContext) -> PluginResult<()> {
            context.register_capability(PluginCapability::new(
                PluginDomain::Inventory,
                "inventory.demo",
                "demo inventory hook",
            ));
            Ok(())
        }
    }

    fn demo_plugin() -> Box<dyn TextQuestPlugin> {
        Box::new(DemoInventoryPlugin)
    }

    // A plugin that requires demo-inventory to be loaded first.
    struct DependentPlugin;

    impl TextQuestPlugin for DependentPlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new("dependent", "0.1.0", "needs demo-inventory").with_manifest(
                PluginManifest {
                    requires: vec!["demo-inventory".to_string()],
                    ..Default::default()
                },
            )
        }
    }

    // A plugin that force-unloads demo-inventory (simulates rgmercs unloading MQ2Melee).
    struct ConflictingPlugin;

    impl TextQuestPlugin for ConflictingPlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new("conflicting", "0.1.0", "unloads demo-inventory").with_manifest(
                PluginManifest {
                    force_unload: vec!["demo-inventory".to_string()],
                    pause_on_load: Some("/enc pause on".to_string()),
                    ..Default::default()
                },
            )
        }
    }

    #[test]
    fn registry_loads_plugin_and_indexes_capabilities() {
        let mut registry = PluginRegistry::new();

        let status = registry.register_factory(demo_plugin).unwrap();

        assert_eq!(status.metadata.name, "demo-inventory");
        assert!(status.enabled);
        assert_eq!(registry.capabilities(PluginDomain::Inventory).len(), 1);
    }

    #[test]
    fn disabled_plugin_capabilities_are_hidden() {
        let mut registry = PluginRegistry::new();
        registry.register_factory(demo_plugin).unwrap();

        let status = registry.disable("demo-inventory").unwrap();

        assert!(!status.enabled);
        assert!(registry.capabilities(PluginDomain::Inventory).is_empty());
    }

    #[test]
    fn requires_satisfied_when_dependency_already_loaded() {
        let mut registry = PluginRegistry::new();
        registry.register_factory(demo_plugin).unwrap();

        let result = registry.register(Box::new(DependentPlugin));

        assert!(result.is_ok(), "dependent plugin should load: {result:?}");
    }

    #[test]
    fn requires_fails_when_dependency_absent() {
        let mut registry = PluginRegistry::new();

        let result = registry.register(Box::new(DependentPlugin));

        assert!(matches!(
            result,
            Err(PluginError::UnsatisfiedRequirement { .. })
        ));
    }

    #[test]
    fn force_unload_disables_conflicting_plugin_before_load() {
        let mut registry = PluginRegistry::new();
        registry.register_factory(demo_plugin).unwrap();
        assert!(registry.status("demo-inventory").unwrap().enabled);

        let result = registry.register(Box::new(ConflictingPlugin));

        assert!(result.is_ok(), "conflicting plugin should load: {result:?}");
        // demo-inventory should now be disabled
        assert!(!registry.status("demo-inventory").unwrap().enabled);
    }

    #[test]
    fn force_unload_plugin_exposes_pause_command() {
        let mut registry = PluginRegistry::new();
        registry.register_factory(demo_plugin).unwrap();

        let status = registry.register(Box::new(ConflictingPlugin)).unwrap();

        assert_eq!(status.pause_command.as_deref(), Some("/enc pause on"));
    }

    #[test]
    fn conflict_status_defaults_to_ok() {
        let mut registry = PluginRegistry::new();
        let status = registry.register_factory(demo_plugin).unwrap();
        assert_eq!(status.conflict, ConflictStatus::Ok);
    }
}
