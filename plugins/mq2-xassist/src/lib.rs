//! MQ2XAssist equivalent — cross-client assist targeting.
//!
//! Handles: MA target tracking, client coordination, assist command propagation.

use textquest_common::plugins::{
    PluginCapability, PluginContext, PluginDomain, PluginMetadata, PluginResult, TextQuestPlugin,
};

#[derive(Default)]
pub struct MQ2XAssistPlugin;

impl TextQuestPlugin for MQ2XAssistPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new(
            "mq2-xassist",
            env!("CARGO_PKG_VERSION"),
            "Cross-client assist targeting — all clients attack the main assist target.",
        )
    }

    fn on_load(&mut self, context: &mut PluginContext) -> PluginResult<()> {
        context.register_capability(PluginCapability::new(
            PluginDomain::Combat,
            "combat.xassist",
            "Coordinates assist target across all group clients.",
        ));
        context.register_capability(PluginCapability::new(
            PluginDomain::Combat,
            "combat.ma_track",
            "Tracks main assist target and broadcasts to group.",
        ));
        Ok(())
    }
}

pub fn plugin() -> Box<dyn TextQuestPlugin> {
    Box::new(MQ2XAssistPlugin)
}
