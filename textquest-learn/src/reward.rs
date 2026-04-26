//! Reward signal modeling and transformation.
//! L-2: Reward shaping for behavioral learning.
//!
//! Operators author YAML reward specs; the binary validates and evaluates.
//! Each term is weighted and summed, then clamped to [-1, 1] per tick.
//! Ban-risk is always a penalty term (hard constraint).

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Known signal names that can appear in reward specs.
/// Matches ledger telemetry and anti-cheat signals.
pub const KNOWN_SIGNALS: &[&str] = &[
    "party_alive_fraction",
    "mana_per_effective_heal",
    "overheal_fraction",
    "antidetect.risk_score",
    "encounter_throughput",
    "party_survival_time",
    "mitigation_ratio",
    "aggro_retention",
    "kills_per_hour",
    "downtime_penalty",
    "time_to_ready",
];

/// Single reward term with weight and signal name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardTerm {
    pub name: String,
    pub weight: f32,
    pub signal: String,
}

impl RewardTerm {
    fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("term name cannot be empty"));
        }
        if !KNOWN_SIGNALS.contains(&self.signal.as_str()) {
            return Err(anyhow!(
                "unknown signal '{}' (known: {})",
                self.signal,
                KNOWN_SIGNALS.join(", ")
            ));
        }
        if !self.weight.is_finite() {
            return Err(anyhow!("term weight must be finite"));
        }
        Ok(())
    }
}

/// Complete reward specification, versioned and operator-authored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardConfig {
    pub id: String,
    pub version: u32,
    pub description: String,
    pub target_class: String,
    pub target_camp: String,
    pub terms: Vec<RewardTerm>,
    #[serde(default = "default_clamp")]
    pub clamp: [f32; 2],
}

fn default_clamp() -> [f32; 2] {
    [-1.0, 1.0]
}

impl RewardConfig {
    /// Validate the entire config against schema rules.
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(anyhow!("reward config id cannot be empty"));
        }
        if self.terms.is_empty() {
            return Err(anyhow!("reward config must have at least one term"));
        }

        // Check clamp inversion
        if self.clamp[0] >= self.clamp[1] {
            return Err(anyhow!(
                "clamp range invalid: [{}, {}] must satisfy min < max",
                self.clamp[0],
                self.clamp[1]
            ));
        }

        // Validate each term
        let mut seen_names = std::collections::HashSet::new();
        for term in &self.terms {
            term.validate()?;
            if !seen_names.insert(&term.name) {
                return Err(anyhow!("duplicate term name: '{}'", term.name));
            }
        }

        // Ban-risk term must exist (hard requirement)
        let has_ban_risk = self
            .terms
            .iter()
            .any(|t| t.signal == "antidetect.risk_score");
        if !has_ban_risk {
            return Err(anyhow!(
                "ban-risk term (antidetect.risk_score) is mandatory in all specs"
            ));
        }

        Ok(())
    }

    /// Parse YAML string into RewardConfig and validate.
    pub fn from_yaml(yaml_str: &str) -> Result<Self> {
        let config: RewardConfig = serde_yaml::from_str(yaml_str)?;
        config.validate()?;
        Ok(config)
    }

    /// Evaluate reward given a signal mapping (signal_name -> value).
    /// Returns clamped reward in range [clamp.0, clamp.1].
    pub fn evaluate(&self, signals: &HashMap<String, f32>) -> Result<f32> {
        let mut total = 0.0;
        for term in &self.terms {
            let signal_val = signals
                .get(&term.signal)
                .copied()
                .ok_or_else(|| anyhow!("missing signal '{}' in state", term.signal))?;
            total += term.weight * signal_val;
        }
        // Clamp to range
        Ok(total.clamp(self.clamp[0], self.clamp[1]))
    }
}

/// Registry of reward configs by (class, camp).
pub struct RewardRegistry {
    configs: HashMap<(String, String), RewardConfig>,
}

impl RewardRegistry {
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
        }
    }

    /// Register a reward config.
    pub fn register(&mut self, config: RewardConfig) -> Result<()> {
        config.validate()?;
        let key = (config.target_class.clone(), config.target_camp.clone());
        self.configs.insert(key, config);
        Ok(())
    }

    /// Look up reward config by class and camp.
    /// camp "*" is a fallback wildcard.
    pub fn get(&self, class: &str, camp: &str) -> Option<RewardConfig> {
        // Try exact match first
        if let Some(cfg) = self.configs.get(&(class.to_string(), camp.to_string())) {
            return Some(cfg.clone());
        }
        // Fall back to wildcard
        self.configs
            .get(&(class.to_string(), "*".to_string()))
            .cloned()
    }

    /// Load all starter specs into registry.
    pub fn load_starters() -> Result<Self> {
        let mut reg = Self::new();
        for spec in STARTER_SPECS {
            let cfg = RewardConfig::from_yaml(spec)?;
            reg.register(cfg)?;
        }
        Ok(reg)
    }
}

impl Default for RewardRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Starter reward specs shipped with the binary.
pub const STARTER_SPECS: &[&str] = &[
    include_str!("../rewards/combat.dps.generic.v1.yaml"),
    include_str!("../rewards/combat.heal.cleric.v1.yaml"),
    include_str!("../rewards/combat.tank.warrior.v1.yaml"),
    include_str!("../rewards/camp.throughput.v1.yaml"),
    include_str!("../rewards/recovery.med.v1.yaml"),
];

/// Marker trait for reward functions (legacy compat).
pub trait RewardFn: Send + Sync {
    /// Compute reward signal.
    fn reward(&self, _state: &[u8]) -> f32 {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reward_fn_marker_exists() {
        struct _DummyReward;
        impl RewardFn for _DummyReward {}
    }

    #[test]
    fn validate_rejects_empty_id() {
        let cfg = RewardConfig {
            id: String::new(),
            version: 1,
            description: "test".into(),
            target_class: "cleric".into(),
            target_camp: "*".into(),
            terms: vec![RewardTerm {
                name: "ban_risk".into(),
                weight: -1.0,
                signal: "antidetect.risk_score".into(),
            }],
            clamp: [-1.0, 1.0],
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_rejects_unknown_signal() {
        let cfg = RewardConfig {
            id: "test".into(),
            version: 1,
            description: "test".into(),
            target_class: "cleric".into(),
            target_camp: "*".into(),
            terms: vec![RewardTerm {
                name: "fake".into(),
                weight: 0.5,
                signal: "nonexistent_signal".into(),
            }],
            clamp: [-1.0, 1.0],
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_rejects_clamp_inversion() {
        let cfg = RewardConfig {
            id: "test".into(),
            version: 1,
            description: "test".into(),
            target_class: "cleric".into(),
            target_camp: "*".into(),
            terms: vec![RewardTerm {
                name: "ban_risk".into(),
                weight: -1.0,
                signal: "antidetect.risk_score".into(),
            }],
            clamp: [1.0, -1.0], // inverted
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_rejects_duplicate_term_names() {
        let cfg = RewardConfig {
            id: "test".into(),
            version: 1,
            description: "test".into(),
            target_class: "cleric".into(),
            target_camp: "*".into(),
            terms: vec![
                RewardTerm {
                    name: "foo".into(),
                    weight: 0.5,
                    signal: "party_alive_fraction".into(),
                },
                RewardTerm {
                    name: "foo".into(),
                    weight: -0.1,
                    signal: "antidetect.risk_score".into(),
                },
            ],
            clamp: [-1.0, 1.0],
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_requires_ban_risk_term() {
        let cfg = RewardConfig {
            id: "test".into(),
            version: 1,
            description: "test".into(),
            target_class: "cleric".into(),
            target_camp: "*".into(),
            terms: vec![RewardTerm {
                name: "survival".into(),
                weight: 0.6,
                signal: "party_alive_fraction".into(),
            }],
            clamp: [-1.0, 1.0],
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn evaluate_basic() {
        let cfg = RewardConfig {
            id: "test".into(),
            version: 1,
            description: "test".into(),
            target_class: "cleric".into(),
            target_camp: "*".into(),
            terms: vec![
                RewardTerm {
                    name: "survival".into(),
                    weight: 0.6,
                    signal: "party_alive_fraction".into(),
                },
                RewardTerm {
                    name: "ban_risk".into(),
                    weight: -1.0,
                    signal: "antidetect.risk_score".into(),
                },
            ],
            clamp: [-1.0, 1.0],
        };

        let mut signals = HashMap::new();
        signals.insert("party_alive_fraction".into(), 0.9);
        signals.insert("antidetect.risk_score".into(), 0.0);

        let reward = cfg.evaluate(&signals).unwrap();
        assert!((reward - 0.54).abs() < 1e-6); // 0.6 * 0.9 + (-1.0 * 0.0)
    }

    #[test]
    fn evaluate_clamps_result() {
        let cfg = RewardConfig {
            id: "test".into(),
            version: 1,
            description: "test".into(),
            target_class: "cleric".into(),
            target_camp: "*".into(),
            terms: vec![
                RewardTerm {
                    name: "big_positive".into(),
                    weight: 10.0,
                    signal: "party_alive_fraction".into(),
                },
                RewardTerm {
                    name: "ban_risk".into(),
                    weight: -1.0,
                    signal: "antidetect.risk_score".into(),
                },
            ],
            clamp: [-1.0, 1.0],
        };

        let mut signals = HashMap::new();
        signals.insert("party_alive_fraction".into(), 0.5);
        signals.insert("antidetect.risk_score".into(), 0.0);

        let reward = cfg.evaluate(&signals).unwrap();
        assert_eq!(reward, 1.0); // clamped from 5.0
    }

    #[test]
    fn registry_exact_match() {
        let cfg = RewardConfig {
            id: "test".into(),
            version: 1,
            description: "test".into(),
            target_class: "cleric".into(),
            target_camp: "plane_of_valor".into(),
            terms: vec![RewardTerm {
                name: "ban_risk".into(),
                weight: -1.0,
                signal: "antidetect.risk_score".into(),
            }],
            clamp: [-1.0, 1.0],
        };

        let mut reg = RewardRegistry::new();
        reg.register(cfg).unwrap();

        let found = reg.get("cleric", "plane_of_valor");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "test");
    }

    #[test]
    fn registry_wildcard_fallback() {
        let cfg = RewardConfig {
            id: "wildcard".into(),
            version: 1,
            description: "test".into(),
            target_class: "cleric".into(),
            target_camp: "*".into(),
            terms: vec![RewardTerm {
                name: "ban_risk".into(),
                weight: -1.0,
                signal: "antidetect.risk_score".into(),
            }],
            clamp: [-1.0, 1.0],
        };

        let mut reg = RewardRegistry::new();
        reg.register(cfg).unwrap();

        let found = reg.get("cleric", "unknown_camp");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "wildcard");
    }

    #[test]
    fn registry_no_match() {
        let reg = RewardRegistry::new();
        let found = reg.get("paladin", "unknown");
        assert!(found.is_none());
    }
}
