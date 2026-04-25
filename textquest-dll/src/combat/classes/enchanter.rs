use std::cell::RefCell;

use textquest_common::combat::{
    AbilityCandidate, EQExpansion, AbilitySet, ActionType, CastResult, CombatRole, CombatStateReq,
    ConditionExpr, SpellEntry, TargetSelector,
};

use crate::combat::{
    mez_queue::MezQueue,
    rotation::{self, RotationGroup},
    strategy::{self, ClassStrategy, CombatContext},
};

const ADD_CONTROL_THRESHOLD: u32 = 2;
const EMERGENCY_STUN_THRESHOLD: u32 = 4;
const MAX_TRACKED_CC_TARGETS: u8 = 4;

const MEZ_RETRY_COOLDOWN_TICKS: u32 = 20;
const COLOR_STUN_COOLDOWN_TICKS: u32 = 120;
const TASH_COOLDOWN_TICKS: u32 = 1_200;
const SLOW_COOLDOWN_TICKS: u32 = 1_200;

/// Enchanter strategy: crowd control first, then kill-target debuffs, then
/// mana-safe nukes.
///
/// Rotation order:
/// 1. Emergency `ColorStun` when add count spikes.
/// 2. Off-target add control via `Tash` -> `Mez` on the strategy-selected CC
///    target.
/// 3. Kill-target debuffs (`Tash`, `Slow`) once adds are stable.
/// 4. `Nuke` only when mana is comfortably above the floor.
pub struct EnchanterStrategy {
    class_id: u8,
    mez_queue: RefCell<MezQueue>,
}

impl EnchanterStrategy {
    pub fn new(class_id: u8) -> Self {
        Self {
            class_id,
            mez_queue: RefCell::new(MezQueue::new(MAX_TRACKED_CC_TARGETS)),
        }
    }

    fn build_ability_sets() -> Vec<AbilitySet> {
        vec![
            AbilitySet {
                name: "Mez".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Bliss".into(),
                        min_level: 65,
                        spell_id: 5520,
                    },
                    AbilityCandidate {
                        name: "Glamour of Kintaz".into(),
                        min_level: 60,
                        spell_id: 3341,
                    },
                    AbilityCandidate {
                        name: "Dazzle".into(),
                        min_level: 36,
                        spell_id: 187,
                    },
                    AbilityCandidate {
                        name: "Entrance".into(),
                        min_level: 24,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Enthrall".into(),
                        min_level: 12,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Mesmerize".into(),
                        min_level: 4,
                        spell_id: 185,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "ColorStun".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Color Slant".into(),
                        min_level: 44,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Color Skew".into(),
                        min_level: 30,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Color Shift".into(),
                        min_level: 16,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Color Flux".into(),
                        min_level: 2,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Haste".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Speed of Vallon".into(),
                        min_level: 65,
                        spell_id: 5522,
                    },
                    AbilityCandidate {
                        name: "Speed of the Shissar".into(),
                        min_level: 60,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Wonderous Rapidity".into(),
                        min_level: 57,
                        spell_id: 1693,
                    },
                    AbilityCandidate {
                        name: "Swift like the Wind".into(),
                        min_level: 44,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Alacrity".into(),
                        min_level: 24,
                        spell_id: 170,
                    },
                    AbilityCandidate {
                        name: "Quickness".into(),
                        min_level: 16,
                        spell_id: 171,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Clarity".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Clarity II".into(),
                        min_level: 54,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Clarity".into(),
                        min_level: 29,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Breeze".into(),
                        min_level: 14,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Slow".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Dreary Deeds".into(),
                        min_level: 60,
                        spell_id: 3344,
                    },
                    AbilityCandidate {
                        name: "Tepid Deeds".into(),
                        min_level: 52,
                        spell_id: 2151,
                    },
                    AbilityCandidate {
                        name: "Shiftless Deeds".into(),
                        min_level: 24,
                        spell_id: 191,
                    },
                    AbilityCandidate {
                        name: "Languid Pace".into(),
                        min_level: 12,
                        spell_id: 190,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Nuke".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Dementia".into(),
                        min_level: 60,
                        spell_id: 3343,
                    },
                    AbilityCandidate {
                        name: "Sanity Warp".into(),
                        min_level: 44,
                        spell_id: 1694,
                    },
                    AbilityCandidate {
                        name: "Chaotic Feedback".into(),
                        min_level: 1,
                        spell_id: 186,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Tash".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Wind of Tashani".into(),
                        min_level: 62,
                        spell_id: 5523,
                    },
                    AbilityCandidate {
                        name: "Tashanian".into(),
                        min_level: 60,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Tashania".into(),
                        min_level: 44,
                        spell_id: 2153,
                    },
                    AbilityCandidate {
                        name: "Tashani".into(),
                        min_level: 20,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Tashan".into(),
                        min_level: 4,
                        spell_id: 188,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Charm".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Command of Druzzil".into(),
                        min_level: 65,
                        spell_id: 5524,
                    },
                    AbilityCandidate {
                        name: "Boltran's Agacerie".into(),
                        min_level: 58,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Allure".into(),
                        min_level: 51,
                        spell_id: 2152,
                    },
                    AbilityCandidate {
                        name: "Cajoling Whispers".into(),
                        min_level: 46,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Beguile".into(),
                        min_level: 30,
                        spell_id: 192,
                    },
                    AbilityCandidate {
                        name: "Charm".into(),
                        min_level: 12,
                        spell_id: 193,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
        ]
    }

    fn build_rotations() -> Vec<RotationGroup> {
        vec![
            {
                let mut g = rotation::group(
                    "EmergencyCc",
                    TargetSelector::SelfOnly,
                    CombatStateReq::Combat,
                );
                g.full_rotation = true;
                g.entries = vec![conditioned(
                    rotation::entry_with_cooldown(
                        "ColorStun",
                        ActionType::Spell("ColorStun".into()),
                        "ColorStun",
                        COLOR_STUN_COOLDOWN_TICKS,
                    ),
                    ConditionExpr::And(vec![
                        ConditionExpr::EnemyCountAbove(EMERGENCY_STUN_THRESHOLD),
                        ConditionExpr::ManaAbove(45.0),
                    ]),
                )];
                g
            },
            {
                let mut g = rotation::group(
                    "AddControl",
                    TargetSelector::StrategyTarget,
                    CombatStateReq::Combat,
                );
                g.entries = vec![
                    conditioned(
                        rotation::entry_with_cooldown(
                            "Tash",
                            ActionType::Spell("Tash".into()),
                            "Tash",
                            TASH_COOLDOWN_TICKS,
                        ),
                        ConditionExpr::And(vec![
                            ConditionExpr::EnemyCountAbove(ADD_CONTROL_THRESHOLD),
                            ConditionExpr::ManaAbove(35.0),
                        ]),
                    ),
                    conditioned(
                        rotation::entry_with_cooldown(
                            "Mez",
                            ActionType::Spell("Mez".into()),
                            "Mez",
                            MEZ_RETRY_COOLDOWN_TICKS,
                        ),
                        ConditionExpr::And(vec![
                            ConditionExpr::EnemyCountAbove(ADD_CONTROL_THRESHOLD),
                            ConditionExpr::ManaAbove(25.0),
                        ]),
                    ),
                ];
                g
            },
            {
                let mut g = rotation::group(
                    "Debuffs",
                    TargetSelector::AutoTarget,
                    CombatStateReq::Combat,
                );
                g.entries = vec![
                    conditioned(
                        rotation::entry_with_cooldown(
                            "Tash",
                            ActionType::Spell("Tash".into()),
                            "Tash",
                            TASH_COOLDOWN_TICKS,
                        ),
                        ConditionExpr::And(vec![
                            ConditionExpr::TargetHpAbove(95.0),
                            ConditionExpr::ManaAbove(35.0),
                        ]),
                    ),
                    conditioned(
                        rotation::entry_with_cooldown(
                            "Slow",
                            ActionType::Spell("Slow".into()),
                            "Slow",
                            SLOW_COOLDOWN_TICKS,
                        ),
                        ConditionExpr::And(vec![
                            ConditionExpr::TargetHpAbove(80.0),
                            ConditionExpr::ManaAbove(50.0),
                        ]),
                    ),
                ];
                g
            },
            {
                let mut g =
                    rotation::group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
                g.entries = vec![conditioned(
                    rotation::entry("Nuke", ActionType::Spell("Nuke".into())),
                    ConditionExpr::ManaAbove(65.0),
                )];
                g
            },
        ]
    }

    fn queued_refresh_target(&self, ctx: &CombatContext) -> Option<u32> {
        let mut mez_queue = self.mez_queue.borrow_mut();
        mez_queue.prune_expired(ctx.tick);

        let mut refresh_target = mez_queue.next_refresh_target(ctx.tick);
        while let Some(target_id) = refresh_target {
            if ctx
                .nearby_enemies
                .iter()
                .any(|enemy| enemy.spawn_id == target_id)
            {
                return Some(target_id);
            }
            mez_queue.remove_target(target_id);
            refresh_target = mez_queue.next_refresh_target(ctx.tick);
        }

        None
    }

    fn first_uncontrolled_add(ctx: &CombatContext) -> Option<u32> {
        if ctx.nearby_enemies.len() <= 1 {
            return None;
        }

        if let Some(target_id) = ctx.extended_targets.and_then(|xtargets| {
            xtargets.cc_add_spawn_ids().into_iter().find(|spawn_id| {
                ctx.nearby_enemies
                    .iter()
                    .find(|enemy| enemy.spawn_id == *spawn_id)
                    .is_some_and(|enemy| !strategy::is_mezzed(enemy))
            })
        }) {
            return Some(target_id);
        }

        let current_target_id = ctx.target.map(|target| target.spawn_id);
        ctx.nearby_enemies
            .iter()
            .enumerate()
            .filter(|(index, enemy)| {
                Some(enemy.spawn_id) != current_target_id
                    && (current_target_id.is_some() || *index > 0)
            })
            .map(|(_, enemy)| enemy)
            .find(|enemy| !strategy::is_mezzed(enemy))
            .map(|enemy| enemy.spawn_id)
    }

    fn queue_target(&self, target_id: u32, level: u8, tick: u32) {
        self.mez_queue.borrow_mut().add_target(
            target_id,
            mez_duration_ticks_for_level(level),
            tick,
        );
    }
}

impl ClassStrategy for EnchanterStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        if let Some(refresh_target) = self.queued_refresh_target(ctx) {
            return Some(refresh_target);
        }

        let target_id = Self::first_uncontrolled_add(ctx)?;
        self.queue_target(target_id, ctx.player.level, ctx.tick);
        Some(target_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        strategy::best_spell_by_mana(ctx)
    }

    fn should_assist(&self, ctx: &CombatContext) -> bool {
        self.queued_refresh_target(ctx).is_none() && Self::first_uncontrolled_add(ctx).is_none()
    }

    fn aoe_threshold(&self) -> u8 {
        EMERGENCY_STUN_THRESHOLD as u8
    }

    fn role(&self) -> CombatRole {
        CombatRole::CrowdControl
    }

    fn rotation_groups(&self) -> Option<Vec<RotationGroup>> {
        Some(Self::build_rotations())
    }

    fn ability_sets(&self) -> Vec<AbilitySet> {
        Self::build_ability_sets()
    }

    fn on_action_complete(&mut self, ctx: &CombatContext) {
        if !ctx.in_combat {
            *self.mez_queue.get_mut() = MezQueue::new(MAX_TRACKED_CC_TARGETS);
            return;
        }

        self.mez_queue.get_mut().prune_expired(ctx.tick);
    }

    fn on_resolved_action_outcome(
        &mut self,
        ctx: &CombatContext,
        entry_name: Option<&str>,
        _spell_id: i32,
        target_id: u32,
        result: CastResult,
    ) {
        if entry_name != Some("Mez") || target_id == 0 {
            return;
        }

        let mez_duration = mez_duration_ticks_for_level(ctx.player.level);
        let mez_queue = self.mez_queue.get_mut();

        match result {
            CastResult::Success => {
                mez_queue.add_target(target_id, mez_duration, ctx.tick);
                mez_queue.record_mez_success(target_id, mez_duration, ctx.tick);
            }
            CastResult::Resisted | CastResult::Immune | CastResult::TakeHold => {
                mez_queue.record_mez_resist(target_id);
            }
            _ => {}
        }
    }
}

fn conditioned(
    mut entry: rotation::RotationEntry,
    condition: ConditionExpr,
) -> rotation::RotationEntry {
    entry.condition = Some(condition);
    entry
}

fn mez_duration_ticks_for_level(level: u8) -> u32 {
    match level {
        0..=11 => 360,
        12..=23 => 720,
        24..=35 => 1_440,
        _ => 1_920,
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use serde::Deserialize;
    use textquest_common::{
        combat::{
            CombatConfig, ExtendedTargetList, ExtendedTargetSlot, XTargetSlotStatus, XTargetType,
        },
        types::SpawnData,
    };

    fn crate_manifest_dir() -> PathBuf {
        let mut dir = std::env::current_dir().expect("test working directory should be readable");

        loop {
            if dir.join("Cargo.toml").exists()
                && dir.file_name().and_then(|name| name.to_str()) == Some("textquest-dll")
            {
                return dir;
            }

            let nested = dir.join("textquest-dll");
            if nested.join("Cargo.toml").exists() {
                return nested;
            }

            if !dir.pop() {
                panic!("could not locate textquest-dll crate root from test working directory");
            }
        }
    }

    fn class_config_path(file_name: &str) -> PathBuf {
        crate_manifest_dir()
            .parent()
            .expect("textquest-dll crate should have a workspace parent")
            .join("config")
            .join("classes")
            .join(file_name)
    }

    #[derive(Clone, Debug, Deserialize)]
    struct ConfigCombatAbility {
        name: String,
        priority: u8,
        #[serde(default)]
        condition: Option<String>,
    }

    #[derive(Clone, Debug, Deserialize)]
    struct ConfigCcAbility {
        name: String,
        cc_type: String,
    }

    #[derive(Clone, Debug, Deserialize)]
    struct ConfigDebuffAbility {
        name: String,
    }

    #[derive(Clone, Debug, Default, Deserialize)]
    struct ConfigProfileOverride {
        #[serde(default)]
        min_level: Option<u8>,
        #[serde(default)]
        max_level: Option<u8>,
        #[serde(default)]
        combat_abilities: Option<Vec<ConfigCombatAbility>>,
        #[serde(default)]
        buff_abilities: Option<Vec<ConfigCombatAbility>>,
        #[serde(default)]
        cc_abilities: Option<Vec<ConfigCcAbility>>,
        #[serde(default)]
        debuff_abilities: Option<Vec<ConfigDebuffAbility>>,
    }

    #[derive(Clone, Debug, Deserialize)]
    struct ConfigProfile {
        class_name: String,
        role: String,
        #[serde(default)]
        level_overrides: Vec<ConfigProfileOverride>,
        #[serde(default)]
        combat_abilities: Vec<ConfigCombatAbility>,
        #[serde(default)]
        buff_abilities: Vec<ConfigCombatAbility>,
        #[serde(default)]
        cc_abilities: Vec<ConfigCcAbility>,
        #[serde(default)]
        debuff_abilities: Vec<ConfigDebuffAbility>,
    }

    #[derive(Clone, Debug)]
    struct ResolvedConfigProfile {
        combat_abilities: Vec<ConfigCombatAbility>,
        buff_abilities: Vec<ConfigCombatAbility>,
        cc_abilities: Vec<ConfigCcAbility>,
        debuff_abilities: Vec<ConfigDebuffAbility>,
    }

    impl ConfigProfile {
        fn load() -> Self {
            let config_path = class_config_path("enchanter.toml");
            let contents =
                std::fs::read_to_string(config_path).expect("enchanter.toml should be readable");
            toml::from_str(&contents).expect("enchanter.toml should parse")
        }

        fn profile_for_level(&self, level: u8) -> ResolvedConfigProfile {
            let selected_override = self
                .level_overrides
                .iter()
                .filter(|ovr| {
                    let min = ovr.min_level.unwrap_or(0);
                    let max = ovr.max_level.unwrap_or(u8::MAX);
                    level >= min && level <= max
                })
                .max_by_key(|ovr| (ovr.min_level.unwrap_or(0), ovr.max_level.unwrap_or(u8::MAX)));

            ResolvedConfigProfile {
                combat_abilities: selected_override
                    .and_then(|ovr| ovr.combat_abilities.clone())
                    .unwrap_or_else(|| self.combat_abilities.clone()),
                buff_abilities: selected_override
                    .and_then(|ovr| ovr.buff_abilities.clone())
                    .unwrap_or_else(|| self.buff_abilities.clone()),
                cc_abilities: selected_override
                    .and_then(|ovr| ovr.cc_abilities.clone())
                    .unwrap_or_else(|| self.cc_abilities.clone()),
                debuff_abilities: selected_override
                    .and_then(|ovr| ovr.debuff_abilities.clone())
                    .unwrap_or_else(|| self.debuff_abilities.clone()),
            }
        }
    }

    fn make_enemy(spawn_id: u32) -> SpawnData {
        SpawnData {
            spawn_id,
            spawn_type: 1,
            ..SpawnData::default()
        }
    }

    fn make_ctx<'a>(
        player: &'a SpawnData,
        target: Option<&'a SpawnData>,
        enemies: &'a [SpawnData],
        config: &'a CombatConfig,
        tick: u32,
    ) -> CombatContext<'a> {
        CombatContext {
            player,
            target,
            nearby_enemies: enemies,
            group_members: &[],
            config,
            tick,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: target.is_some_and(strategy::is_mezzed),
            extended_targets: None,
        }
    }

    fn known_abilities() -> Vec<textquest_common::combat::KnownAbility> {
        EnchanterStrategy::build_ability_sets()
            .iter()
            .flat_map(|set| &set.candidates)
            .map(|candidate| textquest_common::combat::KnownAbility {
                name: candidate.name.clone(),
                spell_id: candidate.spell_id,
                level: candidate.min_level,
            })
            .collect()
    }

    #[test]
    fn enchanter_class_id() {
        let enc = EnchanterStrategy::new(14);
        assert_eq!(enc.class_id(), 14);
    }

    #[test]
    fn enchanter_role_is_cc() {
        let enc = EnchanterStrategy::new(14);
        assert_eq!(enc.role(), CombatRole::CrowdControl);
    }

    #[test]
    fn enchanter_aoe_threshold_matches_emergency_stun_gate() {
        let enc = EnchanterStrategy::new(14);
        assert_eq!(enc.aoe_threshold(), 4);
    }

    #[test]
    fn enchanter_assists_when_no_add_needs_control() {
        let enc = EnchanterStrategy::new(14);
        let player = SpawnData::default();
        let target = make_enemy(1);
        let enemies = vec![target.clone()];
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &enemies, &config, 0);
        assert!(enc.should_assist(&ctx));
        assert!(enc.select_target(&ctx).is_none());
    }

    #[test]
    fn enchanter_select_target_picks_first_uncontrolled_off_target() {
        let enc = EnchanterStrategy::new(14);
        let player = SpawnData {
            level: 60,
            ..SpawnData::default()
        };
        let primary = make_enemy(1);
        let add = make_enemy(2);
        let enemies = vec![primary.clone(), add];
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&primary), &enemies, &config, 10);

        assert_eq!(enc.select_target(&ctx), Some(2));
        assert!(!enc.should_assist(&ctx));
    }

    #[test]
    fn enchanter_select_target_prefers_xtarget_cc_add() {
        let enc = EnchanterStrategy::new(14);
        let player = SpawnData {
            level: 60,
            ..SpawnData::default()
        };
        let primary = make_enemy(1);
        let off_target = make_enemy(2);
        let cc_add = make_enemy(3);
        let enemies = vec![primary.clone(), off_target, cc_add];
        let config = CombatConfig::default();
        let xtargets = ExtendedTargetList {
            slots: vec![
                ExtendedTargetSlot {
                    slot_type: XTargetType::GroupAssistTarget,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 1,
                    name: "main".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::AutoHater,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 3,
                    name: "add".into(),
                },
            ],
            auto_add_haters: true,
        };
        let mut ctx = make_ctx(&player, Some(&primary), &enemies, &config, 10);
        ctx.extended_targets = Some(&xtargets);

        assert_eq!(enc.select_target(&ctx), Some(3));
    }

    #[test]
    fn enchanter_select_target_skips_mezzed_off_targets() {
        let enc = EnchanterStrategy::new(14);
        let player = SpawnData {
            level: 60,
            ..SpawnData::default()
        };
        let primary = make_enemy(1);
        let mut mezzed_add = make_enemy(2);
        mezzed_add.stand_state = 4;
        mezzed_add.speed_run = 0.0;
        let fresh_add = make_enemy(3);
        let enemies = vec![primary.clone(), mezzed_add, fresh_add];
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&primary), &enemies, &config, 10);

        assert_eq!(enc.select_target(&ctx), Some(3));
    }

    #[test]
    fn enchanter_select_target_skips_primary_enemy_when_target_snapshot_is_missing() {
        let enc = EnchanterStrategy::new(14);
        let player = SpawnData {
            level: 60,
            ..SpawnData::default()
        };
        let primary = make_enemy(1);
        let add = make_enemy(2);
        let enemies = vec![primary, add];
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &enemies, &config, 10);

        assert_eq!(enc.select_target(&ctx), Some(2));
    }

    #[test]
    fn enchanter_select_target_returns_refresh_target_when_mez_needs_recast() {
        let mut enc = EnchanterStrategy::new(14);
        let player = SpawnData {
            level: 60,
            ..SpawnData::default()
        };
        let primary = make_enemy(1);
        let tracked_add = make_enemy(2);
        let enemies = vec![primary.clone(), tracked_add];
        let config = CombatConfig::default();
        let initial_ctx = make_ctx(&player, Some(&primary), &enemies, &config, 0);

        enc.on_resolved_action_outcome(&initial_ctx, Some("Mez"), 3341, 2, CastResult::Success);

        let refresh_ctx = make_ctx(&player, Some(&primary), &enemies, &config, 1_900);
        assert_eq!(enc.select_target(&refresh_ctx), Some(2));
    }

    #[test]
    fn enchanter_has_rotation_groups() {
        let enc = EnchanterStrategy::new(14);
        let groups = enc.rotation_groups().expect("rotation groups");
        let names: Vec<&str> = groups.iter().map(|group| group.name.as_str()).collect();
        assert!(names.contains(&"EmergencyCc"));
        assert!(names.contains(&"AddControl"));
        assert!(names.contains(&"Debuffs"));
        assert!(names.contains(&"Combat"));
    }

    #[test]
    fn enchanter_rotation_scenario_controls_add_then_returns_to_primary_target() {
        let mut enc = EnchanterStrategy::new(14);
        let mut groups = enc.rotation_groups().unwrap();
        let player = SpawnData {
            level: 60,
            mana_current: 9_000,
            mana_max: 10_000,
            ..SpawnData::default()
        };
        let primary = make_enemy(1);
        let add = make_enemy(2);
        let enemies = vec![primary.clone(), add];
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&primary), &enemies, &config, 10);

        let cc_target = enc.select_target(&ctx);
        let tash = rotation::execute_rotations_with_strategy_target(&mut groups, &ctx, cc_target)
            .expect("add control should start with Tash");
        assert_eq!(tash.entry_name, "Tash");
        assert_eq!(tash.target_id, 2);

        let mez = rotation::execute_rotations_with_strategy_target(&mut groups, &ctx, cc_target)
            .expect("second add control step should mez");
        assert_eq!(mez.entry_name, "Mez");
        assert_eq!(mez.target_id, 2);

        enc.on_resolved_action_outcome(&ctx, Some("Mez"), 3341, 2, CastResult::Success);
        let mut controlled_add = make_enemy(2);
        controlled_add.stand_state = 4;
        controlled_add.speed_run = 0.0;
        let controlled_enemies = vec![primary.clone(), controlled_add];
        let controlled_ctx = make_ctx(&player, Some(&primary), &controlled_enemies, &config, 10);

        let primary_followup = rotation::execute_rotations_with_strategy_target(
            &mut groups,
            &controlled_ctx,
            enc.select_target(&controlled_ctx),
        )
        .expect("once adds are stable, primary-target debuffs should resume");
        assert_eq!(primary_followup.entry_name, "Tash");
        assert_eq!(primary_followup.target_id, 1);
    }

    #[test]
    fn enchanter_has_ability_sets_for_cc_debuffs_buffs_and_utility() {
        let enc = EnchanterStrategy::new(14);
        let sets = enc.ability_sets();
        let names: Vec<&str> = sets.iter().map(|set| set.name.as_str()).collect();
        assert!(names.contains(&"Mez"));
        assert!(names.contains(&"ColorStun"));
        assert!(names.contains(&"Haste"));
        assert!(names.contains(&"Clarity"));
        assert!(names.contains(&"Slow"));
        assert!(names.contains(&"Nuke"));
        assert!(names.contains(&"Tash"));
        assert!(names.contains(&"Charm"));
    }

    #[test]
    fn enchanter_level_breakpoints_resolve_expected_spells() {
        let sets = EnchanterStrategy::build_ability_sets();
        let known = known_abilities();

        let at_60 = textquest_common::combat::resolve_abilities(&sets, &known, 60);
        assert_eq!(at_60.get("Mez").unwrap().ability_name, "Glamour of Kintaz");
        assert_eq!(
            at_60.get("Haste").unwrap().ability_name,
            "Speed of the Shissar"
        );
        assert_eq!(at_60.get("Clarity").unwrap().ability_name, "Clarity II");
        assert_eq!(at_60.get("Tash").unwrap().ability_name, "Tashanian");
        assert_eq!(
            at_60.get("Charm").unwrap().ability_name,
            "Boltran's Agacerie"
        );

        let at_61 = textquest_common::combat::resolve_abilities(&sets, &known, 61);
        assert_eq!(at_61.get("Mez").unwrap().ability_name, "Glamour of Kintaz");
        assert_eq!(at_61.get("Tash").unwrap().ability_name, "Tashanian");

        let at_62 = textquest_common::combat::resolve_abilities(&sets, &known, 62);
        assert_eq!(at_62.get("Tash").unwrap().ability_name, "Wind of Tashani");

        let at_65 = textquest_common::combat::resolve_abilities(&sets, &known, 65);
        assert_eq!(at_65.get("Mez").unwrap().ability_name, "Bliss");
        assert_eq!(at_65.get("Haste").unwrap().ability_name, "Speed of Vallon");
        assert_eq!(
            at_65.get("Charm").unwrap().ability_name,
            "Command of Druzzil"
        );
    }

    #[test]
    fn enchanter_toml_breakpoints_match_runtime_resolution() {
        let config = ConfigProfile::load();
        let sets = EnchanterStrategy::build_ability_sets();
        let known = known_abilities();

        assert_eq!(config.class_name, "enchanter");
        assert_eq!(config.role, "cc");

        for level in [60_u8, 61, 62, 65] {
            let profile = config.profile_for_level(level);
            let runtime = textquest_common::combat::resolve_abilities(&sets, &known, level);

            let mez = profile
                .cc_abilities
                .iter()
                .find(|ability| ability.cc_type == "mez")
                .expect("mez line should be configured");
            let charm = profile
                .cc_abilities
                .iter()
                .find(|ability| ability.cc_type == "charm")
                .expect("charm line should be configured");

            assert_eq!(
                profile.cc_abilities[0].name,
                runtime["ColorStun"].ability_name
            );
            assert_eq!(mez.name, runtime["Mez"].ability_name);
            assert_eq!(charm.name, runtime["Charm"].ability_name);
            assert_eq!(
                profile.debuff_abilities[0].name,
                runtime["Tash"].ability_name
            );
            assert_eq!(
                profile.debuff_abilities[1].name,
                runtime["Slow"].ability_name
            );
            assert_eq!(
                profile.buff_abilities[0].name,
                runtime["Haste"].ability_name
            );
            assert_eq!(
                profile.buff_abilities[1].name,
                runtime["Clarity"].ability_name
            );
            assert_eq!(
                profile.combat_abilities.last().expect("nuke entry").name,
                runtime["Nuke"].ability_name
            );
        }
    }

    // ── Charm / pet tests ─────────────────────────────────────────────────

    /// Enchanter ability sets expose a "Charm" line with level-appropriate
    /// candidates (level 12 base → Command of Druzzil at 65).
    #[test]
    fn enchanter_charm_ability_set_is_present() {
        let sets = EnchanterStrategy::build_ability_sets();
        let charm = sets.iter().find(|s| s.name == "Charm");
        assert!(charm.is_some(), "Charm ability set must be defined");
        let charm = charm.unwrap();
        assert!(
            !charm.candidates.is_empty(),
            "Charm set must have candidates"
        );
        // Base spell available from level 12
        let base = charm.candidates.iter().find(|c| c.name == "Charm");
        assert!(
            base.is_some(),
            "base 'Charm' candidate (level 12) must be present"
        );
        assert_eq!(base.unwrap().min_level, 12);
        // Top-tier spell at 65
        let top = charm
            .candidates
            .iter()
            .find(|c| c.name == "Command of Druzzil");
        assert!(
            top.is_some(),
            "'Command of Druzzil' (level 65) must be present"
        );
        assert_eq!(top.unwrap().min_level, 65);
    }

    /// Resolve_abilities selects the strongest charm the character can cast.
    #[test]
    fn enchanter_charm_resolves_to_level_appropriate_spell() {
        let sets = EnchanterStrategy::build_ability_sets();
        let known = known_abilities();

        let at_60 = textquest_common::combat::resolve_abilities(&sets, &known, 60);
        assert_eq!(
            at_60
                .get("Charm")
                .expect("Charm resolves at 60")
                .ability_name,
            "Boltran's Agacerie",
        );

        let at_65 = textquest_common::combat::resolve_abilities(&sets, &known, 65);
        assert_eq!(
            at_65
                .get("Charm")
                .expect("Charm resolves at 65")
                .ability_name,
            "Command of Druzzil",
        );
    }

    /// A charmed mob appears as MyPet in the xtarget list.  When that slot is
    /// populated the pet_status helper should see a live pet spawn.
    #[test]
    fn pet_status_detects_charmed_mob_via_xtarget_my_pet() {
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let xtargets = ExtendedTargetList {
            slots: vec![ExtendedTargetSlot {
                slot_type: XTargetType::MyPet,
                status: XTargetSlotStatus::CurrentZone,
                spawn_id: 500,
                name: "a goblin shaman".into(),
            }],
            auto_add_haters: false,
        };
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
            extended_targets: Some(&xtargets),
        };
        let status = ctx.pet_status();
        assert!(
            status.has_pet(),
            "charmed mob in MyPet slot must register as active pet"
        );
        assert_eq!(status.spawn_id, Some(500));
    }

    /// When the charmed mob breaks charm it leaves the MyPet xtarget slot.
    /// After the break, pet_status must report no active pet.
    #[test]
    fn pet_status_reflects_charm_break_when_my_pet_slot_empty() {
        let player = SpawnData::default();
        let config = CombatConfig::default();
        // No xtargets at all (charm broke, mob returned to hostile)
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
        };
        let status = ctx.pet_status();
        assert!(!status.has_pet(), "no MyPet slot means charm is broken");
        assert_eq!(status.spawn_id, None);
    }

    /// After charm breaks the formerly-charmed mob reappears as an enemy add.
    /// The enchanter's select_target should pick it up for re-CC.
    #[test]
    fn enchanter_select_target_picks_up_recharm_candidate_after_break() {
        let enc = EnchanterStrategy::new(14);
        let player = SpawnData {
            level: 60,
            mana_current: 9_000,
            mana_max: 10_000,
            ..SpawnData::default()
        };
        // Primary target and a newly-hostile mob (former charmed pet)
        let primary = make_enemy(1);
        let broke_charm = make_enemy(2); // hostile again
        let enemies = vec![primary.clone(), broke_charm];
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&primary), &enemies, &config, 10);

        // Enchanter should identify the second enemy as a CC candidate
        let cc_target = enc.select_target(&ctx);
        assert_eq!(
            cc_target,
            Some(2),
            "enchanter must target re-appeared hostile mob for recharm/mez"
        );
    }

    /// Pet commands: when a pet is active and idle the pet_attack_action helper
    /// must request an Attack.
    #[test]
    fn pet_attack_action_requests_attack_when_pet_is_idle() {
        use crate::combat::strategy::{PetAction, pet_attack_action};
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 99,
            ..SpawnData::default()
        };
        let enemies = vec![target.clone()];
        let config = CombatConfig::default();
        let xtargets = ExtendedTargetList {
            slots: vec![ExtendedTargetSlot {
                slot_type: XTargetType::MyPet,
                status: XTargetSlotStatus::CurrentZone,
                spawn_id: 77,
                name: "a charmed gnoll".into(),
            }],
            auto_add_haters: false,
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &enemies,
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: Some(&xtargets),
        };
        assert_eq!(
            pet_attack_action(&ctx),
            Some(PetAction::Attack),
            "idle charmed pet should trigger /pet attack"
        );
    }

    /// Pet commands: if the pet is already attacking the current target,
    /// pet_attack_action must not send a redundant command.
    #[test]
    fn pet_attack_action_skips_when_pet_already_on_target() {
        use crate::combat::strategy::pet_attack_action;
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 99,
            ..SpawnData::default()
        };
        let enemies = vec![target.clone()];
        let config = CombatConfig::default();
        // Pet slot + PetTarget slot pointing at same target → already attacking
        let xtargets = ExtendedTargetList {
            slots: vec![
                ExtendedTargetSlot {
                    slot_type: XTargetType::MyPet,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 77,
                    name: "a charmed gnoll".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::MyPetTarget,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 99,
                    name: "a skeleton".into(),
                },
            ],
            auto_add_haters: false,
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &enemies,
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: Some(&xtargets),
        };
        assert_eq!(
            pet_attack_action(&ctx),
            None,
            "no redundant attack command when pet is already on current target"
        );
    }

    // ── end charm / pet tests ──────────────────────────────────────────────

    #[test]
    fn enchanter_toml_level_60_rotation_order_and_thresholds_match_runtime() {
        let profile = ConfigProfile::load().profile_for_level(60);

        let names: Vec<&str> = profile
            .combat_abilities
            .iter()
            .map(|ability| ability.name.as_str())
            .collect();
        assert_eq!(
            names,
            vec![
                "Color Slant",
                "Tashanian",
                "Glamour of Kintaz",
                "Tashanian",
                "Dreary Deeds",
                "Dementia",
            ]
        );

        let priorities: Vec<u8> = profile
            .combat_abilities
            .iter()
            .map(|ability| ability.priority)
            .collect();
        assert_eq!(priorities, vec![1, 2, 3, 4, 5, 6]);

        let conditions: Vec<&str> = profile
            .combat_abilities
            .iter()
            .map(|ability| {
                ability
                    .condition
                    .as_deref()
                    .expect("each combat entry should declare a threshold")
            })
            .collect();
        assert_eq!(
            conditions,
            vec![
                "enemy_count_above_4 and mana_above_45",
                "enemy_count_above_2 and mana_above_35",
                "enemy_count_above_2 and mana_above_25",
                "target_hp_above_95 and mana_above_35",
                "target_hp_above_80 and mana_above_50",
                "mana_above_65",
            ]
        );
    }
}
