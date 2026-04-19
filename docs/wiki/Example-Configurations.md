# Example Configurations

Reference configurations for common TextQuest setups. Copy these templates and adjust values for your environment.

## Table of Contents

1. [Main Config](#main-config)
2. [Accounts Config](#accounts-config)
3. [Camp Configs](#camp-configs)
4. [Class Rotations](#class-rotations)
5. [Per-Toon Overrides](#per-toon-overrides)

---

## Main Config

`config/textquest.toml`:

```toml
process_name = "eqgame"
max_spawns = 10000

[server]
name = "Firiona Vie"
auto_discover = true
polling_interval_ms = 100

[launch]
stagger_ms = 2000
max_clients = 6

[[group]]
id = 1
name = "Group 1"

[[group]]
id = 2
name = "Group 2"

[alerts]
enable_discord = false

[alerts.thresholds]
death_alert = true
stuck_alert = true
memory_warning_mb = 200
```

---

## Accounts Config

`config/accounts.toml`:

```toml
[[accounts]]
name = "account01"
server = "Firiona Vie"
character = "Tank01"
class = "WAR"
group = 1

[[accounts]]
name = "account02"
server = "Firiona Vie"
character = "Healer01"
class = "CLR"
group = 1

[[accounts]]
name = "account03"
server = "Firiona Vie"
character = "Dps01"
class = "Rog"
group = 1

[[accounts]]
name = "account04"
server = "Firiona Vie"
character = "Enchanter01"
class = "ENC"
group = 1

[[accounts]]
name = "account05"
server = "Firiona Vie"
character = "Dps02"
class = "MNK"
group = 2

[[accounts]]
name = "account06"
server = "Firiona Vie"
character = "Shaman01"
class = "SHM"
group = 2
```

---

## Camp Configs

### Basic Camp (g Faydark)

`config/camps/gfaydark_baseline.toml`:

```toml
zone = "gfaydark"
camp_center = [200, -100, 50]
camp_radius = 200
pull_point = [250, -150, 50]
pull_radius = 500
leash_radius = 1000

heal_at_pct = 30
mana_regen_secs = 6
```

### Sebilis Disco Camp

`config/camps/sebilis_disco.toml`:

```toml
zone = "crescentreach"
camp_center = [-350, -650, -24]
camp_radius = 250
pull_point = [-350, -900, -24]
pull_radius = 600
leash_radius = 1200
heal_at_pct = 35
```

### Dreadlands Primary

`config/camps/dreadlands_primary.toml`:

```toml
zone = "dreadlands"
camp_center = [1350, -2775, -188]
camp_radius = 300
pull_point = [1100, -3100, -188]
pull_radius = 700
leash_radius = 1500
heal_at_pct = 30
return_no_aggro = true
```

### Underquarry Scouts

`config/camps/foundation_underquarry_scouts.toml`:

```toml
zone = "shadowrest"
camp_center = [90, -50, 20]
camp_radius = 200
pull_point = [200, -100, 20]
pull_radius = 500
leash_radius = 1000
heal_at_pct = 30
```

### Velketor's Labyrinth Frenzy

`config/camps/velketors_labyrinth_frenzy.toml`:

```toml
zone = "velketors"
camp_center = [80, -250, 25]
camp_radius = 250
pull_point = [-50, -400, 25]
pull_radius = 600
leash_radius = 1200
heal_at_pct = 40
```

---

## Class Rotations

### Warrior (Tank)

`config/classes/warrior.toml`:

```toml
class_name = "warrior"
role = "tank"
rest_command = "/sit"

[[level_overrides]]
name = "warrior-live-62"
min_level = 62
max_level = 62

[[level_overrides.combat_abilities]]
name = "Deflection Discipline"
command = "/disc Deflection Discipline"
cooldown_secs = 900.0
priority = 10

[[level_overrides.combat_abilities]]
name = "Taunt"
command = "/disc Taunt"
cooldown_secs = 6.0
priority = 20
```

### Cleric (Healer)

`config/classes/cleric.toml`:

```toml
class_name = "cleric"
role = "healer"
rest_command = "/sit"

[[spells]]
name = "Superior Healing"
command = "/cast 4"
cooldown_secs = 0
priority = 10

[[spells]]
name = "Complete Heal"
command = "/cast 7"
cooldown_secs = 0
priority = 5

[[spells]]
name = "Celestial Healing"
command = "/cast 6"
cooldown_secs = 0
priority = 15

[[holyshit_rules]]
min_target_hp = 20
spell_name = "Complete Heal"
```

### Enchanter (Crowd Control)

`config/classes/enchanter.toml`:

```toml
class_name = "enchanter"
role = "cc"
rest_command = "/sit"

[[spells]]
name = "Tashan"
command = "/cast 1"
cooldown_secs = 0
priority = 10

[[spells]]
name = "Mesmerize"
command = "/cast 2"
cooldown_secs = 0
priority = 20

[[spells]]
name = "Enthrall"
command = "/cast 3"
cooldown_secs = 0
priority = 25
```

### Shaman (Debuffer)

`config/classes/shaman.toml`:

```toml
class_name = "shaman"
role = "debuffer"
rest_command = "/sit"

[[spells]]
name = "Turgur's Insects"
command = "/cast 2"
cooldown_secs = 0
priority = 10

[[spells]]
name = "Superior Healing"
command = "/cast 5"
cooldown_secs = 0
priority = 20

[[spells]]
name = "Cannibalize III"
command = "/cast 7"
cooldown_secs = 0
priority = 30
```

### Wizard (DPS)

`config/classes/wizard.toml`:

```toml
class_name = "wizard"
role = "dps"
rest_command = "/sit"

[[spells]]
name = "Sunstrike"
command = "/cast 4"
cooldown_secs = 0
priority = 10

[[spells]]
name = "Ice Comet"
command = "/cast 5"
cooldown_secs = 0
priority = 20
```

### Bard (Support)

`config/classes/bard.toml`:

```toml
class_name = "bard"
role = "support"
rest_command = "/melody 1 2 3 4"

[songs]
haste_song = "Vilia's Verses of Celerity"
mana_song = "Cassindra's Chorus of Clarity"
```

### Necromancer (DOT DPS)

`config/classes/necromancer.toml`:

```toml
class_name = "necromancer"
role = "dps"
rest_command = "/sit"

[[spells]]
name = "Disease"
command = "/cast 1"
cooldown_secs = 0
priority = 10

[[spells]]
name = "Plague"
command = "/cast 2"
cooldown_secs = 0
priority = 15

[[spells]]
name = "Splurt"
command = "/cast 5"
cooldown_secs = 0
priority = 20

[[spells]]
name = "Lich"
command = "/cast 8"
cooldown_secs = 0
priority = 5
```

---

## Per-Toon Overrides

### Tank Override Example

`config/toons/Tank01.toml`:

```toml
[[spells]]
name = "Deflection Discipline"
cooldown_secs = 900

[[spells]]
name = "Fortitude Discipline"
cooldown_secs = 900
```

### Healer Override Example

`config/toons/Healer01.toml`:

```toml
[thresholds]
reactive_heal_pct = 45
group_heal_pct = 50
```

### DPS Override Example

`config/toons/Rog01.toml`:

```toml
[[spells]]
name = "Backstab"
cooldown_secs = 0
```

---

## HVT Watchlist

`config/hvt_watchlist.toml`:

```toml
[[named]]
name = "Frostcreeper King"
zone = "gfaydark"
discord_alert = true

[[named]]
name = "Lady Vox"
zone = "karnor"
discord_alert = true

[[named]]
name = "Lord Vyemm"
zone = "dreadlands"
discord_alert = true
```

---

## Related Documentation

- [Configuration Guide](Configuration.md)
- [Camp Configuration](Configuration.md#camps)
- [Class Combat Rotations](Class-Combat-Rotations.md)
- [Command Reference](Command-Reference.md)
- [Troubleshooting](Troubleshooting.md)