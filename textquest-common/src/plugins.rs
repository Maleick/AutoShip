//! Plugin API contracts shared by TextQuest runtimes and external extensions.
//!
//! This module intentionally starts with an in-process registry and Rust trait
//! contract. Dynamic library discovery can layer on top of the `PluginFactory`
//! entry point once the ABI boundary is finalized.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

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
        }
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
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "plugin name must not be empty"),
            Self::DuplicatePlugin(name) => write!(f, "plugin already registered: {name}"),
            Self::NotFound(name) => write!(f, "plugin not found: {name}"),
            Self::Lifecycle(message) => write!(f, "plugin lifecycle failed: {message}"),
        }
    }
}

impl std::error::Error for PluginError {}

/// Enabled/disabled status snapshot for registry consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginStatus {
    pub metadata: PluginMetadata,
    pub enabled: bool,
}

struct RegisteredPlugin {
    plugin: Box<dyn TextQuestPlugin>,
    metadata: PluginMetadata,
    enabled: bool,
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

        let mut context = PluginContext::default();
        plugin.on_load(&mut context)?;
        for capability in context.into_capabilities() {
            metadata.add_capability(capability);
        }

        let status = PluginStatus {
            metadata: metadata.clone(),
            enabled: true,
        };
        self.plugins.insert(
            metadata.name.clone(),
            RegisteredPlugin {
                plugin,
                metadata,
                enabled: true,
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
        }
    }
}

fn validate_metadata(metadata: &PluginMetadata) -> PluginResult<()> {
    if metadata.name.trim().is_empty() {
        return Err(PluginError::EmptyName);
    }
    Ok(())
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
}
