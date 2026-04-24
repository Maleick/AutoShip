//! Example non-core plugin that registers an inventory capability.

use textquest_common::plugins::{
    PluginCapability, PluginContext, PluginDomain, PluginMetadata, PluginResult, TextQuestPlugin,
};

#[derive(Default)]
pub struct ExampleInventoryPlugin;

impl TextQuestPlugin for ExampleInventoryPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new(
            "example-inventory-plugin",
            env!("CARGO_PKG_VERSION"),
            "Demonstrates non-core inventory extension registration.",
        )
    }

    fn on_load(&mut self, context: &mut PluginContext) -> PluginResult<()> {
        context.register_capability(PluginCapability::new(
            PluginDomain::Inventory,
            "inventory.audit",
            "Registers an inventory audit extension point.",
        ));
        Ok(())
    }
}

pub fn plugin() -> Box<dyn TextQuestPlugin> {
    Box::new(ExampleInventoryPlugin)
}
