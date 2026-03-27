use dmft_common::combat::{ConditionExpr, HolyShitAction, HolyShitCondition};

use super::strategy::CombatContext;

/// Evaluates emergency conditional ability rules each tick.
/// HolyShit rules fire BEFORE normal rotation — they are the panic button.
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
    pub fn evaluate(&self, ctx: &CombatContext) -> Option<&HolyShitAction> {
        for rule in &self.rules {
            if Self::eval_condition(&rule.condition, ctx) {
                tracing::info!(priority = rule.priority, "HolyShit rule fired");
                return Some(&rule.action);
            }
        }
        None
    }

    fn eval_condition(expr: &ConditionExpr, ctx: &CombatContext) -> bool {
        match expr {
            ConditionExpr::Always => true,
            ConditionExpr::HpBelow(threshold) => ctx.player.hp_pct() < *threshold,
            ConditionExpr::ManaBelow(threshold) => ctx.player.mana_pct() < *threshold,
            ConditionExpr::TargetHpAbove(threshold) => {
                ctx.target.is_some_and(|t| t.hp_pct() > *threshold)
            }
            ConditionExpr::TargetHpBelow(threshold) => {
                ctx.target.is_some_and(|t| t.hp_pct() < *threshold)
            }
            ConditionExpr::AggroOnMe => {
                // Simplified: check if target is facing us (uses aggro module)
                // For now, approximate as "target exists and is NPC"
                ctx.target.map_or(false, |t| t.spawn_type == 1) // NPC type
            }
            ConditionExpr::And(conditions) => {
                conditions.iter().all(|c| Self::eval_condition(c, ctx))
            }
            ConditionExpr::Or(conditions) => {
                conditions.iter().any(|c| Self::eval_condition(c, ctx))
            }
        }
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}
