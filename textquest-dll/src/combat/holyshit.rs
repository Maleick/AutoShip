use textquest_common::combat::{ConditionExpr, HolyShitAction, HolyShitCondition};

use super::strategy::CombatContext;

/// Evaluates emergency conditional ability rules each tick.
/// `HolyShit` rules fire BEFORE normal rotation — they are the panic button.
pub struct HolyShitEvaluator {
    rules: Vec<HolyShitCondition>,
}

impl HolyShitEvaluator {
    pub fn new(mut rules: Vec<HolyShitCondition>) -> Self {
        // Sort by priority (lower number = higher priority).
        rules.sort_by_key(|r| r.priority);
        Self { rules }
    }

    /// Evaluate all rules against current context. Returns the first matching action.
    #[inline]
    pub fn evaluate(&self, ctx: &CombatContext) -> Option<&HolyShitAction> {
        for rule in &self.rules {
            if Self::eval_condition(&rule.condition, ctx) {
                tracing::info!(priority = rule.priority, "HolyShit rule fired");
                return Some(&rule.action);
            }
        }
        None
    }

    #[inline]
    fn eval_condition(expr: &ConditionExpr, ctx: &CombatContext) -> bool {
        // Delegate to the rotation engine's evaluate_condition for consistency.
        // HolyShit uses the same condition language as rotation entries.
        super::rotation::evaluate_condition(expr, ctx)
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::combat::{
        CombatConfig, ConditionExpr, HolyShitAction, HolyShitCondition,
    };
    use textquest_common::types::SpawnData;

    fn make_player(hp_pct_target: f32, mana_pct_target: f32) -> SpawnData {
        SpawnData {
            spawn_id: 1,
            name: "TestPlayer".into(),
            displayed_name: "TestPlayer".into(),
            spawn_type: 0,
            level: 60,
            class_id: 1,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current: (hp_pct_target * 100.0) as i64,
            hp_max: 10000,
            mana_current: (mana_pct_target * 100.0) as i32,
            mana_max: 10000,
            endurance_current: 100,
            endurance_max: 100,
            speed_run: 0.0,
            stand_state: 0,
        }
    }

    fn make_npc(hp_pct_target: f32) -> SpawnData {
        SpawnData {
            spawn_id: 100,
            name: "TestNPC".into(),
            displayed_name: "TestNPC".into(),
            spawn_type: 1,
            level: 50,
            class_id: 0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current: (hp_pct_target * 100.0) as i64,
            hp_max: 10000,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 0,
            endurance_max: 0,
            speed_run: 0.0,
            stand_state: 0,
        }
    }

    static DEFAULT_CONFIG: std::sync::LazyLock<CombatConfig> =
        std::sync::LazyLock::new(CombatConfig::default);

    fn make_context<'a>(player: &'a SpawnData, target: Option<&'a SpawnData>) -> CombatContext<'a> {
        CombatContext {
            player,
            target,
            nearby_enemies: &[],
            group_members: &[],
            config: &DEFAULT_CONFIG,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
        }
    }

    #[test]
    fn empty_rules_returns_none() {
        let evaluator = HolyShitEvaluator::new(vec![]);
        let player = make_player(50.0, 50.0);
        let ctx = make_context(&player, None);
        assert!(evaluator.evaluate(&ctx).is_none());
        assert_eq!(evaluator.rule_count(), 0);
    }

    #[test]
    fn single_matching_rule_returns_action() {
        let rules = vec![HolyShitCondition {
            priority: 1,
            condition: ConditionExpr::HpBelow(30.0),
            action: HolyShitAction::Flee,
        }];
        let evaluator = HolyShitEvaluator::new(rules);

        let player = make_player(20.0, 100.0);
        let ctx = make_context(&player, None);
        let result = evaluator.evaluate(&ctx);
        assert!(matches!(result, Some(HolyShitAction::Flee)));
    }

    #[test]
    fn non_matching_rule_returns_none() {
        let rules = vec![HolyShitCondition {
            priority: 1,
            condition: ConditionExpr::HpBelow(30.0),
            action: HolyShitAction::Flee,
        }];
        let evaluator = HolyShitEvaluator::new(rules);

        let player = make_player(50.0, 100.0);
        let ctx = make_context(&player, None);
        assert!(evaluator.evaluate(&ctx).is_none());
    }

    #[test]
    fn priority_ordering_lower_number_first() {
        let rules = vec![
            HolyShitCondition {
                priority: 10,
                condition: ConditionExpr::Always,
                action: HolyShitAction::CastSpell(2),
            },
            HolyShitCondition {
                priority: 1,
                condition: ConditionExpr::Always,
                action: HolyShitAction::CastSpell(1),
            },
        ];
        let evaluator = HolyShitEvaluator::new(rules);

        let player = make_player(100.0, 100.0);
        let ctx = make_context(&player, None);
        let result = evaluator.evaluate(&ctx);
        assert!(matches!(result, Some(HolyShitAction::CastSpell(1))));
    }

    #[test]
    fn and_conditions_all_must_match() {
        let rules = vec![HolyShitCondition {
            priority: 1,
            condition: ConditionExpr::And(vec![
                ConditionExpr::HpBelow(50.0),
                ConditionExpr::ManaBelow(20.0),
            ]),
            action: HolyShitAction::Flee,
        }];
        let evaluator = HolyShitEvaluator::new(rules);

        // Both conditions met
        let player = make_player(30.0, 10.0);
        let ctx = make_context(&player, None);
        assert!(evaluator.evaluate(&ctx).is_some());

        // Only HP condition met
        let player2 = make_player(30.0, 80.0);
        let ctx2 = make_context(&player2, None);
        assert!(evaluator.evaluate(&ctx2).is_none());
    }

    #[test]
    fn or_conditions_any_must_match() {
        let rules = vec![HolyShitCondition {
            priority: 1,
            condition: ConditionExpr::Or(vec![
                ConditionExpr::HpBelow(20.0),
                ConditionExpr::ManaBelow(10.0),
            ]),
            action: HolyShitAction::Flee,
        }];
        let evaluator = HolyShitEvaluator::new(rules);

        // Only mana condition met
        let player = make_player(80.0, 5.0);
        let ctx = make_context(&player, None);
        assert!(evaluator.evaluate(&ctx).is_some());

        // Neither met
        let player2 = make_player(80.0, 80.0);
        let ctx2 = make_context(&player2, None);
        assert!(evaluator.evaluate(&ctx2).is_none());
    }

    #[test]
    fn target_hp_conditions() {
        let rules = vec![HolyShitCondition {
            priority: 1,
            condition: ConditionExpr::TargetHpBelow(25.0),
            action: HolyShitAction::UseAbility(100),
        }];
        let evaluator = HolyShitEvaluator::new(rules);

        let player = make_player(100.0, 100.0);
        let npc = make_npc(20.0);
        let ctx = make_context(&player, Some(&npc));
        assert!(evaluator.evaluate(&ctx).is_some());

        // No target
        let ctx_no_target = make_context(&player, None);
        assert!(evaluator.evaluate(&ctx_no_target).is_none());
    }

    #[test]
    fn aggro_on_me_checks_npc_target() {
        let rules = vec![HolyShitCondition {
            priority: 1,
            condition: ConditionExpr::AggroOnMe,
            action: HolyShitAction::Flee,
        }];
        let evaluator = HolyShitEvaluator::new(rules);

        let player = make_player(100.0, 100.0);
        let npc = make_npc(80.0); // spawn_type == 1 (NPC)
        let ctx = make_context(&player, Some(&npc));
        assert!(evaluator.evaluate(&ctx).is_some());

        // Target is a player (spawn_type 0), not an NPC
        let other_player = make_player(80.0, 80.0);
        let ctx2 = make_context(&player, Some(&other_player));
        assert!(evaluator.evaluate(&ctx2).is_none());
    }

    #[test]
    fn always_condition_fires_immediately() {
        let rules = vec![HolyShitCondition {
            priority: 1,
            condition: ConditionExpr::Always,
            action: HolyShitAction::CastSpell(5),
        }];
        let evaluator = HolyShitEvaluator::new(rules);
        let player = make_player(100.0, 100.0);
        let ctx = make_context(&player, None);
        assert!(matches!(
            evaluator.evaluate(&ctx),
            Some(HolyShitAction::CastSpell(5))
        ));
    }

    #[test]
    fn mana_below_condition() {
        let rules = vec![HolyShitCondition {
            priority: 1,
            condition: ConditionExpr::ManaBelow(30.0),
            action: HolyShitAction::UseAbility(99),
        }];
        let evaluator = HolyShitEvaluator::new(rules);

        // Low mana
        let player = make_player(100.0, 15.0);
        let ctx = make_context(&player, None);
        assert!(evaluator.evaluate(&ctx).is_some());

        // High mana
        let player2 = make_player(100.0, 80.0);
        let ctx2 = make_context(&player2, None);
        assert!(evaluator.evaluate(&ctx2).is_none());
    }

    #[test]
    fn target_hp_above_condition() {
        let rules = vec![HolyShitCondition {
            priority: 1,
            condition: ConditionExpr::TargetHpAbove(80.0),
            action: HolyShitAction::CastSpell(10),
        }];
        let evaluator = HolyShitEvaluator::new(rules);

        let player = make_player(100.0, 100.0);
        let npc = make_npc(90.0);
        let ctx = make_context(&player, Some(&npc));
        assert!(evaluator.evaluate(&ctx).is_some());

        let weak_npc = make_npc(50.0);
        let ctx2 = make_context(&player, Some(&weak_npc));
        assert!(evaluator.evaluate(&ctx2).is_none());
    }

    #[test]
    fn nested_and_or_conditions() {
        let rules = vec![HolyShitCondition {
            priority: 1,
            condition: ConditionExpr::Or(vec![
                ConditionExpr::And(vec![
                    ConditionExpr::HpBelow(20.0),
                    ConditionExpr::ManaBelow(20.0),
                ]),
                ConditionExpr::HpBelow(5.0),
            ]),
            action: HolyShitAction::Flee,
        }];
        let evaluator = HolyShitEvaluator::new(rules);

        // hp=3%, mana=80% -> second Or branch (HpBelow(5)) triggers
        let player = make_player(3.0, 80.0);
        let ctx = make_context(&player, None);
        assert!(evaluator.evaluate(&ctx).is_some());

        // hp=15%, mana=15% -> first And branch triggers
        let player2 = make_player(15.0, 15.0);
        let ctx2 = make_context(&player2, None);
        assert!(evaluator.evaluate(&ctx2).is_some());

        // hp=15%, mana=80% -> neither branch
        let player3 = make_player(15.0, 80.0);
        let ctx3 = make_context(&player3, None);
        assert!(evaluator.evaluate(&ctx3).is_none());
    }

    #[test]
    fn rule_count() {
        let rules = vec![
            HolyShitCondition {
                priority: 1,
                condition: ConditionExpr::Always,
                action: HolyShitAction::Flee,
            },
            HolyShitCondition {
                priority: 2,
                condition: ConditionExpr::Always,
                action: HolyShitAction::Flee,
            },
        ];
        let evaluator = HolyShitEvaluator::new(rules);
        assert_eq!(evaluator.rule_count(), 2);
    }

    #[test]
    fn first_matching_rule_wins() {
        // Two rules, both match, but priority 1 should fire first
        let rules = vec![
            HolyShitCondition {
                priority: 1,
                condition: ConditionExpr::HpBelow(50.0),
                action: HolyShitAction::CastSpell(1),
            },
            HolyShitCondition {
                priority: 2,
                condition: ConditionExpr::HpBelow(80.0),
                action: HolyShitAction::CastSpell(2),
            },
        ];
        let evaluator = HolyShitEvaluator::new(rules);
        let player = make_player(30.0, 100.0);
        let ctx = make_context(&player, None);
        assert!(matches!(
            evaluator.evaluate(&ctx),
            Some(HolyShitAction::CastSpell(1))
        ));
    }

    #[test]
    fn aggro_on_me_no_target_returns_false() {
        let rules = vec![HolyShitCondition {
            priority: 1,
            condition: ConditionExpr::AggroOnMe,
            action: HolyShitAction::Flee,
        }];
        let evaluator = HolyShitEvaluator::new(rules);
        let player = make_player(100.0, 100.0);
        let ctx = make_context(&player, None);
        assert!(evaluator.evaluate(&ctx).is_none());
    }

    #[test]
    fn use_ability_action_variant() {
        let rules = vec![HolyShitCondition {
            priority: 1,
            condition: ConditionExpr::Always,
            action: HolyShitAction::UseAbility(42),
        }];
        let evaluator = HolyShitEvaluator::new(rules);
        let player = make_player(100.0, 100.0);
        let ctx = make_context(&player, None);
        assert!(matches!(
            evaluator.evaluate(&ctx),
            Some(HolyShitAction::UseAbility(42))
        ));
    }
}
