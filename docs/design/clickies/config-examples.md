# Clickies Module — Configuration Examples

Complete configuration examples for different character classes and playstyles.

## 1. Warrior (Tank) — Survivability Focus

```toml
[clickies]
enabled = true
verbose_logging = true
evaluation_frequency_ms = 500

# Emergency healing during combat
[[clickies.group]]
name = "emergency_heals"
phase = "combat"
priority = 1

[[clickies.group.item]]
slot = 1
name = "Healing Potion"
condition = "hp_pct < 50 && in_combat && item_ready"
cooldown_ms = 6000

[[clickies.group.item]]
slot = 2
name = "Greater Healing Potion"
condition = "hp_pct < 25 && in_combat && item_ready"
cooldown_ms = 12000
use_count = 5

# Downtime sustenance
[[clickies.group]]
name = "downtime_sustenance"
phase = "downtime"
priority = 1

[[clickies.group.item]]
slot = 3
name = "Meat Pie"
condition = "hunger > 30"
cooldown_ms = 500

[[clickies.group.item]]
slot = 4
name = "Waterskin"
condition = "thirst > 30"
cooldown_ms = 500

# Pre-combat buffs
[[clickies.group]]
name = "precomm_buffs"
phase = "precomm"
priority = 2

[[clickies.group.item]]
slot = 20
name = "Ring of the Warrior"
condition = "not_in_combat && hp_pct > 90"
cooldown_ms = 180000

[[clickies.group.item]]
slot = 21
name = "Cloak of Thorns"
condition = "not_in_combat"
cooldown_ms = 240000
```

## 2. Magician (DPS) — Mana Management

```toml
[clickies]
enabled = true
verbose_logging = false
evaluation_frequency_ms = 500

# Mana recovery during and after combat
[[clickies.group]]
name = "mana_recovery"
phase = "combat"
priority = 1

[[clickies.group.item]]
slot = 1
name = "Mana Potion"
condition = "mana_pct < 30 && in_combat && item_ready"
cooldown_ms = 8000

[[clickies.group.item]]
slot = 2
name = "Greater Mana Potion"
condition = "mana_pct < 15 && in_combat && item_ready"
cooldown_ms = 15000
use_count = 3

# Downtime mana regen items
[[clickies.group]]
name = "downtime_mana"
phase = "downtime"
priority = 1

[[clickies.group.item]]
slot = 3
name = "Spirit Shrine Clicky"
condition = "mana_pct < 80 && not_in_combat"
cooldown_ms = 30000

# Emergency healing
[[clickies.group]]
name = "emergency_heals"
phase = "combat"
priority = 2

[[clickies.group.item]]
slot = 4
name = "Healing Potion"
condition = "hp_pct < 40 && in_combat"
cooldown_ms = 6000

# Downtime sustenance
[[clickies.group]]
name = "downtime_sustenance"
phase = "downtime"
priority = 2

[[clickies.group.item]]
slot = 5
name = "Bread Loaf"
condition = "hunger > 25"
cooldown_ms = 500

[[clickies.group.item]]
slot = 6
name = "Tea Cup"
condition = "thirst > 25"
cooldown_ms = 500

# Pre-combat damage buffs
[[clickies.group]]
name = "precomm_buffs"
phase = "precomm"
priority = 1

[[clickies.group.item]]
slot = 20
name = "Ring of the Summoner"
condition = "not_in_combat"
cooldown_ms = 120000

[[clickies.group.item]]
slot = 21
name = "Cloak of Embers"
condition = "not_in_combat && not_buffed(damage_buff)"
cooldown_ms = 180000
```

## 3. Necromancer (Hybrid) — Balanced Approach

```toml
[clickies]
enabled = true
verbose_logging = false
evaluation_frequency_ms = 500

# Healing during combat (spells + potions)
[[clickies.group]]
name = "combat_heals"
phase = "combat"
priority = 1

[[clickies.group.item]]
slot = 1
name = "Healing Potion"
condition = "hp_pct < 45 && in_combat && item_ready"
cooldown_ms = 5000

# Mana management during combat
[[clickies.group]]
name = "combat_mana"
phase = "combat"
priority = 2

[[clickies.group.item]]
slot = 2
name = "Mana Potion"
condition = "mana_pct < 35 && in_combat && item_ready"
cooldown_ms = 8000

# Downtime recovery (aggressive, reuse often)
[[clickies.group]]
name = "downtime_recovery"
phase = "downtime"
priority = 1

[[clickies.group.item]]
slot = 3
name = "Bowl of Porridge"
condition = "hunger > 20"
cooldown_ms = 300

[[clickies.group.item]]
slot = 4
name = "Tea Cup"
condition = "thirst > 20"
cooldown_ms = 300

[[clickies.group.item]]
slot = 5
name = "Spirit Shrine"
condition = "mana_pct < 70 && not_in_combat"
cooldown_ms = 30000

# Pre-combat utility
[[clickies.group]]
name = "precomm_buffs"
phase = "precomm"
priority = 1

[[clickies.group.item]]
slot = 20
name = "Ring of Shadows"
condition = "not_in_combat"
cooldown_ms = 150000

[[clickies.group.item]]
slot = 21
name = "Cloak of Undeath"
condition = "not_in_combat && not_buffed(undead_bonus)"
cooldown_ms = 180000
```

## 4. Cleric (Support) — Group Healing Focus

```toml
[clickies]
enabled = true
verbose_logging = true
evaluation_frequency_ms = 400  # More frequent for group support

# Self-healing
[[clickies.group]]
name = "self_healing"
phase = "combat"
priority = 2

[[clickies.group.item]]
slot = 1
name = "Healing Potion"
condition = "hp_pct < 50 && in_combat && item_ready"
cooldown_ms = 5000

[[clickies.group.item]]
slot = 2
name = "Greater Healing Potion"
condition = "hp_pct < 30 && in_combat && item_ready"
cooldown_ms = 10000
use_count = 5

# Mana recovery
[[clickies.group]]
name = "mana_recovery"
phase = "combat"
priority = 1

[[clickies.group.item]]
slot = 3
name = "Mana Potion"
condition = "mana_pct < 40 && in_combat && item_ready"
cooldown_ms = 8000

# Downtime sustenance
[[clickies.group]]
name = "downtime_sustenance"
phase = "downtime"
priority = 1

[[clickies.group.item]]
slot = 4
name = "Bread Loaf"
condition = "hunger > 30"
cooldown_ms = 400

[[clickies.group.item]]
slot = 5
name = "Waterskin"
condition = "thirst > 30"
cooldown_ms = 400

# Pre-combat preparation
[[clickies.group]]
name = "precomm_buffs"
phase = "precomm"
priority = 1

[[clickies.group.item]]
slot = 20
name = "Ring of the Cleric"
condition = "not_in_combat"
cooldown_ms = 120000

[[clickies.group.item]]
slot = 21
name = "Cloak of Healing"
condition = "not_in_combat && not_buffed(healing_buff)"
cooldown_ms = 180000
```

## 5. Monk (Melee Burst) — Damage and Sustenance

```toml
[clickies]
enabled = true
evaluation_frequency_ms = 500

# Emergency healing only (mobs die fast)
[[clickies.group]]
name = "emergency_heals"
phase = "combat"
priority = 1

[[clickies.group.item]]
slot = 1
name = "Healing Potion"
condition = "hp_pct < 30 && in_combat && item_ready"
cooldown_ms = 5000

# Quick mana boost (haste, abilities)
[[clickies.group]]
name = "combat_endurance"
phase = "combat"
priority = 2

[[clickies.group.item]]
slot = 2
name = "Endurance Potion"
condition = "endurance_pct < 25 && in_combat && item_ready"
cooldown_ms = 6000

# Aggressive downtime reuse
[[clickies.group]]
name = "downtime_recovery"
phase = "downtime"
priority = 1

[[clickies.group.item]]
slot = 3
name = "Meat Pie"
condition = "hunger > 15"  # More aggressive than others
cooldown_ms = 200

[[clickies.group.item]]
slot = 4
name = "Waterskin"
condition = "thirst > 15"
cooldown_ms = 200

# Pre-combat haste/damage buffs
[[clickies.group]]
name = "precomm_buffs"
phase = "precomm"
priority = 1

[[clickies.group.item]]
slot = 20
name = "Ring of the Martial Artist"
condition = "not_in_combat"
cooldown_ms = 90000

[[clickies.group.item]]
slot = 21
name = "Cloak of Swiftness"
condition = "not_in_combat && not_buffed(haste)"
cooldown_ms = 120000
```

## Advanced: Multi-Potion Groups with Priority

```toml
# Scenario: Multiple potion types with fallback logic
[clickies]
enabled = true
evaluation_frequency_ms = 400

# Primary healing potions (fast-acting, high cooldown)
[[clickies.group]]
name = "healing_tier1"
phase = "combat"
priority = 1

[[clickies.group.item]]
slot = 1
name = "Superior Healing Potion"
condition = "hp_pct < 35 && in_combat && item_ready"
cooldown_ms = 10000

# Secondary healing (slower, lower cooldown)
[[clickies.group]]
name = "healing_tier2"
phase = "combat"
priority = 2

[[clickies.group.item]]
slot = 2
name = "Healing Potion"
condition = "hp_pct < 50 && in_combat && item_ready && last_healing_potion > 5s"
cooldown_ms = 5000

# Endurance sustenance (outside mana)
[[clickies.group]]
name = "endurance"
phase = "combat"
priority = 3

[[clickies.group.item]]
slot = 3
name = "Endurance Draught"
condition = "endurance_pct < 40 && in_combat && item_ready"
cooldown_ms = 6000

# Passive downtime (continuous)
[[clickies.group]]
name = "passive_downtime"
phase = "downtime"
priority = 10  # Lowest priority

[[clickies.group.item]]
slot = 4
name = "Bowl of Porridge"
condition = "hunger > 20"
cooldown_ms = 200

[[clickies.group.item]]
slot = 5
name = "Waterskin"
condition = "thirst > 20"
cooldown_ms = 200
```

## Configuration Best Practices

1. **Cooldown Timing**: Match in-game item reuse timers (usually 5-60 seconds)
2. **Stat Thresholds**: Set HP/Mana triggers 10-15% above minimum survivable
3. **Priority Ordering**: 1 = highest priority, 10 = lowest
4. **Phase Management**: Use `downtime` for passive recovery, `combat` for active defense
5. **Use Counts**: Limit powerful items to 3-5 per encounter
6. **Testing**: Validate with actual combat logs to refine thresholds

## Item Slot Reference

| Slots | Type |
|-------|------|
| 1-16 | Main inventory |
| 17-20 | Trade/Bag slots |
| 21-36 | Equipment slots (neck, wrists, hands, waist, etc.) |

See EQ wiki for full inventory layout.
