use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

    #[test]
    fn condition_expr_all_variants_constructible() {
        let _and = ConditionExpr::And(vec![ConditionExpr::Always]);
        let _or = ConditionExpr::Or(vec![ConditionExpr::AggroOnMe]);
        let _hp = ConditionExpr::HpBelow(50.0);
        let _mana = ConditionExpr::ManaBelow(20.0);
        let _thp_above = ConditionExpr::TargetHpAbove(90.0);
        let _thp_below = ConditionExpr::TargetHpBelow(10.0);
        let _aggro = ConditionExpr::AggroOnMe;
        let _always = ConditionExpr::Always;
    }

    #[test]
    fn condition_expr_nested_and_or() {
        let expr = ConditionExpr::And(vec![
            ConditionExpr::HpBelow(30.0),
            ConditionExpr::Or(vec![
                ConditionExpr::AggroOnMe,
                ConditionExpr::TargetHpBelow(20.0),
            ]),
        ]);
        // Just verify construction and debug formatting
        let debug = format!("{:?}", expr);
        assert!(debug.contains("And"));
        assert!(debug.contains("Or"));
    }

    #[test]
    fn holyshit_condition_construction() {
        let cond = HolyShitCondition {
            priority: 1,
            condition: ConditionExpr::HpBelow(20.0),
            action: HolyShitAction::Flee,
        };
        assert_eq!(cond.priority, 1);
        assert!(matches!(cond.action, HolyShitAction::Flee));
    }

    #[test]
    fn holyshit_action_all_variants() {
        let actions = [
            HolyShitAction::CastSpell(3),
            HolyShitAction::UseAbility(100),
            HolyShitAction::UseItem(5001),
            HolyShitAction::Flee,
        ];
        assert_eq!(actions.len(), 4);
    }

    #[test]
    fn pull_method_all_variants() {
        let _spell = PullMethod::SpellPull { spell_slot: 1 };
        let _bow = PullMethod::BowPull;
        let _prox = PullMethod::ProximityPull;
    }

    #[test]
    fn assist_mode_all_variants() {
        let _train = AssistMode::AssistTrain;
        let _split = AssistMode::SplitDps;
        let _solo = AssistMode::Solo;
    }

    #[test]
    fn spell_entry_construction() {
        let entry = SpellEntry {
            slot: 1,
            spell_id: 12345,
            name: "Complete Heal".into(),
            min_mana_pct: 30.0,
            priority: 1,
            is_aoe: false,
        };
        assert_eq!(entry.slot, 1);
        assert_eq!(entry.spell_id, 12345);
        assert_eq!(entry.name, "Complete Heal");
        assert!(!entry.is_aoe);
    }

    #[test]
    fn spell_entry_default_spell_id() {
        // spell_id has #[serde(default)] so should deserialize as 0 when missing
        let json = r#"{"slot":1,"name":"Test","min_mana_pct":10.0,"priority":1,"is_aoe":false}"#;
        let entry: SpellEntry = serde_json::from_str(json).expect("deserialize");
        assert_eq!(entry.spell_id, 0);
    }

    #[test]
    fn discipline_entry_construction() {
        let disc = DisciplineEntry {
            name: "Defensive".into(),
            spell_id: 9999,
            priority: 1,
            cooldown_ticks: 6000,
            min_hp_pct: 0.0,
            max_hp_pct: 50.0,
            min_endurance_pct: 10.0,
        };
        assert_eq!(disc.name, "Defensive");
        assert_eq!(disc.cooldown_ticks, 6000);
        assert!((disc.max_hp_pct - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn combat_config_serialization_roundtrip() {
        let config = CombatConfig {
            role: CombatRole::MainTank,
            pull_method: Some(PullMethod::BowPull),
            assist_mode: AssistMode::AssistTrain,
            spells: vec![SpellEntry {
                slot: 1,
                spell_id: 100,
                name: "Taunt".into(),
                min_mana_pct: 0.0,
                priority: 1,
                is_aoe: false,
            }],
            disciplines: vec![],
            holyshit_rules: vec![HolyShitCondition {
                priority: 1,
                condition: ConditionExpr::HpBelow(20.0),
                action: HolyShitAction::Flee,
            }],
            mana_floor: 15.0,
            aoe_threshold: 5,
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let restored: CombatConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.role, CombatRole::MainTank);
        assert!(restored.pull_method.is_some());
        assert_eq!(restored.spells.len(), 1);
        assert_eq!(restored.holyshit_rules.len(), 1);
        assert_eq!(restored.aoe_threshold, 5);
    }

    #[test]
    fn combat_status_serialization_roundtrip() {
        let statuses = [
            CombatStatus::Idle,
            CombatStatus::Engaging { target_id: 42 },
            CombatStatus::Casting {
                spell_slot: 3,
                target_id: 100,
            },
            CombatStatus::OnGcd,
            CombatStatus::Pulling { target_id: 7 },
            CombatStatus::Recovering,
            CombatStatus::Fleeing,
            CombatStatus::Dead,
        ];
        for status in &statuses {
            let json = serde_json::to_string(status).expect("serialize");
            let restored: CombatStatus = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*status, restored);
        }
    }
}
