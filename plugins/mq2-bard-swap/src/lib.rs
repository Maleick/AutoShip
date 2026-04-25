//! MQ2BardSwap equivalent — automatic bard instrument swapping for twist songs.
//!
//! Handles: instrument slot management, twist timing, song queueing.
//! Note: MQ2Twist is force-unloaded by rgmercs; this plugin activates for non-rgmercs configs.

use textquest_common::plugins::{
    PluginCapability, PluginContext, PluginDomain, PluginMetadata, PluginResult, TextQuestPlugin,
};

#[derive(Default)]
pub struct MQ2BardSwapPlugin;

impl TextQuestPlugin for MQ2BardSwapPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new(
            "mq2-bard-swap",
            env!("CARGO_PKG_VERSION"),
            "Bard instrument swapping for twist song optimization with timing.",
        )
    }

    fn on_load(&mut self, context: &mut PluginContext) -> PluginResult<()> {
        context.register_capability(PluginCapability::new(
            PluginDomain::Combat,
            "combat.bard_twist",
            "Manages bard twist song rotation and instrument swapping.",
        ));
        context.register_capability(PluginCapability::new(
            PluginDomain::Combat,
            "combat.instrument_swap",
            "Swaps bard instruments on configurable timing.",
        ));
        Ok(())
    }
}

pub fn plugin() -> Box<dyn TextQuestPlugin> {
    Box::new(MQ2BardSwapPlugin)
}
