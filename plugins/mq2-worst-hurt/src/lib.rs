//! MQ2WorstHurt equivalent — automatic healer target selection based on damage.
//!
//! Handles: group member HP tracking, worst-hurt detection, heal assist targeting.

use textquest_common::plugins::{
    PluginCapability, PluginContext, PluginDomain, PluginMetadata, PluginResult, TextQuestPlugin,
};

#[derive(Default)]
pub struct MQ2WorstHurtPlugin;

impl TextQuestPlugin for MQ2WorstHurtPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new(
            "mq2-worst-hurt",
            env!("CARGO_PKG_VERSION"),
            "Healer assist target selection — targets most damaged group member.",
        )
    }

    fn on_load(&mut self, context: &mut PluginContext) -> PluginResult<()> {
        context.register_capability(PluginCapability::new(
            PluginDomain::Combat,
            "combat.heal_assist",
            "Automatically targets worst-hurt group member for healing.",
        ));
        context.register_capability(PluginCapability::new(
            PluginDomain::Combat,
            "combat.hp_track",
            "Tracks group member HP for heal targeting.",
        ));
        Ok(())
    }
}

pub fn plugin() -> Box<dyn TextQuestPlugin> {
    Box::new(MQ2WorstHurtPlugin)
}
