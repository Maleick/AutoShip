//! Test plugin that demonstrates error handling and isolation.
//!
//! This plugin intentionally fails during on_load to verify that error
//! handling and plugin isolation work correctly in the registry.

use textquest_common::plugins::{PluginContext, PluginMetadata, PluginError, PluginResult, TextQuestPlugin};

#[derive(Default)]
pub struct ErrorPlugin {
    fail_on_load: bool,
}

impl ErrorPlugin {
    pub fn new(fail_on_load: bool) -> Self {
        Self { fail_on_load }
    }
}

impl TextQuestPlugin for ErrorPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new(
            "test-error-plugin",
            "0.1.0",
            "Test plugin that demonstrates error handling and isolation.",
        )
    }

    fn on_load(&mut self, _context: &mut PluginContext) -> PluginResult<()> {
        if self.fail_on_load {
            eprintln!("[test-error-plugin] intentionally failing on load");
            Err(PluginError::Lifecycle(
                "test error: intentional failure for isolation testing".to_string(),
            ))
        } else {
            eprintln!("[test-error-plugin] loaded successfully");
            Ok(())
        }
    }

    fn on_unload(&mut self) -> PluginResult<()> {
        eprintln!("[test-error-plugin] unloaded");
        Ok(())
    }
}

pub fn plugin() -> Box<dyn TextQuestPlugin> {
    Box::new(ErrorPlugin::new(false))
}

pub fn failing_plugin() -> Box<dyn TextQuestPlugin> {
    Box::new(ErrorPlugin::new(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_plugin_can_load_successfully() {
        let mut plugin = ErrorPlugin::new(false);
        let mut context = PluginContext::default();

        let result = plugin.on_load(&mut context);
        assert!(result.is_ok());
    }

    #[test]
    fn error_plugin_returns_lifecycle_error_when_configured() {
        let mut plugin = ErrorPlugin::new(true);
        let mut context = PluginContext::default();

        let result = plugin.on_load(&mut context);
        assert!(matches!(result, Err(PluginError::Lifecycle(_))));
    }

    #[test]
    fn error_plugin_can_unload() {
        let mut plugin = ErrorPlugin::new(false);
        let result = plugin.on_unload();
        assert!(result.is_ok());
    }
}
