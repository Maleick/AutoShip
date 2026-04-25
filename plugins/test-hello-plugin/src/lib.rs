//! Minimal test plugin that loads and unloads without errors.

use textquest_common::plugins::{PluginContext, PluginMetadata, PluginResult, TextQuestPlugin};

#[derive(Default)]
pub struct HelloPlugin;

impl TextQuestPlugin for HelloPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new(
            "test-hello-plugin",
            "0.1.0",
            "Minimal test plugin for plugin system verification.",
        )
    }

    fn on_load(&mut self, _context: &mut PluginContext) -> PluginResult<()> {
        eprintln!("[test-hello-plugin] loaded successfully");
        Ok(())
    }

    fn on_unload(&mut self) -> PluginResult<()> {
        eprintln!("[test-hello-plugin] unloaded successfully");
        Ok(())
    }
}

pub fn plugin() -> Box<dyn TextQuestPlugin> {
    Box::new(HelloPlugin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_plugin_has_valid_metadata() {
        let plugin = HelloPlugin;
        let metadata = plugin.metadata();

        assert_eq!(metadata.name, "test-hello-plugin");
        assert_eq!(metadata.version, "0.1.0");
        assert!(!metadata.description.is_empty());
    }

    #[test]
    fn hello_plugin_loads_and_unloads() {
        let mut plugin = HelloPlugin;
        let mut context = PluginContext::default();

        let load_result = plugin.on_load(&mut context);
        assert!(load_result.is_ok());

        let unload_result = plugin.on_unload();
        assert!(unload_result.is_ok());
    }
}
