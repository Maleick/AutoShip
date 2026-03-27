use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CombatStatus {
    Idle,
    Engaging { target_id: u32 },
    Casting { spell_slot: u8, target_id: u32 },
    OnGcd,
    Pulling { target_id: u32 },
    Recovering,
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
pub struct CombatConfig {
    pub role: CombatRole,
    pub pull_method: Option<PullMethod>,
    pub assist_mode: AssistMode,
    pub spells: Vec<SpellEntry>,
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
            holyshit_rules: Vec::new(),
            mana_floor: 20.0,
            aoe_threshold: 3,
        }
    }
}
