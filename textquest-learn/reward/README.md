# Reward Function Library

Operators author reward specifications in YAML. The binary validates and evaluates them to drive learner behavior.

## Schema

```yaml
id: combat.dps.generic.v1
version: 1
description: "Brief description of what this reward emphasizes"
target_class: "*"  # or specific class like "cleric", "warrior"
target_camp: "*"   # or specific camp like "plane_of_valor"
terms:
  - name: throughput           # unique identifier within spec
    weight: +0.7               # multiplier for this signal
    signal: encounter_throughput
  - name: ban_risk
    weight: -1.0               # hard penalty to prevent erratic behavior
    signal: antidetect.risk_score
clamp: [-1.0, 1.0]            # final reward clamped to this range
```

## Evaluation

For each tick, the binary:
1. Collects signal values from ledger telemetry
2. Computes: `reward = sum(weight[i] * signal[i])`
3. Clamps result to `[clamp.min, clamp.max]`

Clamping ensures no single term dominates; weight balancing is operator responsibility.

## Known Signals

- `party_alive_fraction` — fraction of party members alive [0, 1]
- `mana_per_effective_heal` — mana efficiency (normalized)
- `overheal_fraction` — wasted healing / total healing [0, 1]
- `encounter_throughput` — DPS-like encounter metric
- `party_survival_time` — seconds party lasted in encounter
- `mitigation_ratio` — damage mitigated / total damage taken [0, 1]
- `aggro_retention` — % of fight where tank held aggro [0, 1]
- `kills_per_hour` — encounters / hour (camp metric)
- `downtime_penalty` — fraction of time group is not engaged [0, 1]
- `time_to_ready` — seconds to buff and return to camp [0, ∞)
- `antidetect.risk_score` — M5 anti-cheat telemetry signal [0, 1]

## Validation Rules

The binary enforces:
- `id` and `version` are required and non-empty
- Each `term.signal` must be a known signal name
- Each `term.name` must be unique within the spec
- `clamp[0] < clamp[1]` (min < max)
- Every spec **must include** an `antidetect.risk_score` term (ban-risk is mandatory)

## Starter Specs

Five shipped configs demonstrate best practices:

1. **combat.dps.generic.v1** — Wildcard DPS reward (encounter throughput + survival)
2. **combat.heal.cleric.v1** — Cleric-specific (party survival + mana efficiency)
3. **combat.tank.warrior.v1** — Warrior-specific (aggro + mitigation)
4. **camp.throughput.v1** — Camp grinding (kills/hour - downtime)
5. **recovery.med.v1** — Post-wipe recovery (minimize time to ready)

## Authoring Guidelines

- **Ban-risk always present** — Signals from M5 telemetry capture erratic behavior. Always weight `antidetect.risk_score` at `-1.0` or stronger.
- **Weights are additive** — Multiple positive weights are summed; don't make individual weights too large.
- **Clamp handles scale** — If you weight something at `+5.0`, clamping will cap it at `+1.0`. Use relative magnitudes between `+/-0.1` and `+/-1.0`.
- **Version on change** — Increment `version` when modifying the spec; policies train under specific versions and should not be retrained on changed specs.

## Example: Custom Cleric Spec

```yaml
id: combat.heal.cleric.raider.v2
version: 2
description: "Cleric reward for raid healing: party survival > mana efficiency"
target_class: cleric
target_camp: raider  # specific camp
terms:
  - name: survival
    weight: +0.8
    signal: party_alive_fraction
  - name: efficiency
    weight: +0.15
    signal: mana_per_effective_heal
  - name: overheal
    weight: -0.15
    signal: overheal_fraction
  - name: evasion
    weight: -1.0
    signal: antidetect.risk_score
clamp: [-1.0, 1.0]
```
