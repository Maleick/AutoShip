use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// HP threshold above which clerics should cancel current heal (duck to interrupt).
/// Prevents wasting mana on a heal when the target is already healthy.
const HEAL_CANCEL_THRESHOLD: f32 = 85.0;

/// HP threshold for emergency heals — anyone below this gets top-priority healing.
const EMERGENCY_HP: f32 = 30.0;

/// HP threshold for moderate heals — below this, cast a standard heal.
const MODERATE_HP: f32 = 65.0;

/// Cleric strategy: healer with resurrection, prioritized heal tiers, buff support.
///
/// Priority order:
/// 1. Resurrect dead group members
/// 2. Emergency heal (group member < 30% HP)
/// 3. Moderate heal (group member < 65% HP)
/// 4. Out-of-combat: group buffs
/// 5. Med (sit for mana regen)
pub struct ClericStrategy {
    class_id: u8,
    /// Tracks whether we've already targeted a corpse for rez this combat cycle.
    /// Reset when rez cast completes or target changes.
    rez_pending: bool,
}

impl ClericStrategy {
    pub fn new(class_id: u8) -> Self {
        Self {
            class_id,
            rez_pending: false,
        }
    }

    /// Find the group member with the lowest HP percentage (alive only).
    fn lowest_hp_member(&self, ctx: &CombatContext) -> Option<(u32, f32)> {
        ctx.group_members
            .iter()
            .filter(|m| !m.is_dead && m.hp_pct > 0.0)
            .min_by(|a, b| {
                a.hp_pct
                    .partial_cmp(&b.hp_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|m| (m.spawn_id, m.hp_pct))
    }

    /// Find a dead group member who needs resurrection.
    fn dead_member<'a>(&self, ctx: &CombatContext<'a>) -> Option<&'a str> {
        ctx.group_members
            .iter()
            .find(|m| m.is_dead)
            .map(|m| m.name.as_str())
    }

    /// Find a rez spell in the spell list (by name convention).
    fn find_rez_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        ctx.config
            .spells
            .iter()
            .find(|s| {
                let name = s.name.to_lowercase();
                name.contains("resurrect")
                    || name.contains("reviviscence")
                    || name.contains("renewal")
                    || name.contains("rez")
            })
            .cloned()
    }

    /// Find a buff spell (HP buffs, AC buffs, symbol/aegolism).
    fn find_buff_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();
        ctx.config
            .spells
            .iter()
            .filter(|s| {
                let name = s.name.to_lowercase();
                name.contains("symbol")
                    || name.contains("aegolism")
                    || name.contains("armor")
                    || name.contains("guard")
                    || name.contains("buff")
            })
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
    }

    /// Check if the cleric should cancel an in-progress heal because the target
    /// has recovered above threshold. Called from the combat FSM during Casting state.
    pub fn should_cancel_heal(&self, ctx: &CombatContext) -> bool {
        let Some((_, lowest_hp)) = self.lowest_hp_member(ctx) else {
            return true; // no one to heal, cancel
        };
        lowest_hp >= HEAL_CANCEL_THRESHOLD
    }
}

impl ClassStrategy for ClericStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        // Priority 1: Dead group member for rez — signal to the FSM that we need
        // corpse targeting. We use spawn_id 0 as a sentinel (no real spawn has id 0).
        // The actual `/target <name>'s corpse` command is issued by the Combatant FSM
        // when it detects select_spell returns a rez spell.
        if !ctx.in_combat && self.dead_member(ctx).is_some() && self.find_rez_spell(ctx).is_some()
        {
            return None; // don't override target — rez spell selection handles it
        }

        // Priority 2: Lowest HP group member for healing
        self.lowest_hp_member(ctx).map(|(id, _)| id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();

        // Priority 1: Resurrect dead group members (only out of combat)
        if !ctx.in_combat {
            if self.dead_member(ctx).is_some() {
                if let Some(rez) = self.find_rez_spell(ctx) {
                    if mana_pct >= rez.min_mana_pct {
                        tracing::info!(spell = %rez.name, "Cleric: casting resurrection");
                        return Some(rez);
                    }
                }
            }
        }

        let (_, lowest_hp) = self.lowest_hp_member(ctx)?;

        // Priority 2: Emergency heal — highest priority spell
        if lowest_hp < EMERGENCY_HP {
            return ctx
                .config
                .spells
                .iter()
                .filter(|s| !is_rez_spell(s) && !is_buff_spell(s))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned();
        }

        // Priority 3: Moderate heal — lower priority (efficient) spell
        if lowest_hp < MODERATE_HP {
            return ctx
                .config
                .spells
                .iter()
                .filter(|s| !is_rez_spell(s) && !is_buff_spell(s))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .min_by_key(|s| s.priority)
                .cloned();
        }

        // Priority 4: Out-of-combat buffs
        if !ctx.in_combat {
            if let Some(buff) = self.find_buff_spell(ctx) {
                return Some(buff);
            }
        }

        // Priority 5: Everyone is healthy, med up.
        None
    }

    fn on_action_complete(&mut self, _ctx: &CombatContext) {
        // Clear rez pending flag after cast completes
        self.rez_pending = false;
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        false
    }

    fn aoe_threshold(&self) -> u8 {
        // Never AoE.
        255
    }

    fn role(&self) -> CombatRole {
        CombatRole::Healer
    }
}

/// Check if a spell entry is a resurrection spell.
fn is_rez_spell(s: &SpellEntry) -> bool {
    let name = s.name.to_lowercase();
    name.contains("resurrect")
        || name.contains("reviviscence")
        || name.contains("renewal")
        || name.contains("rez")
}

/// Check if a spell entry is a buff spell.
fn is_buff_spell(s: &SpellEntry) -> bool {
    let name = s.name.to_lowercase();
    name.contains("symbol")
        || name.contains("aegolism")
        || name.contains("armor")
        || name.contains("guard")
        || name.contains("buff")
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
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
        }
    }

    fn heal_spell(name: &str, priority: u8) -> SpellEntry {
        SpellEntry {
            slot: priority,
            spell_id: priority as i32 * 100,
            name: name.to_string(),
            min_mana_pct: 10.0,
            priority,
            is_aoe: false,
        }
    }

    /// Build a test context. Returns the config (caller must keep alive) and a
    /// context that borrows it. Uses a raw pointer cast to tie lifetimes — safe
    /// because tests are single-threaded and the config outlives the context.
    fn make_config(spells: &[SpellEntry]) -> dmft_common::combat::CombatConfig {
        dmft_common::combat::CombatConfig {
            spells: spells.to_vec(),
            ..Default::default()
        }
    }

    #[test]
    fn cleric_role_is_healer() {
        let cleric = ClericStrategy::new(2);
        assert_eq!(cleric.role(), CombatRole::Healer);
    }

    #[test]
    fn cleric_does_not_assist() {
        let cleric = ClericStrategy::new(2);
        let player = dmft_common::types::SpawnData::default();
        let config = dmft_common::combat::CombatConfig::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: false,
        };
        assert!(!cleric.should_assist(&ctx));
    }

    #[test]
    fn emergency_heal_uses_highest_priority() {
        let cleric = ClericStrategy::new(2);
        let player = dmft_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![make_member(10, 20.0, false)]; // below EMERGENCY_HP
        let spells = vec![
            heal_spell("Minor Heal", 1),
            heal_spell("Complete Heal", 10),
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
        };

        let spell = cleric.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Complete Heal"); // highest priority
    }

    #[test]
    fn moderate_heal_uses_efficient_spell() {
        let cleric = ClericStrategy::new(2);
        let player = dmft_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![make_member(10, 50.0, false)]; // between EMERGENCY and MODERATE
        let spells = vec![
            heal_spell("Minor Heal", 1),
            heal_spell("Complete Heal", 10),
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
        };

        let spell = cleric.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Minor Heal"); // lowest priority = most efficient
    }

    #[test]
    fn everyone_healthy_returns_none() {
        let cleric = ClericStrategy::new(2);
        let player = dmft_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![make_member(10, 95.0, false)]; // above MODERATE_HP
        let spells = vec![heal_spell("Minor Heal", 1)];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
        };

        assert!(cleric.select_spell(&ctx).is_none());
    }

    #[test]
    fn cancel_heal_when_group_recovered() {
        let cleric = ClericStrategy::new(2);
        let player = dmft_common::types::SpawnData::default();
        let members = vec![make_member(10, 90.0, false)]; // above HEAL_CANCEL_THRESHOLD
        let config = dmft_common::combat::CombatConfig::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
        };
        assert!(cleric.should_cancel_heal(&ctx));
    }

    #[test]
    fn dont_cancel_heal_when_group_still_hurt() {
        let cleric = ClericStrategy::new(2);
        let player = dmft_common::types::SpawnData::default();
        let members = vec![make_member(10, 40.0, false)]; // below threshold
        let config = dmft_common::combat::CombatConfig::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
        };
        assert!(!cleric.should_cancel_heal(&ctx));
    }

    #[test]
    fn rez_spell_out_of_combat() {
        let cleric = ClericStrategy::new(2);
        let player = dmft_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![
            make_member(10, 0.0, true),  // dead
            make_member(11, 80.0, false), // alive
        ];
        let spells = vec![
            heal_spell("Minor Heal", 1),
            SpellEntry {
                slot: 8,
                spell_id: 391,
                name: "Resurrection".to_string(),
                min_mana_pct: 10.0,
                priority: 20,
                is_aoe: false,
            },
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: false,
        };

        let spell = cleric.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Resurrection");
    }

    #[test]
    fn no_rez_during_combat() {
        let cleric = ClericStrategy::new(2);
        let player = dmft_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![
            make_member(10, 0.0, true),  // dead
            make_member(11, 25.0, false), // alive but critical
        ];
        let spells = vec![
            heal_spell("Minor Heal", 1),
            heal_spell("Complete Heal", 10),
            SpellEntry {
                slot: 8,
                spell_id: 391,
                name: "Resurrection".to_string(),
                min_mana_pct: 10.0,
                priority: 20,
                is_aoe: false,
            },
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
        };

        let spell = cleric.select_spell(&ctx).unwrap();
        // Should heal the living, not rez the dead during combat
        assert_ne!(spell.name, "Resurrection");
        assert_eq!(spell.name, "Complete Heal"); // emergency heal
    }

    #[test]
    fn buff_when_idle_and_healthy() {
        let cleric = ClericStrategy::new(2);
        let player = dmft_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![make_member(10, 95.0, false)]; // healthy
        let spells = vec![
            heal_spell("Minor Heal", 1),
            SpellEntry {
                slot: 5,
                spell_id: 500,
                name: "Symbol of Naltron".to_string(),
                min_mana_pct: 10.0,
                priority: 5,
                is_aoe: false,
            },
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: false,
        };

        let spell = cleric.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Symbol of Naltron");
    }

    #[test]
    fn no_buff_during_combat() {
        let cleric = ClericStrategy::new(2);
        let player = dmft_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![make_member(10, 95.0, false)]; // healthy
        let spells = vec![
            heal_spell("Minor Heal", 1),
            SpellEntry {
                slot: 5,
                spell_id: 500,
                name: "Symbol of Naltron".to_string(),
                min_mana_pct: 10.0,
                priority: 5,
                is_aoe: false,
            },
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
        };

        // In combat with everyone healthy — should return None (med)
        assert!(cleric.select_spell(&ctx).is_none());
    }

    #[test]
    fn lowest_hp_excludes_dead_members() {
        let cleric = ClericStrategy::new(2);
        let player = dmft_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![
            make_member(10, 0.0, true),  // dead — should be skipped
            make_member(11, 50.0, false), // alive, hurt
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
        };

        let (id, hp) = cleric.lowest_hp_member(&ctx).unwrap();
        assert_eq!(id, 11);
        assert!((hp - 50.0).abs() < f32::EPSILON);
    }
}
