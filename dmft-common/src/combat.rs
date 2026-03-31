use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CombatStatus {
    Idle,
    Engaging { target_id: u32 },
    Casting { spell_slot: u8, target_id: u32 },
    OnGcd,
    Pulling { target_id: u32 },
    Recovering,
    Fleeing,
    Dead,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CombatRole {
    MainTank,
    OffTank,
    Healer,
    Puller,
    DpsMelee,
    DpsRanged,
    CrowdControl,
    Support,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PullMethod {
    SpellPull { spell_slot: u8 },
    BowPull,
    ProximityPull,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AssistMode {
    AssistTrain,
    SplitDps,
    Solo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpellEntry {
    pub slot: u8,
    /// EQ spell ID — used by CastSpell FFI. 0 = use whatever is memorized in slot.
    #[serde(default)]
    pub spell_id: i32,
    pub name: String,
    pub min_mana_pct: f32,
    pub priority: u8,
    pub is_aoe: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionExpr {
    And(Vec<ConditionExpr>),
    Or(Vec<ConditionExpr>),
    HpBelow(f32),
    ManaBelow(f32),
    TargetHpAbove(f32),
    TargetHpBelow(f32),
    AggroOnMe,
    Always,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolyShitCondition {
    pub priority: u8,
    pub condition: ConditionExpr,
    pub action: HolyShitAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HolyShitAction {
    CastSpell(u8),
    UseAbility(u32),
    UseItem(u32),
    Flee,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisciplineEntry {
    pub name: String,
    pub spell_id: i32,
    pub priority: u8,
    /// Cooldown in game ticks (~20 ticks/sec). Disciplines have long reuse timers.
    pub cooldown_ticks: u32,
    /// Minimum HP % to use this disc (e.g., Defensive only when < 50% HP)
    pub min_hp_pct: f32,
    /// Maximum HP % (e.g., don't waste Defensive at full HP)
    pub max_hp_pct: f32,
    /// Minimum endurance % required
    pub min_endurance_pct: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatConfig {
    pub role: CombatRole,
    pub pull_method: Option<PullMethod>,
    pub assist_mode: AssistMode,
    pub spells: Vec<SpellEntry>,
    pub disciplines: Vec<DisciplineEntry>,
    pub holyshit_rules: Vec<HolyShitCondition>,
    pub mana_floor: f32,
    pub aoe_threshold: u8,
}

impl Default for CombatConfig {
    fn default() -> Self {
        Self {
            role: CombatRole::DpsMelee,
            pull_method: None,
            assist_mode: AssistMode::AssistTrain,
            spells: Vec::new(),
            disciplines: Vec::new(),
            holyshit_rules: Vec::new(),
            mana_floor: 20.0,
            aoe_threshold: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combat_status_idle_variant() {
        let status = CombatStatus::Idle;
        assert!(matches!(status, CombatStatus::Idle));
    }

    #[test]
    fn combat_status_engaging_carries_target() {
        let status = CombatStatus::Engaging { target_id: 42 };
        if let CombatStatus::Engaging { target_id } = status {
            assert_eq!(target_id, 42);
        } else {
            panic!("expected Engaging");
        }
    }

    #[test]
    fn combat_role_equality() {
        assert_eq!(CombatRole::MainTank, CombatRole::MainTank);
        assert_ne!(CombatRole::MainTank, CombatRole::Healer);
        assert_ne!(CombatRole::Puller, CombatRole::DpsMelee);
    }

    #[test]
    fn combat_config_default() {
        let config = CombatConfig::default();
        assert_eq!(config.role, CombatRole::DpsMelee);
        assert!(config.pull_method.is_none());
        assert!(matches!(config.assist_mode, AssistMode::AssistTrain));
        assert!(config.spells.is_empty());
        assert!(config.disciplines.is_empty());
        assert!(config.holyshit_rules.is_empty());
        assert!((config.mana_floor - 20.0).abs() < f32::EPSILON);
        assert_eq!(config.aoe_threshold, 3);
    }

    #[test]
    fn combat_status_all_variants_constructible() {
        let _idle = CombatStatus::Idle;
        let _engaging = CombatStatus::Engaging { target_id: 1 };
        let _casting = CombatStatus::Casting {
            spell_slot: 0,
            target_id: 1,
        };
        let _gcd = CombatStatus::OnGcd;
        let _pulling = CombatStatus::Pulling { target_id: 1 };
        let _recovering = CombatStatus::Recovering;
        let _fleeing = CombatStatus::Fleeing;
        let _dead = CombatStatus::Dead;
    }

    #[test]
    fn combat_role_all_variants_constructible() {
        let _mt = CombatRole::MainTank;
        let _ot = CombatRole::OffTank;
        let _healer = CombatRole::Healer;
        let _puller = CombatRole::Puller;
        let _melee = CombatRole::DpsMelee;
        let _ranged = CombatRole::DpsRanged;
        let _cc = CombatRole::CrowdControl;
        let _support = CombatRole::Support;
    }
}
