//! Per-class combat rotation configuration used by scenario tests.

use std::time::Duration;

/// One action in a class combat rotation.
#[derive(Debug, Clone, PartialEq)]
pub enum CombatAction {
    /// A direct spell cast.
    Spell {
        /// Display name for metrics and combat events.
        name: String,
        /// Damage dealt when the cast completes.
        damage: u32,
        /// Simulated cast time.
        cast_time: Duration,
        /// Whether the hit should be recorded as critical.
        critical: bool,
    },
    /// A melee swing.
    Melee {
        /// Damage dealt when the swing lands.
        damage: u32,
        /// Simulated weapon delay.
        delay: Duration,
        /// Whether the hit should be recorded as critical.
        critical: bool,
    },
}

impl CombatAction {
    /// Construct a spell action.
    #[must_use]
    pub fn spell(
        name: impl Into<String>,
        damage: u32,
        cast_time: Duration,
        critical: bool,
    ) -> Self {
        Self::Spell {
            name: name.into(),
            damage,
            cast_time,
            critical,
        }
    }

    /// Construct a melee action.
    #[must_use]
    pub fn melee(damage: u32, delay: Duration, critical: bool) -> Self {
        Self::Melee {
            damage,
            delay,
            critical,
        }
    }

    /// Simulated duration of the action.
    #[must_use]
    pub fn duration(&self) -> Duration {
        match self {
            Self::Spell { cast_time, .. } => *cast_time,
            Self::Melee { delay, .. } => *delay,
        }
    }

    /// Damage dealt by the action when it completes.
    #[must_use]
    pub fn damage(&self) -> u32 {
        match self {
            Self::Spell { damage, .. } | Self::Melee { damage, .. } => *damage,
        }
    }

    /// Whether this action records a critical hit.
    #[must_use]
    pub fn is_critical(&self) -> bool {
        match self {
            Self::Spell { critical, .. } | Self::Melee { critical, .. } => *critical,
        }
    }

    /// Whether this action is a spell cast.
    #[must_use]
    pub fn is_spell(&self) -> bool {
        matches!(self, Self::Spell { .. })
    }

    /// Whether this action is a melee hit.
    #[must_use]
    pub fn is_melee(&self) -> bool {
        matches!(self, Self::Melee { .. })
    }

    /// Event name for spell actions.
    #[must_use]
    pub fn spell_name(&self) -> Option<&str> {
        match self {
            Self::Spell { name, .. } => Some(name.as_str()),
            Self::Melee { .. } => None,
        }
    }
}

/// Static configuration for a class rotation scenario.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassConfig {
    /// EQ class name, e.g. `Warrior` or `Wizard`.
    pub class_name: String,
    /// Spawn ID used as the damage source in combat data.
    pub source_id: u32,
    /// Ordered actions that make up one rotation.
    pub rotation: Vec<CombatAction>,
    /// Optional deterministic interrupt cadence for scenario validation.
    pub interrupt_every: Option<u32>,
}

impl ClassConfig {
    /// Construct a class config from an explicit rotation.
    #[must_use]
    pub fn new(class_name: impl Into<String>, source_id: u32, rotation: Vec<CombatAction>) -> Self {
        Self {
            class_name: class_name.into(),
            source_id,
            rotation,
            interrupt_every: None,
        }
    }

    /// Set a deterministic interrupt cadence.
    #[must_use]
    pub fn with_interrupt_every(mut self, interrupt_every: Option<u32>) -> Self {
        self.interrupt_every = interrupt_every;
        self
    }

    /// Baseline warrior rotation for validation scenarios.
    #[must_use]
    pub fn warrior(source_id: u32) -> Self {
        Self::new(
            "Warrior",
            source_id,
            vec![
                CombatAction::melee(85, Duration::from_millis(2400), false),
                CombatAction::melee(120, Duration::from_millis(2400), true),
            ],
        )
    }

    /// Baseline wizard rotation for validation scenarios.
    #[must_use]
    pub fn wizard(source_id: u32) -> Self {
        Self::new(
            "Wizard",
            source_id,
            vec![
                CombatAction::spell("Draught", 420, Duration::from_millis(3000), false),
                CombatAction::spell("Ice Comet", 780, Duration::from_millis(5500), true),
            ],
        )
    }

    /// Baseline cleric rotation for validation scenarios.
    #[must_use]
    pub fn cleric(source_id: u32) -> Self {
        Self::new(
            "Cleric",
            source_id,
            vec![
                CombatAction::spell("Smite", 160, Duration::from_millis(2500), false),
                CombatAction::melee(40, Duration::from_millis(2400), false),
            ],
        )
    }

    /// Total simulated time for one full rotation.
    #[must_use]
    pub fn rotation_duration(&self) -> Duration {
        self.rotation
            .iter()
            .fold(Duration::ZERO, |total, action| total + action.duration())
    }

    /// Validate that the configured rotation is appropriate for its class.
    pub fn validate(&self) -> Result<(), String> {
        if self.class_name.trim().is_empty() {
            return Err("class_name must not be empty".to_string());
        }
        if self.source_id == 0 {
            return Err("source_id must be non-zero".to_string());
        }
        if self.rotation.is_empty() {
            return Err(format!("{} rotation must not be empty", self.class_name));
        }
        if let Some(0) = self.interrupt_every {
            return Err("interrupt_every must be greater than zero".to_string());
        }

        for action in &self.rotation {
            if action.duration().is_zero() {
                return Err(format!(
                    "{} rotation contains a zero-duration action",
                    self.class_name
                ));
            }
            if action.damage() == 0 {
                return Err(format!(
                    "{} rotation contains a zero-damage action",
                    self.class_name
                ));
            }
        }

        match ClassArchetype::for_name(&self.class_name) {
            Some(ClassArchetype::Melee) if !self.rotation.iter().any(CombatAction::is_melee) => {
                Err(format!(
                    "{} rotation must include melee damage",
                    self.class_name
                ))
            }
            Some(ClassArchetype::Caster) if !self.rotation.iter().any(CombatAction::is_spell) => {
                Err(format!(
                    "{} rotation must include spell damage",
                    self.class_name
                ))
            }
            Some(_) => Ok(()),
            None => Err(format!("unsupported combat class: {}", self.class_name)),
        }
    }
}

/// Strategy abstraction consumed by combat scenarios.
pub trait ClassStrategy: Send + Sync {
    /// Static config backing this strategy.
    fn config(&self) -> &ClassConfig;

    /// Ordered rotation actions for one pass.
    fn rotation(&self) -> &[CombatAction] {
        &self.config().rotation
    }

    /// Validate the strategy before a scenario run.
    fn validate(&self) -> Result<(), String> {
        self.config().validate()
    }
}

/// Deterministic strategy used by test scenarios.
#[derive(Debug, Clone)]
pub struct StaticClassStrategy {
    config: ClassConfig,
}

impl StaticClassStrategy {
    /// Construct a strategy from static class configuration.
    #[must_use]
    pub fn new(config: ClassConfig) -> Self {
        Self { config }
    }
}

impl ClassStrategy for StaticClassStrategy {
    fn config(&self) -> &ClassConfig {
        &self.config
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClassArchetype {
    Melee,
    Caster,
    Hybrid,
}

impl ClassArchetype {
    fn for_name(class_name: &str) -> Option<Self> {
        match class_name.trim().to_ascii_lowercase().as_str() {
            "warrior" | "rogue" | "monk" | "berserker" => Some(Self::Melee),
            "cleric" | "druid" | "shaman" | "wizard" | "magician" | "mage" | "necromancer"
            | "enchanter" => Some(Self::Caster),
            "paladin" | "shadow knight" | "shadowknight" | "ranger" | "bard" | "beastlord" => {
                Some(Self::Hybrid)
            }
            _ => None,
        }
    }
}
