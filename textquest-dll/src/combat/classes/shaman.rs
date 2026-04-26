use textquest_common::combat::{CastResult, CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext, PetAction};

const EMERGENCY_HP: f32 = 40.0;
const HEAL_HP: f32 = 70.0;
const STABLE_GROUP_HP: f32 = 80.0;
const CANNI_MANA_BELOW: f32 = 55.0;
const DOT_MANA_MIN: f32 = 60.0;
const DOT_TARGET_HP_MIN: f32 = 40.0;

const SLOW_COOLDOWN_TICKS: u32 = 60;
const MALO_COOLDOWN_TICKS: u32 = 60;
const HEAL_COOLDOWN_TICKS: u32 = 80;
const CANNI_COOLDOWN_TICKS: u32 = 240;
const DOT_COOLDOWN_TICKS: u32 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShamanSpellKind {
    Slow,
    Malo,
    Heal,
    Canni,
    Dot,
    Buff,
    Other,
}

/// Shaman strategy: hybrid healer/slower/DoT with explicit resist handling
/// and mana triage.
///
/// Priority order:
/// 0. Cure detrimental effects.
/// 1. Emergency heal below 40%.
/// 2. Land slow on each new target.
/// 3. If slow is resisted, cast Malo before retrying slow.
/// 4. Maintain moderate heals below 70%.
/// 5. Cannibalize only when the group is stable and mana is low.
/// 6. DoT only when the mob is already slowed and mana is comfortable.
///
/// EQ class ID: 10
pub struct ShamanStrategy {
    class_id: u8,
    /// Track if current target has been slowed (resets on target change)
    target_slowed: bool,
    /// Track whether the current target has already been magic-resist debuffed.
    target_maloed: bool,
    /// When a slow is resisted, force a Malo before retrying the slow.
    retry_malo_before_slow: bool,
    last_target_id: u32,
    last_slow_tick: Option<u32>,
    last_malo_tick: Option<u32>,
    last_heal_tick: Option<u32>,
    last_canni_tick: Option<u32>,
    last_dot_tick: Option<u32>,
}

impl ShamanStrategy {
    pub fn new(class_id: u8) -> Self {
        Self {
            class_id,
            target_slowed: false,
            target_maloed: false,
            retry_malo_before_slow: false,
            last_target_id: 0,
            last_slow_tick: None,
            last_malo_tick: None,
            last_heal_tick: None,
            last_canni_tick: None,
            last_dot_tick: None,
        }
    }

    fn afflicted_member(&self, ctx: &CombatContext) -> Option<u32> {
        strategy::prioritized_afflicted_member(ctx).map(|(spawn_id, _)| spawn_id)
    }

    fn find_cure_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();
        let cures: Vec<&SpellEntry> = ctx
            .config
            .spells
            .iter()
            .filter(|s| strategy::is_standard_cure_spell(s))
            .filter(|s| mana_pct >= s.min_mana_pct)
            .collect();
        if strategy::afflicted_member_count(ctx) > 1
            && let Some(group_cure) = cures
                .iter()
                .copied()
                .filter(|s| strategy::is_group_cure_spell(s))
                .max_by_key(|s| s.priority)
        {
            return Some(group_cure.clone());
        }
        cures.into_iter().max_by_key(|s| s.priority).cloned()
    }

    fn cure_target(&self, ctx: &CombatContext) -> Option<u32> {
        if strategy::afflicted_member_count(ctx) > 1
            && self
                .find_cure_spell(ctx)
                .is_some_and(|spell| strategy::is_group_cure_spell(&spell))
        {
            return Some(ctx.player.spawn_id);
        }
        self.afflicted_member(ctx)
    }

    fn classify_spell(spell: &SpellEntry) -> ShamanSpellKind {
        let name = spell.name.to_ascii_lowercase();

        if name.contains("canni") || name.contains("cannibal") {
            return ShamanSpellKind::Canni;
        }

        if name.contains("malo") || name.contains("malos") {
            return ShamanSpellKind::Malo;
        }

        if name.contains("slow")
            || name.contains("turgur")
            || name.contains("tigir")
            || name.contains("togor")
            || name.contains("sloth")
            || name.contains("deeds")
            || name.contains("sleep")
        {
            return ShamanSpellKind::Slow;
        }

        if name.contains("heal")
            || name.contains("mending")
            || name.contains("chloroblast")
            || name.contains("torpor")
            || name.contains("quiescence")
            || name.contains("replenishment")
            || name.contains("salve")
        {
            return ShamanSpellKind::Heal;
        }

        if strategy::is_standard_cure_spell(spell) {
            return ShamanSpellKind::Other;
        }

        if name.contains("avatar")
            || name.contains("focus")
            || name.contains("talisman")
            || name.contains("celerity")
            || name.contains("regrowth")
            || name.contains("spirit")
            || name.contains("form of")
            || name.contains("quickening")
        {
            return ShamanSpellKind::Buff;
        }

        if name.contains("scourge")
            || name.contains("envenom")
            || name.contains("plague")
            || name.contains("pox")
            || name.contains("venom")
            || name.contains("affliction")
            || name.contains("disease")
            || name.contains("breath")
            || name.contains("bane")
            || name.contains("grummus")
        {
            return ShamanSpellKind::Dot;
        }

        ShamanSpellKind::Other
    }

    fn highest_priority_spell_by_kind(
        &self,
        ctx: &CombatContext,
        kind: ShamanSpellKind,
    ) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();
        ctx.config
            .spells
            .iter()
            .filter(|spell| Self::classify_spell(spell) == kind)
            .filter(|spell| mana_pct >= spell.min_mana_pct)
            .max_by_key(|spell| spell.priority)
            .cloned()
    }

    fn cooldown_ready(last_tick: Option<u32>, cooldown_ticks: u32, now: u32) -> bool {
        last_tick.is_none_or(|last| now.saturating_sub(last) >= cooldown_ticks)
    }

    fn lowest_group_hp(&self, ctx: &CombatContext) -> Option<f32> {
        strategy::lowest_hp_member(ctx).map(|(_, hp)| hp)
    }

    fn group_is_stable(&self, ctx: &CombatContext) -> bool {
        self.lowest_group_hp(ctx)
            .is_none_or(|hp| hp >= STABLE_GROUP_HP)
    }

    fn should_cast_canni(&self, ctx: &CombatContext) -> bool {
        ctx.player.mana_pct() <= CANNI_MANA_BELOW
            && ctx.player.hp_pct() >= STABLE_GROUP_HP
            && self.group_is_stable(ctx)
    }

    fn should_cast_dot(&self, ctx: &CombatContext) -> bool {
        if ctx.player.mana_pct() < DOT_MANA_MIN || !self.target_slowed || !self.group_is_stable(ctx)
        {
            return false;
        }

        ctx.target
            .map(|target| target.hp_pct() > DOT_TARGET_HP_MIN)
            .unwrap_or(true)
    }

    fn select_friendly_target(&self, ctx: &CombatContext) -> Option<u32> {
        if let Some(cure_target) = self.cure_target(ctx)
            && self.find_cure_spell(ctx).is_some()
        {
            return Some(cure_target);
        }

        let (heal_target, hp) = strategy::lowest_hp_member(ctx)?;
        let emergency_heal_ready = hp < EMERGENCY_HP
            && self
                .highest_priority_spell_by_kind(ctx, ShamanSpellKind::Heal)
                .is_some();
        let moderate_heal_ready = hp < HEAL_HP
            && Self::cooldown_ready(self.last_heal_tick, HEAL_COOLDOWN_TICKS, ctx.tick)
            && self
                .highest_priority_spell_by_kind(ctx, ShamanSpellKind::Heal)
                .is_some();

        if emergency_heal_ready || moderate_heal_ready {
            return Some(heal_target);
        }

        None
    }

    fn resolved_spell<'a>(
        &self,
        ctx: &'a CombatContext,
        entry_name: Option<&str>,
        spell_id: i32,
    ) -> Option<&'a SpellEntry> {
        if spell_id > 0
            && let Some(spell) = ctx
                .config
                .spells
                .iter()
                .find(|spell| spell.spell_id == spell_id)
        {
            return Some(spell);
        }

        entry_name.and_then(|name| ctx.config.spells.iter().find(|spell| spell.name == name))
    }

    fn apply_spell_outcome(&mut self, tick: u32, spell: &SpellEntry, result: CastResult) {
        match Self::classify_spell(spell) {
            ShamanSpellKind::Slow => {
                self.last_slow_tick = Some(tick);
                if result.landed() {
                    self.target_slowed = true;
                    self.retry_malo_before_slow = false;
                } else if matches!(
                    result,
                    CastResult::Resisted | CastResult::Immune | CastResult::TakeHold
                ) {
                    self.target_slowed = false;
                    self.retry_malo_before_slow = true;
                }
            }
            ShamanSpellKind::Malo => {
                self.last_malo_tick = Some(tick);
                if result.landed() {
                    self.target_maloed = true;
                    self.retry_malo_before_slow = false;
                }
            }
            ShamanSpellKind::Heal => {
                if result.landed() {
                    self.last_heal_tick = Some(tick);
                }
            }
            ShamanSpellKind::Canni => {
                if result.landed() {
                    self.last_canni_tick = Some(tick);
                }
            }
            ShamanSpellKind::Dot => {
                if result.landed() {
                    self.last_dot_tick = Some(tick);
                }
            }
            ShamanSpellKind::Buff | ShamanSpellKind::Other => {}
        }
    }

    fn reset_target_state(&mut self) {
        self.target_slowed = false;
        self.target_maloed = false;
        self.retry_malo_before_slow = false;
        self.last_slow_tick = None;
        self.last_malo_tick = None;
        self.last_dot_tick = None;
    }
}

impl ClassStrategy for ShamanStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        self.select_friendly_target(ctx)
            .or_else(|| ctx.target.map(|target| target.spawn_id))
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();

        // Priority 0: Cure detrimental effects (shaman is the premier curer)
        if strategy::afflicted_member_count(ctx) > 0 {
            if let Some(cure) = self.find_cure_spell(ctx) {
                return Some(cure);
            }
        }

        // Priority 1: Emergency heal (group member below 40%)
        if let Some(hp) = self.lowest_group_hp(ctx)
            && hp < EMERGENCY_HP
        {
            return self.highest_priority_spell_by_kind(ctx, ShamanSpellKind::Heal);
        }

        // Priority 2: If slow resisted, land Malo before re-trying slow.
        if !self.target_slowed
            && self.retry_malo_before_slow
            && !self.target_maloed
            && Self::cooldown_ready(self.last_malo_tick, MALO_COOLDOWN_TICKS, ctx.tick)
            && let Some(malo) = self.highest_priority_spell_by_kind(ctx, ShamanSpellKind::Malo)
        {
            return Some(malo);
        }

        // Priority 3: Slow every fresh target once the retry window is open.
        if !self.target_slowed
            && Self::cooldown_ready(self.last_slow_tick, SLOW_COOLDOWN_TICKS, ctx.tick)
            && let Some(slow) = self.highest_priority_spell_by_kind(ctx, ShamanSpellKind::Slow)
        {
            return Some(slow);
        }

        // Priority 4: Moderate heal.
        if let Some(hp) = self.lowest_group_hp(ctx)
            && hp < HEAL_HP
            && Self::cooldown_ready(self.last_heal_tick, HEAL_COOLDOWN_TICKS, ctx.tick)
        {
            return self.highest_priority_spell_by_kind(ctx, ShamanSpellKind::Heal);
        }

        // Priority 5: Convert HP into mana only when the group is stable.
        if self.should_cast_canni(ctx)
            && Self::cooldown_ready(self.last_canni_tick, CANNI_COOLDOWN_TICKS, ctx.tick)
            && let Some(canni) = self.highest_priority_spell_by_kind(ctx, ShamanSpellKind::Canni)
        {
            return Some(canni);
        }

        // Priority 6: DoT only after slow is secure and mana is comfortable.
        if self.should_cast_dot(ctx)
            && Self::cooldown_ready(self.last_dot_tick, DOT_COOLDOWN_TICKS, ctx.tick)
        {
            return self.highest_priority_spell_by_kind(ctx, ShamanSpellKind::Dot);
        }

        // Priority 7: fall back only to uncategorized utility or clicky
        // spells. Slow, Malo, heals, Canni, and DoTs are all handled above
        // with explicit gates so they do not starve higher-value actions.
        ctx.config
            .spells
            .iter()
            .filter(|spell| matches!(Self::classify_spell(spell), ShamanSpellKind::Other))
            .filter(|spell| mana_pct >= spell.min_mana_pct)
            .max_by_key(|spell| spell.priority)
            .cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn pet_action(&self, ctx: &CombatContext) -> Option<PetAction> {
        strategy::pet_attack_action(ctx)
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        let new_target = ctx.target.map_or(0, |t| t.spawn_id);
        if new_target != self.last_target_id {
            self.reset_target_state();
            self.last_target_id = new_target;
        }
        if let Some(target) = ctx.target {
            tracing::info!(
                target_id = target.spawn_id,
                target_name = %target.name,
                slowed = self.target_slowed,
                maloed = self.target_maloed,
                "Shaman engaging"
            );
        }
    }

    fn on_cast_outcome(&mut self, ctx: &CombatContext, gem: u8, result: CastResult) {
        if matches!(
            result,
            CastResult::Interrupted | CastResult::Aborted | CastResult::Cancelled
        ) {
            self.on_cast_interrupted(ctx, gem);
        }
    }

    fn on_resolved_action_outcome(
        &mut self,
        ctx: &CombatContext,
        entry_name: Option<&str>,
        spell_id: i32,
        _target_id: u32,
        result: CastResult,
    ) {
        if let Some(spell) = self.resolved_spell(ctx, entry_name, spell_id) {
            self.apply_spell_outcome(ctx.tick, spell, result);
        }
    }

    fn on_action_complete(&mut self, ctx: &CombatContext) {
        // Only reset slow tracking when out of combat (target died / disengage).
        // During combat, on_engage handles new-target resets. Resetting here
        // unconditionally caused the shaman to re-cast slow every GCD cycle.
        if !ctx.in_combat {
            self.reset_target_state();
            self.last_target_id = 0;
        }
    }

    fn aoe_threshold(&self) -> u8 {
        3
    }

    fn role(&self) -> CombatRole {
        CombatRole::Healer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::strategy::GroupMemberState;
    use textquest_common::combat::{AssistMode, CastResult, CombatConfig};

    fn test_config_with_spells() -> CombatConfig {
        CombatConfig {
            role: CombatRole::Healer,
            pull_method: None,
            assist_mode: AssistMode::AssistTrain,
            mana_floor: 20.0,
            aoe_threshold: 3,
            spells: vec![
                SpellEntry {
                    slot: 1,
                    spell_id: 0,
                    name: "Turgur's Insects".into(),
                    min_mana_pct: 30.0,
                    priority: 10,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 2,
                    spell_id: 0,
                    name: "Greater Healing".into(),
                    min_mana_pct: 20.0,
                    priority: 8,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 3,
                    spell_id: 0,
                    name: "Envenomed Bolt".into(),
                    min_mana_pct: 25.0,
                    priority: 5,
                    is_aoe: false,
                },
            ],
            holyshit_rules: vec![],
            disciplines: vec![],
            target_scan: textquest_common::combat::TargetScanConfig::default(),
            cast_retry_policy: Default::default(),
        }
    }

    fn liveish_config_with_spells() -> CombatConfig {
        CombatConfig {
            role: CombatRole::Healer,
            pull_method: None,
            assist_mode: AssistMode::AssistTrain,
            mana_floor: 35.0,
            aoe_threshold: 3,
            spells: vec![
                SpellEntry {
                    slot: 1,
                    spell_id: 1001,
                    name: "Turgur's Insects".into(),
                    min_mana_pct: 35.0,
                    priority: 100,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 2,
                    spell_id: 1002,
                    name: "Malo".into(),
                    min_mana_pct: 25.0,
                    priority: 95,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 3,
                    spell_id: 1003,
                    name: "Kragg's Mending".into(),
                    min_mana_pct: 30.0,
                    priority: 90,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 4,
                    spell_id: 1004,
                    name: "Cannibalize IV".into(),
                    min_mana_pct: 0.0,
                    priority: 80,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 5,
                    spell_id: 1005,
                    name: "Ancient: Scourge of Nife".into(),
                    min_mana_pct: 60.0,
                    priority: 70,
                    is_aoe: false,
                },
            ],
            holyshit_rules: vec![],
            disciplines: vec![],
            target_scan: textquest_common::combat::TargetScanConfig::default(),
            cast_retry_policy: Default::default(),
        }
    }

    #[test]
    fn shaman_class_id() {
        let shaman = ShamanStrategy::new(10);
        assert_eq!(shaman.class_id(), 10);
    }

    #[test]
    fn shaman_role_is_healer() {
        let shaman = ShamanStrategy::new(10);
        assert_eq!(shaman.role(), CombatRole::Healer);
    }

    #[test]
    fn shaman_prioritizes_slow_on_new_target() {
        let shaman = ShamanStrategy::new(10);
        let config = test_config_with_spells();
        let player = textquest_common::types::SpawnData {
            mana_current: 80,
            mana_max: 100,
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };
        let spell = shaman.select_spell(&ctx);
        assert!(spell.is_some());
        assert!(spell.unwrap().name.contains("Turgur"));
    }

    #[test]
    fn shaman_heals_when_group_low() {
        let mut shaman = ShamanStrategy::new(10);
        shaman.target_slowed = true; // already slowed
        let config = test_config_with_spells();
        let player = textquest_common::types::SpawnData {
            mana_current: 80,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![GroupMemberState {
            spawn_id: 1,
            hp_pct: 30.0,
            mana_pct: 50.0,
            class_id: 1,
            is_dead: false,
            name: "Warrior".into(),
            has_detrimental: false,
        }];
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };
        let spell = shaman.select_spell(&ctx);
        assert!(spell.is_some());
        assert!(spell.unwrap().name.contains("Heal"));
    }

    #[test]
    fn shaman_dots_when_slowed_and_healthy() {
        let mut shaman = ShamanStrategy::new(10);
        shaman.target_slowed = true;
        let config = test_config_with_spells();
        let player = textquest_common::types::SpawnData {
            mana_current: 80,
            mana_max: 100,
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };
        let spell = shaman.select_spell(&ctx);
        assert!(spell.is_some());
        assert!(spell.unwrap().name.contains("Envenomed"));
    }

    #[test]
    fn shaman_aoe_threshold() {
        let shaman = ShamanStrategy::new(10);
        assert_eq!(shaman.aoe_threshold(), 3);
    }

    #[test]
    fn shaman_should_assist() {
        let shaman = ShamanStrategy::new(10);
        let config = CombatConfig::default();
        let player = textquest_common::types::SpawnData::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };
        assert!(shaman.should_assist(&ctx));
    }

    #[test]
    fn shaman_select_target_heal_when_low() {
        let shaman = ShamanStrategy::new(10);
        let config = test_config_with_spells();
        let player = textquest_common::types::SpawnData::default();
        let target = textquest_common::types::SpawnData {
            spawn_id: 99,
            ..Default::default()
        };
        let group = vec![GroupMemberState {
            spawn_id: 42,
            hp_pct: 50.0, // below 70%
            mana_pct: 100.0,
            class_id: 1,
            is_dead: false,
            name: String::new(),
            has_detrimental: false,
        }];
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &group,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };
        assert_eq!(shaman.select_target(&ctx), Some(42));
    }

    #[test]
    fn shaman_select_target_mob_when_healthy() {
        let shaman = ShamanStrategy::new(10);
        let config = CombatConfig::default();
        let player = textquest_common::types::SpawnData::default();
        let target = textquest_common::types::SpawnData {
            spawn_id: 99,
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };
        assert_eq!(shaman.select_target(&ctx), Some(99));
    }

    #[test]
    fn shaman_select_target_afflicted_member_before_heal_or_dps() {
        let shaman = ShamanStrategy::new(10);
        let config = CombatConfig {
            spells: vec![SpellEntry {
                slot: 1,
                spell_id: 2001,
                name: "Cure Disease".into(),
                min_mana_pct: 10.0,
                priority: 1,
                is_aoe: false,
            }],
            ..CombatConfig::default()
        };
        let player = textquest_common::types::SpawnData::default();
        let target = textquest_common::types::SpawnData {
            spawn_id: 99,
            ..Default::default()
        };
        let group = vec![
            GroupMemberState {
                spawn_id: 42,
                hp_pct: 85.0,
                mana_pct: 100.0,
                class_id: 1,
                is_dead: false,
                name: String::new(),
                has_detrimental: true,
            },
            GroupMemberState {
                spawn_id: 43,
                hp_pct: 50.0,
                mana_pct: 100.0,
                class_id: 1,
                is_dead: false,
                name: String::new(),
                has_detrimental: false,
            },
        ];
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &group,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };
        assert_eq!(shaman.select_target(&ctx), Some(42));
    }

    #[test]
    fn shaman_select_target_uses_mob_when_moderate_heal_is_throttled() {
        let mut shaman = ShamanStrategy::new(10);
        shaman.last_heal_tick = Some(100);
        let config = liveish_config_with_spells();
        let player = textquest_common::types::SpawnData {
            mana_current: 90,
            mana_max: 100,
            ..Default::default()
        };
        let target = textquest_common::types::SpawnData {
            spawn_id: 77,
            hp_current: 1000,
            hp_max: 1000,
            ..Default::default()
        };
        let group = vec![GroupMemberState {
            spawn_id: 42,
            hp_pct: 50.0,
            mana_pct: 100.0,
            class_id: 1,
            is_dead: false,
            name: String::new(),
            has_detrimental: false,
        }];
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &group,
            config: &config,
            tick: 101,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };

        assert_eq!(shaman.select_spell(&ctx).unwrap().name, "Turgur's Insects");
        assert_eq!(shaman.select_target(&ctx), Some(77));
    }

    #[test]
    fn shaman_group_cure_targets_self_for_multiple_afflicted_members() {
        let shaman = ShamanStrategy::new(10);
        let player = textquest_common::types::SpawnData {
            spawn_id: 10,
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let group = vec![
            GroupMemberState {
                spawn_id: 42,
                hp_pct: 85.0,
                mana_pct: 100.0,
                class_id: 1,
                is_dead: false,
                name: String::new(),
                has_detrimental: true,
            },
            GroupMemberState {
                spawn_id: 43,
                hp_pct: 50.0,
                mana_pct: 100.0,
                class_id: 1,
                is_dead: false,
                name: String::new(),
                has_detrimental: true,
            },
        ];
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Cure Disease".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 2,
                    spell_id: 101,
                    name: "Radiant Cure".into(),
                    min_mana_pct: 10.0,
                    priority: 1,
                    is_aoe: true,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &group,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };

        assert_eq!(shaman.select_spell(&ctx).unwrap().name, "Radiant Cure");
        assert_eq!(shaman.select_target(&ctx), Some(10));
    }

    #[test]
    fn shaman_on_engage_resets_slow_on_new_target() {
        let mut shaman = ShamanStrategy::new(10);
        shaman.target_slowed = true;
        shaman.last_target_id = 1;
        let config = CombatConfig::default();
        let player = textquest_common::types::SpawnData::default();
        let new_target = textquest_common::types::SpawnData {
            spawn_id: 2,
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&new_target),
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };
        shaman.on_engage(&ctx);
        assert!(!shaman.target_slowed);
        assert_eq!(shaman.last_target_id, 2);
    }

    #[test]
    fn shaman_on_engage_keeps_slow_same_target() {
        let mut shaman = ShamanStrategy::new(10);
        shaman.target_slowed = true;
        shaman.last_target_id = 5;
        let config = CombatConfig::default();
        let player = textquest_common::types::SpawnData::default();
        let target = textquest_common::types::SpawnData {
            spawn_id: 5,
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };
        shaman.on_engage(&ctx);
        assert!(shaman.target_slowed);
    }

    #[test]
    fn shaman_no_spells_returns_none() {
        let mut shaman = ShamanStrategy::new(10);
        shaman.target_slowed = true;
        let config = CombatConfig::default();
        let player = textquest_common::types::SpawnData::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };
        assert!(shaman.select_spell(&ctx).is_none());
    }

    #[test]
    fn shaman_resisted_slow_queues_malo_before_retrying_slow() {
        let mut shaman = ShamanStrategy::new(10);
        let config = liveish_config_with_spells();
        let player = textquest_common::types::SpawnData {
            mana_current: 90,
            mana_max: 100,
            ..Default::default()
        };
        let target = textquest_common::types::SpawnData {
            spawn_id: 77,
            hp_current: 1000,
            hp_max: 1000,
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 100,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };

        shaman.on_engage(&ctx);
        assert_eq!(shaman.select_spell(&ctx).unwrap().name, "Turgur's Insects");

        shaman.on_resolved_action_outcome(
            &ctx,
            Some("Turgur's Insects"),
            1001,
            77,
            CastResult::Resisted,
        );

        let retry_ctx = CombatContext { tick: 160, ..ctx };
        assert_eq!(shaman.select_spell(&retry_ctx).unwrap().name, "Malo");

        shaman.on_resolved_action_outcome(&retry_ctx, Some("Malo"), 1002, 77, CastResult::Success);

        let slow_retry_ctx = CombatContext {
            tick: 260,
            ..retry_ctx
        };
        assert_eq!(
            shaman.select_spell(&slow_retry_ctx).unwrap().name,
            "Turgur's Insects"
        );
    }

    #[test]
    fn shaman_tracks_fallback_gem_outcomes_by_resolved_spell_id() {
        let mut shaman = ShamanStrategy::new(10);
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    slot: 1,
                    spell_id: 1001,
                    name: "Turgur's Insects".into(),
                    min_mana_pct: 35.0,
                    priority: 100,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 2,
                    spell_id: 1002,
                    name: "Malo".into(),
                    min_mana_pct: 25.0,
                    priority: 95,
                    is_aoe: false,
                },
                SpellEntry {
                    slot: 4,
                    spell_id: 1003,
                    name: "Kragg's Mending".into(),
                    min_mana_pct: 30.0,
                    priority: 90,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let player = textquest_common::types::SpawnData {
            mana_current: 90,
            mana_max: 100,
            ..Default::default()
        };
        let target = textquest_common::types::SpawnData {
            spawn_id: 77,
            hp_current: 1000,
            hp_max: 1000,
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 150,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };

        shaman.on_cast_outcome(&ctx, 4, CastResult::Success);
        shaman.on_resolved_action_outcome(&ctx, Some("Malo"), 1002, 77, CastResult::Success);

        assert!(shaman.target_maloed);
        assert_eq!(shaman.last_malo_tick, Some(150));
        assert_eq!(shaman.last_heal_tick, None);
    }

    #[test]
    fn shaman_treats_mending_line_as_heal() {
        let mut shaman = ShamanStrategy::new(10);
        shaman.target_slowed = true;
        let config = liveish_config_with_spells();
        let player = textquest_common::types::SpawnData {
            mana_current: 80,
            mana_max: 100,
            ..Default::default()
        };
        let group_members = vec![GroupMemberState {
            spawn_id: 33,
            hp_pct: 35.0,
            mana_pct: 75.0,
            class_id: 1,
            is_dead: false,
            name: "Tank".into(),
            has_detrimental: false,
        }];
        let target = textquest_common::types::SpawnData {
            spawn_id: 77,
            hp_current: 1000,
            hp_max: 1000,
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &group_members,
            config: &config,
            tick: 200,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };

        assert_eq!(shaman.select_spell(&ctx).unwrap().name, "Kragg's Mending");
    }

    #[test]
    fn shaman_canni_respects_cooldown_when_group_is_stable() {
        let mut shaman = ShamanStrategy::new(10);
        shaman.target_slowed = true;
        let config = liveish_config_with_spells();
        let player = textquest_common::types::SpawnData {
            mana_current: 45,
            mana_max: 100,
            hp_current: 5000,
            hp_max: 5000,
            ..Default::default()
        };
        let target = textquest_common::types::SpawnData {
            spawn_id: 77,
            hp_current: 1000,
            hp_max: 1000,
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 300,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };

        assert_eq!(shaman.select_spell(&ctx).unwrap().name, "Cannibalize IV");

        shaman.on_resolved_action_outcome(
            &ctx,
            Some("Cannibalize IV"),
            1004,
            77,
            CastResult::Success,
        );

        let followup_ctx = CombatContext { tick: 301, ..ctx };
        assert!(shaman.select_spell(&followup_ctx).is_none());
    }

    #[test]
    fn shaman_treats_cloud_of_grummus_as_dot_for_cooldown_gating() {
        let mut shaman = ShamanStrategy::new(10);
        shaman.target_slowed = true;
        shaman.last_dot_tick = Some(200);
        let mut config = liveish_config_with_spells();
        config.spells[4].name = "Cloud of Grummus".into();
        let player = textquest_common::types::SpawnData {
            mana_current: 80,
            mana_max: 100,
            ..Default::default()
        };
        let target = textquest_common::types::SpawnData {
            spawn_id: 77,
            hp_current: 1000,
            hp_max: 1000,
            ..Default::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 201,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
        };

        assert!(shaman.select_spell(&ctx).is_none());
    }
}
