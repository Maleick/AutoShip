use std::collections::HashMap;
use textquest_learn::reward::RewardConfig;

/// Golden test: combat.dps.generic fixture
#[test]
fn golden_combat_dps_generic() {
    let yaml = r#"
id: combat.dps.generic.v1
version: 1
description: "Generic DPS reward: maximize encounter throughput + party survival"
target_class: "*"
target_camp: "*"
terms:
  - name: throughput
    weight: +0.7
    signal: encounter_throughput
  - name: survival
    weight: +0.2
    signal: party_survival_time
  - name: ban_risk
    weight: -1.0
    signal: antidetect.risk_score
clamp: [-1.0, 1.0]
"#;

    let cfg = RewardConfig::from_yaml(yaml).expect("should parse");
    assert_eq!(cfg.id, "combat.dps.generic.v1");
    assert_eq!(cfg.terms.len(), 3);

    // Scenario 1: healthy encounter, no ban risk
    let mut signals = HashMap::new();
    signals.insert("encounter_throughput".into(), 0.8);
    signals.insert("party_survival_time".into(), 0.9);
    signals.insert("antidetect.risk_score".into(), 0.0);

    let reward = cfg.evaluate(&signals).expect("should evaluate");
    let expected = 0.7 * 0.8 + 0.2 * 0.9 - 1.0 * 0.0; // 0.56 + 0.18 = 0.74
    assert!((reward - expected).abs() < 1e-6);

    // Scenario 2: bad encounter with ban risk
    signals.insert("encounter_throughput".into(), 0.1);
    signals.insert("party_survival_time".into(), 0.2);
    signals.insert("antidetect.risk_score".into(), 0.5);

    let reward = cfg.evaluate(&signals).expect("should evaluate");
    let expected = (0.7_f32 * 0.1 + 0.2 * 0.2 - 1.0 * 0.5).clamp(-1.0, 1.0); // 0.07 + 0.04 - 0.5 = -0.39
    assert!((reward - expected).abs() < 1e-6);
}

/// Golden test: combat.heal.cleric fixture
#[test]
fn golden_combat_heal_cleric() {
    let yaml = r#"
id: combat.heal.cleric.v1
version: 1
description: "Cleric reward: emphasize party survival + efficient mana use"
target_class: cleric
target_camp: "*"
terms:
  - name: party_survival
    weight: +0.6
    signal: party_alive_fraction
  - name: mana_efficiency
    weight: +0.2
    signal: mana_per_effective_heal
  - name: overheal_penalty
    weight: -0.1
    signal: overheal_fraction
  - name: ban_risk
    weight: -1.0
    signal: antidetect.risk_score
clamp: [-1.0, 1.0]
"#;

    let cfg = RewardConfig::from_yaml(yaml).expect("should parse");
    assert_eq!(cfg.target_class, "cleric");

    // Scenario 1: excellent healing
    let mut signals = HashMap::new();
    signals.insert("party_alive_fraction".into(), 1.0);
    signals.insert("mana_per_effective_heal".into(), 0.9);
    signals.insert("overheal_fraction".into(), 0.1);
    signals.insert("antidetect.risk_score".into(), 0.0);

    let reward = cfg.evaluate(&signals).expect("should evaluate");
    let expected = 0.6 * 1.0 + 0.2 * 0.9 - 0.1 * 0.1 - 1.0 * 0.0; // 0.6 + 0.18 - 0.01 = 0.77
    assert!((reward - expected).abs() < 1e-6);

    // Scenario 2: poor healing with overheal spam
    signals.insert("party_alive_fraction".into(), 0.5);
    signals.insert("mana_per_effective_heal".into(), 0.3);
    signals.insert("overheal_fraction".into(), 0.6);
    signals.insert("antidetect.risk_score".into(), 0.0);

    let reward = cfg.evaluate(&signals).expect("should evaluate");
    let expected = 0.6 * 0.5 + 0.2 * 0.3 + (-0.1) * 0.6; // 0.3 + 0.06 - 0.06 = 0.3
    assert!((reward - expected).abs() < 1e-6);
}

/// Golden test: combat.tank.warrior fixture
#[test]
fn golden_combat_tank_warrior() {
    let yaml = r#"
id: combat.tank.warrior.v1
version: 1
description: "Warrior reward: maximize aggro retention + mitigation"
target_class: warrior
target_camp: "*"
terms:
  - name: aggro_retention
    weight: +0.6
    signal: aggro_retention
  - name: mitigation
    weight: +0.3
    signal: mitigation_ratio
  - name: ban_risk
    weight: -1.0
    signal: antidetect.risk_score
clamp: [-1.0, 1.0]
"#;

    let cfg = RewardConfig::from_yaml(yaml).expect("should parse");
    assert_eq!(cfg.target_class, "warrior");

    // Scenario 1: perfect tank
    let mut signals = HashMap::new();
    signals.insert("aggro_retention".into(), 1.0);
    signals.insert("mitigation_ratio".into(), 0.8);
    signals.insert("antidetect.risk_score".into(), 0.0);

    let reward = cfg.evaluate(&signals).expect("should evaluate");
    let expected = 0.6 * 1.0 + 0.3 * 0.8; // 0.6 + 0.24 = 0.84
    assert!((reward - expected).abs() < 1e-6);
}

/// Golden test: camp.throughput fixture
#[test]
fn golden_camp_throughput() {
    let yaml = r#"
id: camp.throughput.v1
version: 1
description: "Camp reward: maximize kills per hour with downtime penalty"
target_class: "*"
target_camp: "*"
terms:
  - name: kills_per_hour
    weight: +0.8
    signal: kills_per_hour
  - name: downtime_penalty
    weight: -0.2
    signal: downtime_penalty
  - name: ban_risk
    weight: -1.0
    signal: antidetect.risk_score
clamp: [-1.0, 1.0]
"#;

    let cfg = RewardConfig::from_yaml(yaml).expect("should parse");

    // Scenario 1: fast camp
    let mut signals = HashMap::new();
    signals.insert("kills_per_hour".into(), 0.9);
    signals.insert("downtime_penalty".into(), 0.1);
    signals.insert("antidetect.risk_score".into(), 0.0);

    let reward = cfg.evaluate(&signals).expect("should evaluate");
    let expected = 0.8 * 0.9 + (-0.2) * 0.1; // 0.72 - 0.02 = 0.7
    assert!((reward - expected).abs() < 1e-6);

    // Scenario 2: slow camp with lots of downtime
    signals.insert("kills_per_hour".into(), 0.4);
    signals.insert("downtime_penalty".into(), 0.8);
    signals.insert("antidetect.risk_score".into(), 0.0);

    let reward = cfg.evaluate(&signals).expect("should evaluate");
    let expected = 0.8 * 0.4 + (-0.2) * 0.8; // 0.32 - 0.16 = 0.16
    assert!((reward - expected).abs() < 1e-6);
}

/// Golden test: recovery.med fixture
#[test]
fn golden_recovery_med() {
    let yaml = r#"
id: recovery.med.v1
version: 1
description: "Recovery reward: minimize time to ready state after wipe"
target_class: "*"
target_camp: "*"
terms:
  - name: time_to_ready
    weight: -0.9
    signal: time_to_ready
  - name: ban_risk
    weight: -1.0
    signal: antidetect.risk_score
clamp: [-1.0, 1.0]
"#;

    let cfg = RewardConfig::from_yaml(yaml).expect("should parse");

    // Scenario 1: quick recovery
    let mut signals = HashMap::new();
    signals.insert("time_to_ready".into(), 0.1); // 10% of max acceptable time
    signals.insert("antidetect.risk_score".into(), 0.0);

    let reward = cfg.evaluate(&signals).expect("should evaluate");
    let expected = (-0.9) * 0.1; // -0.09
    assert!((reward - expected).abs() < 1e-6);

    // Scenario 2: slow recovery (clamped)
    signals.insert("time_to_ready".into(), 2.0); // exceeds clamp
    signals.insert("antidetect.risk_score".into(), 0.0);

    let reward = cfg.evaluate(&signals).expect("should evaluate");
    assert_eq!(reward, -1.0); // clamped
}

/// Test: all starter specs load and validate
#[test]
fn starter_specs_all_valid() {
    let yaml_specs = vec![
        include_str!("../rewards/combat.dps.generic.v1.yaml"),
        include_str!("../rewards/combat.heal.cleric.v1.yaml"),
        include_str!("../rewards/combat.tank.warrior.v1.yaml"),
        include_str!("../rewards/camp.throughput.v1.yaml"),
        include_str!("../rewards/recovery.med.v1.yaml"),
    ];

    for yaml in yaml_specs {
        let cfg = RewardConfig::from_yaml(yaml).expect("should parse and validate");
        cfg.validate().expect("should validate");

        // Verify ban-risk presence
        let has_ban_risk = cfg
            .terms
            .iter()
            .any(|t| t.signal == "antidetect.risk_score");
        assert!(has_ban_risk, "spec {} missing ban-risk term", cfg.id);
    }
}
