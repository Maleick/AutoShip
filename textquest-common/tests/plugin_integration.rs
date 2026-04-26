//! Integration tests for the plugin system.
//!
//! These tests verify end-to-end plugin loading, execution, and isolation.

use textquest_common::plugins::{
    PluginCapability, PluginContext, PluginDomain, PluginError, PluginManifest, PluginMetadata,
    PluginRegistry, PluginResult, TextQuestPlugin,
};

/// Helper plugin factory for testing.
struct TestPlugin {
    name: String,
    version: String,
    on_load_fails: bool,
    capabilities_to_register: Vec<PluginCapability>,
    manifest: PluginManifest,
}

impl TestPlugin {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            on_load_fails: false,
            capabilities_to_register: vec![],
            manifest: PluginManifest::default(),
        }
    }

    fn with_manifest(mut self, manifest: PluginManifest) -> Self {
        self.manifest = manifest;
        self
    }

    fn with_capability(mut self, cap: PluginCapability) -> Self {
        self.capabilities_to_register.push(cap);
        self
    }

    fn fail_on_load(mut self) -> Self {
        self.on_load_fails = true;
        self
    }
}

impl TextQuestPlugin for TestPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new(&self.name, &self.version, "test plugin")
            .with_manifest(self.manifest.clone())
    }

    fn on_load(&mut self, context: &mut PluginContext) -> PluginResult<()> {
        if self.on_load_fails {
            return Err(PluginError::Lifecycle(format!(
                "Intentional failure for {}: {}",
                self.name, "testing error isolation"
            )));
        }

        for cap in &self.capabilities_to_register {
            context.register_capability(cap.clone());
        }

        Ok(())
    }
}

#[test]
fn integration_load_and_execute_plugin() {
    let mut registry = PluginRegistry::new();

    let plugin = TestPlugin::new("test-basic-load");
    let result = registry.register(Box::new(plugin));

    assert!(result.is_ok());
    let status = result.unwrap();
    assert_eq!(status.metadata.name, "test-basic-load");
    assert!(status.enabled);
}

#[test]
fn integration_plugin_api_calls_work() {
    let mut registry = PluginRegistry::new();

    let plugin = TestPlugin::new("test-api-calls")
        .with_capability(PluginCapability::new(
            PluginDomain::Combat,
            "combat.rotation",
            "Test rotation",
        ))
        .with_capability(PluginCapability::new(
            PluginDomain::Inventory,
            "inventory.audit",
            "Test audit",
        ));

    registry.register(Box::new(plugin)).unwrap();

    let combat_caps = registry.capabilities(PluginDomain::Combat);
    assert_eq!(combat_caps.len(), 1);
    assert_eq!(combat_caps[0].name, "combat.rotation");

    let inventory_caps = registry.capabilities(PluginDomain::Inventory);
    assert_eq!(inventory_caps.len(), 1);
    assert_eq!(inventory_caps[0].name, "inventory.audit");

    let nav_caps = registry.capabilities(PluginDomain::Navigation);
    assert_eq!(nav_caps.len(), 0);
}

#[test]
fn integration_plugin_unload_cleanup() {
    let mut registry = PluginRegistry::new();

    let plugin = TestPlugin::new("test-cleanup").with_capability(PluginCapability::new(
        PluginDomain::System,
        "system.cleanup",
        "Test cleanup hook",
    ));

    registry.register(Box::new(plugin)).unwrap();

    // Verify capabilities are present before unload
    let caps_before = registry.capabilities(PluginDomain::System);
    assert_eq!(caps_before.len(), 1);

    // Disable the plugin
    let status = registry.disable("test-cleanup").unwrap();
    assert!(!status.enabled);

    // Verify capabilities are removed after unload
    let caps_after = registry.capabilities(PluginDomain::System);
    assert_eq!(caps_after.len(), 0);
}

#[test]
fn integration_error_isolation_prevents_registry_corruption() {
    let mut registry = PluginRegistry::new();

    // Load a good plugin first
    let good_plugin = TestPlugin::new("good-plugin").with_capability(PluginCapability::new(
        PluginDomain::Combat,
        "combat.good",
        "Good capability",
    ));
    registry.register(Box::new(good_plugin)).unwrap();

    // Try to load a plugin that will fail
    let bad_plugin = TestPlugin::new("bad-plugin").fail_on_load();
    let result = registry.register(Box::new(bad_plugin));

    // The bad plugin should fail to load
    assert!(matches!(result, Err(PluginError::Lifecycle(_))));

    // Good plugin should still be available
    assert!(registry.status("good-plugin").is_some());
    let caps = registry.capabilities(PluginDomain::Combat);
    assert_eq!(caps.len(), 1);
    assert_eq!(caps[0].name, "combat.good");
}

#[test]
fn integration_plugin_dependencies_enforced() {
    let mut registry = PluginRegistry::new();

    // Try to load a plugin that requires a missing dependency
    let dependent = TestPlugin::new("dependent").with_manifest(PluginManifest {
        requires: vec!["missing-plugin".to_string()],
        ..Default::default()
    });

    let result = registry.register(Box::new(dependent));

    assert!(matches!(
        result,
        Err(PluginError::UnsatisfiedRequirement { .. })
    ));
}

#[test]
fn integration_plugin_conflicts_resolved() {
    let mut registry = PluginRegistry::new();

    // Load first plugin
    let plugin1 = TestPlugin::new("plugin1");
    registry.register(Box::new(plugin1)).unwrap();
    assert!(registry.status("plugin1").unwrap().enabled);

    // Load conflicting plugin that force_unloads plugin1
    let plugin2 = TestPlugin::new("plugin2").with_manifest(PluginManifest {
        force_unload: vec!["plugin1".to_string()],
        ..Default::default()
    });

    registry.register(Box::new(plugin2)).unwrap();

    // plugin1 should now be disabled
    assert!(!registry.status("plugin1").unwrap().enabled);
    // plugin2 should be enabled
    assert!(registry.status("plugin2").unwrap().enabled);
}

#[test]
fn integration_multiple_capabilities_per_domain() {
    let mut registry = PluginRegistry::new();

    let plugin1 = TestPlugin::new("multi1")
        .with_capability(PluginCapability::new(PluginDomain::Combat, "combat.a", "A"))
        .with_capability(PluginCapability::new(PluginDomain::Combat, "combat.b", "B"));

    let plugin2 = TestPlugin::new("multi2").with_capability(PluginCapability::new(
        PluginDomain::Combat,
        "combat.c",
        "C",
    ));

    registry.register(Box::new(plugin1)).unwrap();
    registry.register(Box::new(plugin2)).unwrap();

    let caps = registry.capabilities(PluginDomain::Combat);
    assert_eq!(caps.len(), 3);
    assert!(caps.iter().any(|c| c.name == "combat.a"));
    assert!(caps.iter().any(|c| c.name == "combat.b"));
    assert!(caps.iter().any(|c| c.name == "combat.c"));
}

#[test]
fn integration_reenable_after_disable() {
    let mut registry = PluginRegistry::new();

    let plugin = TestPlugin::new("toggle").with_capability(PluginCapability::new(
        PluginDomain::Inventory,
        "inventory.test",
        "Test",
    ));

    registry.register(Box::new(plugin)).unwrap();

    // Disable
    registry.disable("toggle").unwrap();
    assert!(!registry.status("toggle").unwrap().enabled);
    assert_eq!(registry.capabilities(PluginDomain::Inventory).len(), 0);

    // Re-enable
    registry.enable("toggle").unwrap();
    assert!(registry.status("toggle").unwrap().enabled);
    assert_eq!(registry.capabilities(PluginDomain::Inventory).len(), 1);
}
