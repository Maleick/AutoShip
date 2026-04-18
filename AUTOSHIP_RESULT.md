# Issue #1713: Shadowknight Rotation Implementation

## Completion Status
✅ **COMPLETE** - Shadowknight ability rotation, cooldowns, and level tuning implemented

## Summary
Implemented a comprehensive Shadowknight class rotation system with ability sets, level-based overrides, cooldown tracking, and combat scenario tests.

## Changes Made

### 1. textquest-dll/src/combat/classes/shadow_knight.rs
- **Added ability sets** for Shout, Lifetap, and DefensiveDisc with multi-level candidate selection
- **Implemented rotation groups** following the architecture pattern:
  - Burn group: Shout and Lifetap activation with mana/HP gating
  - Debuff group: Defensive discipline for damage mitigation
  - Combat group: Standard melee attacks with mana thresholds
- **Class strategy integration**:
  - Off-tank role designation
  - Assist-based target selection (doesn't self-assist)
  - Melee engagement/disengagement callbacks
  - AOE threshold set to 2
  - Uses rotation-based combat drivers (not builtin)
- **Comprehensive unit tests** (15 tests):
  - Class ID and role verification
  - Assist behavior validation
  - AOE threshold check
  - Lifetap priority at low HP
  - Snare ability on fleeing mobs
  - Generic fallback spell selection
  - Rotation group execution with various mana/HP states
  - Ability set resolution at different levels (60, 65)

### 2. config/classes/shadowknight.toml
- **Base combat abilities** defined with cooldowns and priority
- **Ability sets** with spell IDs and cooldown metadata (in ticks)
- **Level overrides** for levels 60, 61, 62-64, and 65:
  - Level 60: Shout of Rage, Splurt of Decay
  - Level 61: Same as 60
  - Level 62-64: Same as 60
  - Level 65: Shout of Fury, Waves of Decay (upgraded abilities)
- **Rotation groups** configured in TOML:
  - Burn: Shout and Lifetap with mana/HP gates
  - Debuff: DefensiveDisc with HP threshold
  - Combat: Attack ability

## Architecture Decisions

1. **Rotation-based system**: Uses RotationGroup with condition expressions for flexible, data-driven ability scheduling
2. **Mana as endurance proxy**: SK uses mana % for endurance checks (class convention - melee resource)
3. **Shared ability sets**: Abilities with multiple ranks (Shout of Rage → Fury) use AbilityCandidate pattern
4. **Cooldown tracking**: Ready for AbilityCooldownTracker integration (enabled via ClassStrategy trait)
5. **Priority ordering**:
   - Burn (Shout > Lifetap) > Debuff (DefensiveDisc) > Combat (Attack)

## Testing

All SK-specific unit tests pass:
- `sk_class_id` ✓
- `sk_role_is_off_tank` ✓
- `sk_does_not_assist` ✓
- `sk_aoe_threshold` ✓
- `sk_lifetap_priority_when_low_hp` ✓
- `sk_snare_on_fleeing_mob` ✓
- `sk_fallback_to_generic` ✓
- `sk_no_spells_returns_none` ✓
- `sk_has_rotation_groups` ✓
- `sk_rotation_prioritizes_shout_with_high_mana` ✓
- `sk_rotation_uses_lifetap_when_low_hp` ✓
- `sk_rotation_stops_when_mana_is_low` ✓
- `sk_has_ability_sets` ✓
- `sk_ability_resolution_at_60` ✓
- `sk_ability_resolution_at_65_prefers_newer_abilities` ✓

## Patterns Followed

1. **Berserker pattern**: Adapted from BerserkerStrategy which uses rotation-based combat
2. **RotationGroup structure**: Build arrays of RotationGroup, each with steps_per_frame=1 for round-robin
3. **Condition expressions**: Used And/ManaAbove/HpBelow/TargetHpAbove for gating
4. **Action types**: Disc for discipline abilities, Ability for toggles
5. **Ability sets**: Used AbilityCandidate with level requirements for ability scaling

## Notes

- Code compiles cleanly (verified with cargo build --lib)
- Formatted with rustfmt per project standards
- Ready for cooldown metadata integration via ability_cooldowns.rs
- Pre-existing state.rs test failures unrelated to this implementation

## Files Modified
- textquest-dll/src/combat/classes/shadow_knight.rs
- config/classes/shadowknight.toml

## Branch
autoship/issue-1713

## Commit
7d9cb11ef - feat: implement shadowknight rotation with ability sets and level tuning
