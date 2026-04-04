use dmft_common::combat::{CombatRole, SpellEntry};
use crate::combat::strategy::{ClassStrategy, CombatContext};
use crate::combat::twist::{SongSlot, TwistAction, TwistEngine};

pub struct BardStrategy { class_id: u8, twist: TwistEngine, melody_fallback_active: bool, tick: u32 }

impl BardStrategy {
    pub fn new(class_id: u8) -> Self { Self { class_id, twist: TwistEngine::new(vec![]), melody_fallback_active: false, tick: 0 } }
    pub fn with_twist(class_id: u8, songs: Vec<SongSlot>) -> Self { Self { class_id, twist: TwistEngine::new(songs), melody_fallback_active: false, tick: 0 } }
    fn melody_command(spells: &[SpellEntry]) -> String { if spells.is_empty() { return "/melody".into(); } format!("/melody {}", spells.iter().map(|s| s.slot.to_string()).collect::<Vec<_>>().join(" ")) }
    fn spells_to_songs(spells: &[SpellEntry]) -> Vec<SongSlot> { spells.iter().map(|s| SongSlot { gem: s.slot, priority: s.priority, min_recast_ticks: crate::combat::twist::DEFAULT_TWIST_DELAY_TICKS }).collect() }
    pub fn is_twisting(&self) -> bool { self.twist.is_active() }
    pub fn hold_song(&mut self, gem: u8) { self.twist.hold(gem); }
    pub fn release_hold(&mut self) { self.twist.release_hold(); }
}

impl ClassStrategy for BardStrategy {
    fn class_id(&self) -> u8 { self.class_id }
    fn select_target(&self, ctx: &CombatContext) -> Option<u32> { if ctx.in_combat { ctx.target.map(|t| t.spawn_id) } else { None } }
    fn select_spell(&self, _ctx: &CombatContext) -> Option<SpellEntry> { None }
    fn should_assist(&self, _ctx: &CombatContext) -> bool { true }
    fn on_engage(&mut self, ctx: &CombatContext) {
        self.tick = ctx.tick;
        if !ctx.config.spells.is_empty() {
            let songs = Self::spells_to_songs(&ctx.config.spells);
            if songs.len() >= 2 { self.twist.set_songs(songs); self.twist.start(); if let TwistAction::Cast { gem } = self.twist.tick(ctx.tick) { crate::eq::slash_command(&format!("/cast {gem}")); } return; }
        }
        if ctx.config.spells.is_empty() { return; }
        crate::eq::slash_command(&Self::melody_command(&ctx.config.spells));
        self.melody_fallback_active = true;
    }
    fn on_action_complete(&mut self, ctx: &CombatContext) {
        self.tick = ctx.tick;
        if !ctx.in_combat { if self.twist.is_active() { self.twist.stop(); } if self.melody_fallback_active { crate::eq::slash_command("/melody"); self.melody_fallback_active = false; } return; }
        if self.twist.is_active() { if let TwistAction::Cast { gem } = self.twist.tick(ctx.tick) { crate::eq::slash_command(&format!("/cast {gem}")); } }
    }
    fn aoe_threshold(&self) -> u8 { 3 }
    fn role(&self) -> CombatRole { CombatRole::Support }
}

#[cfg(test)]
mod tests {
    use super::*; use dmft_common::types::SpawnData;
    fn sp(id: i32, n: &str, sl: u8) -> SpellEntry { SpellEntry { slot: sl, spell_id: id, name: n.into(), min_mana_pct: 0.0, priority: 1, is_aoe: false } }
    fn cx<'a>(p: &'a SpawnData, c: &'a dmft_common::combat::CombatConfig, ic: bool, t: u32) -> CombatContext<'a> { CombatContext { player: p, target: None, nearby_enemies: &[], group_members: &[], config: c, tick: t, in_combat: ic, ch_chain_slot: None } }
    #[test] fn melody_cmd() { assert_eq!(BardStrategy::melody_command(&[sp(1,"A",1),sp(2,"B",3)]), "/melody 1 3"); }
    #[test] fn melody_empty() { assert_eq!(BardStrategy::melody_command(&[]), "/melody"); }
    #[test] fn spell_none() { assert!(BardStrategy::new(8).select_spell(&cx(&SpawnData::default(), &dmft_common::combat::CombatConfig::default(), true, 0)).is_none()); }
    #[test] fn role() { assert_eq!(BardStrategy::new(8).role(), CombatRole::Support); }
    #[test] fn id() { assert_eq!(BardStrategy::new(8).class_id(), 8); }
    #[test] fn engage_twist() { let mut b = BardStrategy::new(8); let p = SpawnData::default(); let c = dmft_common::combat::CombatConfig { spells: vec![sp(1,"A",1),sp(2,"B",2)], ..Default::default() }; b.on_engage(&cx(&p,&c,true,0)); assert!(b.is_twisting()); assert!(!b.melody_fallback_active); }
    #[test] fn engage_melody() { let mut b = BardStrategy::new(8); let p = SpawnData::default(); let c = dmft_common::combat::CombatConfig { spells: vec![sp(1,"A",1)], ..Default::default() }; b.on_engage(&cx(&p,&c,true,0)); assert!(!b.is_twisting()); assert!(b.melody_fallback_active); }
    #[test] fn engage_empty() { let mut b = BardStrategy::new(8); let p = SpawnData::default(); let c = dmft_common::combat::CombatConfig::default(); b.on_engage(&cx(&p,&c,true,0)); assert!(!b.is_twisting()); assert!(!b.melody_fallback_active); }
    #[test] fn disengage_twist() { let mut b = BardStrategy::new(8); let p = SpawnData::default(); let c = dmft_common::combat::CombatConfig { spells: vec![sp(1,"A",1),sp(2,"B",2)], ..Default::default() }; b.on_engage(&cx(&p,&c,true,0)); b.on_action_complete(&cx(&p,&c,false,50)); assert!(!b.is_twisting()); }
    #[test] fn disengage_melody() { let mut b = BardStrategy::new(8); let p = SpawnData::default(); let c = dmft_common::combat::CombatConfig { spells: vec![sp(1,"A",1)], ..Default::default() }; b.on_engage(&cx(&p,&c,true,0)); b.on_action_complete(&cx(&p,&c,false,50)); assert!(!b.melody_fallback_active); }
    #[test] fn combat_keeps() { let mut b = BardStrategy::new(8); let p = SpawnData::default(); let c = dmft_common::combat::CombatConfig { spells: vec![sp(1,"A",1),sp(2,"B",2)], ..Default::default() }; b.on_engage(&cx(&p,&c,true,0)); b.on_action_complete(&cx(&p,&c,true,50)); assert!(b.is_twisting()); }
    #[test] fn full_cycle() { let mut b = BardStrategy::new(8); let p = SpawnData::default(); let c = dmft_common::combat::CombatConfig { spells: vec![sp(1,"A",1),sp(2,"B",2)], ..Default::default() }; b.on_engage(&cx(&p,&c,true,0)); assert!(b.is_twisting()); b.on_action_complete(&cx(&p,&c,false,100)); assert!(!b.is_twisting()); b.on_engage(&cx(&p,&c,true,200)); assert!(b.is_twisting()); }
    #[test] fn preconfigured() { assert_eq!(BardStrategy::with_twist(8, vec![SongSlot{gem:0,priority:1,min_recast_ticks:66},SongSlot{gem:1,priority:2,min_recast_ticks:66}]).twist.song_count(), 2); }
    #[test] fn hold_rel() { let mut b = BardStrategy::new(8); let p = SpawnData::default(); let c = dmft_common::combat::CombatConfig { spells: vec![sp(1,"A",1),sp(2,"B",2)], ..Default::default() }; b.on_engage(&cx(&p,&c,true,0)); b.hold_song(5); b.release_hold(); assert!(b.is_twisting()); }
    #[test] fn conv() { let s = BardStrategy::spells_to_songs(&[sp(1,"A",1),sp(2,"B",3)]); assert_eq!(s.len(), 2); assert_eq!(s[0].gem, 1); assert_eq!(s[1].gem, 3); }
}
