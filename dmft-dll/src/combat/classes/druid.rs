use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext};

/// HP threshold for emergency heals.
const EMERGENCY_HP: f32 = 45.0;

/// HP threshold for standard heals.
const MODERATE_HP: f32 = 65.0;

/// HP threshold for snare (fleeing mob prevention).
const SNARE_HP: f32 = 20.0;

/// Druid strategy: hybrid healer/nuker/snarer with resurrection and buff support.
///
/// Priority order (MQ2-style cascade):
/// 0. Resurrect dead group members (out of combat, if rez spell available)
/// 1. Cure detrimental effects (poison/disease/curse)
/// 2. Emergency heal (< 45% HP)
/// 3. Snare on fleeing mobs (< 20% HP)
/// 4. Moderate heal (< 65% HP)
/// 5. Nuke/DoT
/// 6. Out-of-combat: group buffs (regen, DS, resist)
///
/// EQ class ID: 6
pub struct DruidStrategy {
    class_id: u8,
}

impl DruidStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }

    fn dead_member<'a>(&self, ctx: &CombatContext<'a>) -> Option<&'a str> {
        ctx.group_members
            .iter()
            .find(|m| m.is_dead)
            .map(|m| m.name.as_str())
    }

    fn find_spell_by_category<'a>(
        &self,
        spells: &'a [SpellEntry],
        keywords: &[&str],
    ) -> Option<&'a SpellEntry> {
        spells.iter().find(|s| {
            let name = s.name.to_lowercase();
            keywords.iter().any(|kw| name.contains(kw))
        })
    }
}

impl ClassStrategy for DruidStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        // Rez targeting: don't override target — rez spell selection handles corpse targeting
        if !ctx.in_combat
            && self.dead_member(ctx).is_some()
            && self
                .find_spell_by_category(&ctx.config.spells, &["resurrect", "rez", "reviviscence"])
                .is_some()
        {
            return None;
        }

        if let Some((heal_target, hp)) = strategy::lowest_hp_member(ctx)
            && hp < MODERATE_HP
        {
            return Some(heal_target);
        }
        ctx.target.map(|t| t.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();

        // Priority 0: Resurrect dead group members (out of combat only)
        if !ctx.in_combat {
            if self.dead_member(ctx).is_some() {
                if let Some(rez) = self.find_spell_by_category(
                    &ctx.config.spells,
                    &["resurrect", "rez", "reviviscence"],
                ) {
                    if mana_pct >= rez.min_mana_pct {
                        return Some(rez.clone());
                    }
                }
            }
        }

        // Priority 1: Cure detrimental effects
        let has_afflicted = ctx
            .group_members
            .iter()
            .any(|m| !m.is_dead && m.has_detrimental);
        if has_afflicted {
            if let Some(cure) = self.find_spell_by_category(
                &ctx.config.spells,
                &["cure", "purify", "remove", "counteract"],
            ) {
                if mana_pct >= cure.min_mana_pct {
                    return Some(cure.clone());
                }
            }
        }

        // Priority 2: Emergency heal
        if let Some((_, hp)) = strategy::lowest_hp_member(ctx)
            && hp < EMERGENCY_HP
        {
            // Even if no heal spell is configured, do NOT fall through to snare
            // when a group member is critically low. Return the heal or None.
            return ctx
                .config
                .spells
                .iter()
                .filter(|s| s.name.to_lowercase().contains("heal"))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned();
        }

        // Priority 3: Snare on low-HP mob (fleeing prevention)
        if let Some(target) = ctx.target
            && target.hp_pct() < SNARE_HP
            && let Some(snare) = ctx
                .config
                .spells
                .iter()
                .filter(|s| {
                    let name = s.name.to_lowercase();
                    name.contains("snare") || name.contains("ensnare")
                })
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned()
        {
            return Some(snare);
        }

        // Priority 4: Heal if group member below moderate threshold
        if let Some((_, hp)) = strategy::lowest_hp_member(ctx)
            && hp < MODERATE_HP
        {
            return ctx
                .config
                .spells
                .iter()
                .filter(|s| s.name.to_lowercase().contains("heal"))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned();
        }

        // Priority 5: Nuke/DoT (in combat)
        if ctx.in_combat {
            return ctx
                .config
                .spells
                .iter()
                .filter(|s| {
                    let name = s.name.to_lowercase();
                    !name.contains("heal")
                        && !name.contains("snare")
                        && !name.contains("ensnare")
                        && !name.contains("regen")
                        && !name.contains("skin")
                        && !name.contains("resist")
                        && !name.contains("shield")
                        && !name.contains("resurrect")
                        && !name.contains("rez")
                })
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned();
        }

        // Priority 6: Out-of-combat buffs (regen, damage shield, resist buffs)
        ctx.config
            .spells
            .iter()
            .filter(|s| {
                let name = s.name.to_lowercase();
                name.contains("regen")
                    || name.contains("skin")
                    || name.contains("resist")
                    || name.contains("shield")
                    || name.contains("buff")
            })
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
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

    fn make_member(spawn_id: u32, hp_pct: f32, is_dead: bool) -> GroupMemberState {
        GroupMemberState {
            spawn_id,
            hp_pct,
            mana_pct: 100.0,
            class_id: 1,
            is_dead,
            name: format!("Player{spawn_id}"),
            has_detrimental: false,
        }
    }

    #[test]
    fn druid_class_id() {
        let druid = DruidStrategy::new(6);
        assert_eq!(druid.class_id(), 6);
    }

    #[test]
    fn druid_role_is_healer() {
        let druid = DruidStrategy::new(6);
        assert_eq!(druid.role(), CombatRole::Healer);
    }

    #[test]
    fn druid_should_assist() {
        let druid = DruidStrategy::new(6);
        let config = dmft_common::combat::CombatConfig::default();
        let player = dmft_common::types::SpawnData::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
        };
        assert!(druid.should_assist(&ctx));
    }

    #[test]
    fn druid_lowest_hp_excludes_dead() {
        let player = dmft_common::types::SpawnData::default();
        let members = vec![
            make_member(1, 0.0, true),   // dead
            make_member(2, 40.0, false), // alive, hurt
        ];
        let config = dmft_common::combat::CombatConfig::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
        };

        let (id, hp) = strategy::lowest_hp_member(&ctx).unwrap();
        assert_eq!(id, 2);
        assert!((hp - 40.0).abs() < f32::EPSILON);
    }
}
