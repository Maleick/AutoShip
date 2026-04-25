//! MQ2Melee equivalent — melee combat automation with discipline firing.
//!
//! Handles: discipline priority list, cooldown tracking, combat positioning.
//! Note: MQ2Melee is force-unloaded by rgmercs; this plugin activates for non-rgmercs configs.

use textquest_common::plugins::{
    PluginCapability, PluginContext, PluginDomain, PluginMetadata, PluginResult, TextQuestPlugin,
};

#[derive(Default)]
pub struct MQ2MeleePlugin;

impl TextQuestPlugin for MQ2MeleePlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new(
            "mq2-melee",
            env!("CARGO_PKG_VERSION"),
            "Melee combat automation with discipline priority firing and cooldown tracking.",
        )
    }

    fn on_load(&mut self, context: &mut PluginContext) -> PluginResult<()> {
        context.register_capability(PluginCapability::new(
            PluginDomain::Combat,
            "combat.disc_fire",
            "Fires combat disciplines on cooldown with configurable priority list.",
        ));
        context.register_capability(PluginCapability::new(
            PluginDomain::Combat,
            "combat.proc_track",
            "Tracks proc status and combat procs.",
        ));
        context.register_capability(PluginCapability::new(
            PluginDomain::Combat,
            "combat.position",
            "Manages combat positioning and melee stance.",
        ));
        Ok(())
    }
}

pub fn plugin() -> Box<dyn TextQuestPlugin> {
    Box::new(MQ2MeleePlugin)
}
