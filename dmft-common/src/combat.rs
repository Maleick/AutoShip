use serde::{Deserialize, Serialize};

/// Current state of a character in the combat FSM.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CombatStatus {
    /// Not in combat, no pending actions.
    Idle,
    /// Actively attacking a target.
    Engaging {
        /// Spawn ID of the mob being engaged.
        target_id: u32,
    },
    /// Casting a spell on a target.
    Casting {
        /// Memorized spell slot (0-indexed).
        spell_slot: u8,
        /// Spawn ID of the cast target.
        target_id: u32,
    },
    /// Waiting for the global cooldown to expire.
    OnGcd,
    /// Pulling a mob back to camp.
    Pulling {
        /// Spawn ID of the mob being pulled.
        target_id: u32,
    },
    /// Post-combat recovery (regen, med, rebuff).
    Recovering,
    /// Running away from danger.
    Fleeing,
    /// Character is dead, awaiting resurrect or respawn.
    Dead,
}

/// Role a character fills in the group combat formation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CombatRole {
    /// Primary tank — holds aggro on the main target.
    MainTank,
    /// Secondary tank — picks up adds or swap targets.
    OffTank,
    /// Healer — keeps the group alive.
    Healer,
    /// Puller — brings mobs to camp.
    Puller,
    /// Melee DPS — fights in melee range.
    DpsMelee,
    /// Ranged DPS — nukes or uses ranged attacks.
    DpsRanged,
    /// Crowd control — mezzes, roots, or charms adds.
    CrowdControl,
    /// Support — buffs, debuffs, utility (bard, shaman, etc.).
    Support,
}

/// How the puller initiates a pull.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PullMethod {
    /// Pull with a spell from the given memorized slot.
    SpellPull {
        /// Memorized spell slot to cast.
        spell_slot: u8,
    },
    /// Pull with a bow/ranged attack.
    BowPull,
    /// Walk close enough for the mob to aggro.
    ProximityPull,
}

/// How multiple DPS characters divide their targets.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AssistMode {
    /// Everyone assists the main assist target.
    AssistTrain,
    /// DPS split across multiple targets.
    SplitDps,
    /// Each character picks its own target.
    Solo,
}

/// A memorized spell available for the combat rotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpellEntry {
    /// Memorized spell slot (0-indexed gem number).
    pub slot: u8,
    /// EQ spell ID — used by `CastSpell` FFI. 0 = use whatever is memorized in slot.
    #[serde(default)]
    pub spell_id: i32,
    /// Human-readable spell name for logging/config.
    pub name: String,
    /// Minimum mana % required to cast this spell.
    pub min_mana_pct: f32,
    /// Priority in the rotation (lower = higher priority).
    pub priority: u8,
    /// Whether this spell is area-of-effect.
    pub is_aoe: bool,
}

/// Boolean expression tree for evaluating combat conditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionExpr {
    /// All sub-conditions must be true.
    And(Vec<ConditionExpr>),
    /// At least one sub-condition must be true.
    Or(Vec<ConditionExpr>),
    /// Character HP is below the given percentage.
    HpBelow(f32),
    /// Character mana is below the given percentage.
    ManaBelow(f32),
    /// Current target HP is above the given percentage.
    TargetHpAbove(f32),
    /// Current target HP is below the given percentage.
    TargetHpBelow(f32),
    /// The character has aggro from a mob.
    AggroOnMe,
    /// Always true — unconditional trigger.
    Always,
}

/// An emergency reaction rule that fires when conditions are met.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolyShitCondition {
    /// Evaluation priority (lower = checked first).
    pub priority: u8,
    /// Boolean condition tree that triggers this rule.
    pub condition: ConditionExpr,
    /// Action to take when the condition is true.
    pub action: HolyShitAction,
}

/// Emergency action to execute when a HolyShit condition fires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HolyShitAction {
    /// Cast a spell from the given memorized slot.
    CastSpell(u8),
    /// Use a combat ability by ID.
    UseAbility(u32),
    /// Use an inventory item by ID.
    UseItem(u32),
    /// Run away from combat.
    Flee,
}

/// A warrior/monk/etc discipline (activated combat ability with a reuse timer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisciplineEntry {
    /// Human-readable discipline name.
    pub name: String,
    /// EQ spell ID for the discipline.
    pub spell_id: i32,
    /// Priority relative to other disciplines (lower = higher priority).
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

/// Per-character combat configuration (role, spells, rules).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatConfig {
    /// This character's combat role.
    pub role: CombatRole,
    /// How this character pulls (if puller role).
    pub pull_method: Option<PullMethod>,
    /// How DPS targets are divided.
    pub assist_mode: AssistMode,
    /// Spell rotation entries.
    pub spells: Vec<SpellEntry>,
    /// Discipline rotation entries.
    pub disciplines: Vec<DisciplineEntry>,
    /// Emergency reaction rules evaluated each tick.
    pub holyshit_rules: Vec<HolyShitCondition>,
    /// Mana % floor — stop casting below this.
    pub mana_floor: f32,
    /// Minimum mob count to trigger AoE spells.
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

/// Whether a character is ready to cast a spell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CastReadiness {
    /// Ready to cast — no cooldowns or active casts.
    Ready,
    /// Currently casting a spell.
    Casting,
    /// Waiting for the global cooldown to expire.
    GlobalCooldown,
    /// Post-cast recovery window.
    Recovering,
    /// Spell is not memorized in any gem slot.
    NotMemorized,
}

impl std::fmt::Display for CastReadiness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ready => write!(f, "Ready"),
            Self::Casting => write!(f, "Casting"),
            Self::GlobalCooldown => write!(f, "GCD"),
            Self::Recovering => write!(f, "Recovering"),
            Self::NotMemorized => write!(f, "Not Memorized"),
        }
    }
}

/// Outcome of a cast attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CastResult {
    /// Spell landed successfully.
    Success,
    /// Cast was interrupted (took damage, moved, etc.).
    Interrupted,
    /// Spell fizzled (failed skill check).
    Fizzled,
    /// Target was out of range.
    OutOfRange,
    /// Not enough mana to cast.
    OutOfMana,
    /// Target is immune to this spell.
    Immune,
    /// Target resisted the spell.
    Resisted,
    /// Caster was not ready (GCD, already casting, etc.).
    NotReady,
}

impl std::fmt::Display for CastResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "Success"),
            Self::Interrupted => write!(f, "Interrupted"),
            Self::Fizzled => write!(f, "Fizzled"),
            Self::OutOfRange => write!(f, "Out of Range"),
            Self::OutOfMana => write!(f, "Out of Mana"),
            Self::Immune => write!(f, "Immune"),
            Self::Resisted => write!(f, "Resisted"),
            Self::NotReady => write!(f, "Not Ready"),
        }
    }
}

/// Telemetry snapshot for a single cast attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CastTelemetry {
    /// Outcome of the cast.
    pub result: CastResult,
    /// Wall-clock duration of the cast in milliseconds.
    pub cast_duration_ms: u64,
    /// EQ spell ID that was cast.
    pub spell_id: i32,
    /// Spawn ID of the cast target (0 = self/none).
    pub target_id: u32,
    /// Unix timestamp (milliseconds) when the cast completed.
    pub timestamp_ms: u64,
}

impl std::fmt::Display for CastTelemetry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "spell={} target={} result={} duration={}ms",
            self.spell_id, self.target_id, self.result, self.cast_duration_ms
        )
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

    #[test]
    fn cast_readiness_all_variants_constructible() {
        let variants = [
            CastReadiness::Ready,
            CastReadiness::Casting,
            CastReadiness::GlobalCooldown,
            CastReadiness::Recovering,
            CastReadiness::NotMemorized,
        ];
        assert_eq!(variants.len(), 5);
    }

    #[test]
    fn cast_readiness_display() {
        assert_eq!(CastReadiness::Ready.to_string(), "Ready");
        assert_eq!(CastReadiness::Casting.to_string(), "Casting");
        assert_eq!(CastReadiness::GlobalCooldown.to_string(), "GCD");
        assert_eq!(CastReadiness::Recovering.to_string(), "Recovering");
        assert_eq!(CastReadiness::NotMemorized.to_string(), "Not Memorized");
    }

    #[test]
    fn cast_readiness_serialization_roundtrip() {
        let variants = [
            CastReadiness::Ready,
            CastReadiness::Casting,
            CastReadiness::GlobalCooldown,
            CastReadiness::Recovering,
            CastReadiness::NotMemorized,
        ];
        for variant in &variants {
            let json = serde_json::to_string(variant).expect("serialize");
            let restored: CastReadiness = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*variant, restored);
        }
    }

    #[test]
    fn cast_result_all_variants_constructible() {
        let variants = [
            CastResult::Success,
            CastResult::Interrupted,
            CastResult::Fizzled,
            CastResult::OutOfRange,
            CastResult::OutOfMana,
            CastResult::Immune,
            CastResult::Resisted,
            CastResult::NotReady,
        ];
        assert_eq!(variants.len(), 8);
    }

    #[test]
    fn cast_result_display() {
        assert_eq!(CastResult::Success.to_string(), "Success");
        assert_eq!(CastResult::Interrupted.to_string(), "Interrupted");
        assert_eq!(CastResult::Fizzled.to_string(), "Fizzled");
        assert_eq!(CastResult::OutOfRange.to_string(), "Out of Range");
        assert_eq!(CastResult::OutOfMana.to_string(), "Out of Mana");
        assert_eq!(CastResult::Immune.to_string(), "Immune");
        assert_eq!(CastResult::Resisted.to_string(), "Resisted");
        assert_eq!(CastResult::NotReady.to_string(), "Not Ready");
    }

    #[test]
    fn cast_result_serialization_roundtrip() {
        let variants = [
            CastResult::Success,
            CastResult::Interrupted,
            CastResult::Fizzled,
            CastResult::OutOfRange,
            CastResult::OutOfMana,
            CastResult::Immune,
            CastResult::Resisted,
            CastResult::NotReady,
        ];
        for variant in &variants {
            let json = serde_json::to_string(variant).expect("serialize");
            let restored: CastResult = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*variant, restored);
        }
    }

    #[test]
    fn cast_telemetry_construction_and_display() {
        let telemetry = CastTelemetry {
            result: CastResult::Success,
            cast_duration_ms: 2500,
            spell_id: 12345,
            target_id: 42,
            timestamp_ms: 1700000000000,
        };
        assert_eq!(telemetry.result, CastResult::Success);
        assert_eq!(telemetry.cast_duration_ms, 2500);
        assert_eq!(telemetry.spell_id, 12345);
        assert_eq!(telemetry.target_id, 42);
        let display = telemetry.to_string();
        assert!(display.contains("spell=12345"));
        assert!(display.contains("target=42"));
        assert!(display.contains("result=Success"));
        assert!(display.contains("duration=2500ms"));
    }

    #[test]
    fn cast_telemetry_serialization_roundtrip() {
        let telemetry = CastTelemetry {
            result: CastResult::Interrupted,
            cast_duration_ms: 1200,
            spell_id: 9999,
            target_id: 100,
            timestamp_ms: 1700000001000,
        };
        let json = serde_json::to_string(&telemetry).expect("serialize");
        let restored: CastTelemetry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(telemetry, restored);
    }

    #[test]
    fn cast_readiness_equality_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(CastReadiness::Ready);
        set.insert(CastReadiness::Ready);
        set.insert(CastReadiness::Casting);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn cast_result_equality_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(CastResult::Success);
        set.insert(CastResult::Success);
        set.insert(CastResult::Fizzled);
        assert_eq!(set.len(), 2);
    }
}
