//! Test plugin that demonstrates TextQuest API usage and capability registration.

use textquest_common::plugins::{
    PluginCapability, PluginContext, PluginDomain, PluginMetadata, PluginResult, TextQuestPlugin,
};

#[derive(Default)]
pub struct ApiTestPlugin {
    loaded: bool,
}

impl TextQuestPlugin for ApiTestPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new(
            "test-api-plugin",
            "0.1.0",
            "Test plugin that uses the TextQuest plugin API.",
        )
    }

    fn on_load(&mut self, context: &mut PluginContext) -> PluginResult<()> {
        // Register multiple capabilities across different domains
        context.register_capability(PluginCapability::new(
            PluginDomain::Combat,
            "combat.test-rotation",
            "Test combat rotation capability",
        ));
        context.register_capability(PluginCapability::new(
            PluginDomain::Navigation,
            "navigation.test-pathfinding",
            "Test pathfinding capability",
        ));
        context.register_capability(PluginCapability::new(
            PluginDomain::Inventory,
            "inventory.test-loot",
            "Test loot management capability",
        ));

        self.loaded = true;
        eprintln!("[test-api-plugin] loaded and registered 3 capabilities");
        Ok(())
    }

    fn on_unload(&mut self) -> PluginResult<()> {
        if self.loaded {
            eprintln!("[test-api-plugin] unloaded");
            self.loaded = false;
        }
        Ok(())
    }
}

pub fn plugin() -> Box<dyn TextQuestPlugin> {
    Box::new(ApiTestPlugin::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_plugin_registers_multiple_capabilities() {
        let mut plugin = ApiTestPlugin::default();
        let mut context = PluginContext::default();

        plugin.on_load(&mut context).unwrap();

        let capabilities = context.capabilities();
        assert_eq!(capabilities.len(), 3);
        assert!(capabilities.iter().any(|c| c.domain == PluginDomain::Combat));
        assert!(capabilities.iter().any(|c| c.domain == PluginDomain::Navigation));
        assert!(capabilities.iter().any(|c| c.domain == PluginDomain::Inventory));
    }

    #[test]
    fn api_plugin_tracks_loaded_state() {
        let mut plugin = ApiTestPlugin::default();
        assert!(!plugin.loaded);

        let mut context = PluginContext::default();
        plugin.on_load(&mut context).unwrap();
        assert!(plugin.loaded);

        plugin.on_unload().unwrap();
        assert!(!plugin.loaded);
    }
}
