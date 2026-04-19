# Advanced Topics

Advanced automation techniques, tuning strategies, and optimization for TextQuest operators.

## Table of Contents

1. [Camp Tuning](#camp-tuning)
2. [Combat Optimization](#combat-optimization)
3. [Login Sequencing](#login-sequencing)
4. [Navigation Strategies](#navigation-strategies)
5. [Performance Tuning](#performance-tuning)
6. [Automation Patterns](#automation-patterns)

---

## Camp Tuning

### Pull Radius Optimization

The pull radius determines how far the puller travels to engage mobs. Optimal settings depend on:

| Camp Type | Pull Radius | Camp Radius | Notes |
|----------|------------|-----------|-------|
| Single camp | 400-600 | 150-200 | Standard static camp |
| Kite camp | 800-1200 | 200-300 | Mobile pulling |
| Roaming | Varies | Full zone | Multi-point circuits |

### Distance Gates

TextQuest uses distance gates to prevent premature engagement:

```toml
pull_radius = 500          # Engage mobs within this range
camp_radius = 200          # Return to camp if mob escapes
leash_radius = 1000        # Give up chase beyond this
```

### Return No Aggro

For camps where adds frequently reset:

```toml
return_no_aggro = true     # Don't return if mob reset
```

---

## Combat Optimization

### Mana Management Floors

Reserve mana for critical actions:

| Class | Reserve | Purpose |
|-------|--------|---------|
| Cleric | 15% | Emergency heals |
| Enchanter | 20% | Mez recharm |
| Wizard | 25% | Evac/gate |
| Necromancer | 30% | FD recovery |

### Threshold Tuning

Adjust healing thresholds based on content difficulty:

```toml
heal_at_pct = 30           # Request group heal at 30% HP
self_heal_at_pct = 40      # Self-heal at 40% HP
nuke_at_pct = 60           # Only nuke above 60% mana
```

### Burn Windows

For burst damage on named targets:

1. Identify burn target (HP > 80%)
2. Enable all burn disciplines
3. Stack DPS classes
4. Time burst with mez refresh

---

## Login Sequencing

### Stagger Configuration

Prevent server overload with staggered login:

```toml
[launch]
stagger_ms = 2000           # 2s between each client
max_clients = 6             # Parallel launches
```

### Post-Login Sequencing

After login completes, TextQuest can run automation:

1. Buff cycle (Enchanters/Bards first)
2. Equipment check
3. Camp navigation
4. Combat enable

### Account Grouping

Group accounts by launch priority:

```toml
[[accounts]]
name = "warrior01"
server = "Firiona Vie"
character = "Tank01"
class = "WAR"
group = 1                  # Launch first

[[accounts]]
name = "cleric01"
server = "Firiona Vie"
character = "Healer01"
class = "CLR"
group = 2                  # Launch second
```

---

## Navigation Strategies

### Navmesh vs Straight-Line

| Method | Use When |
|--------|----------|
| Navmesh | Complex geometry, obstacles |
| Straight-line | Open areas, debug |

### Waypoint Queuing

For complex routes:

```bash
textquest.exe nav-path zone x1 y1 z1 x2 y2 z2
```

### Stuck Recovery

TextQuest includes automatic stuck detection:

1. Movement timeout detection
2. Random strafe attempt
3. FD reset fallback

---

## Performance Tuning

### Polling Intervals

```toml
[server]
polling_interval_ms = 100     # Game state read interval
spawn_update_ms = 100     # Spawn list update
```

Lower values = more responsive but higher CPU.

### IPC Latency

Monitor IPC latency in the debug screen:

- < 5ms: Excellent
- 5-10ms: Normal
- > 10ms: Check system load

### Memory Management

TextQuest monitors memory usage:

```toml
[alerts.thresholds]
memory_warning_mb = 200    # Alert if exceeds 200MB
```

---

## Automation Patterns

### CH Chain (Complete Heal)

For raid tank healing:

```bash
:ch start 1234,1235,1236 3   # 3 second interval
```

Chain timing:

| Cleric | Offset | Target |
|--------|-------|--------|
| Cleric 1 | +0s | Main tank |
| Cleric 2 | +3s | Main tank |
| Cleric 3 | +6s | Main tank |

### Mez Priority

Enchanter mez order:

1. Tash first (reduces resist)
2. Highest level add first (most dangerous)
3. Re-mez before expiration
4. Color Flux as emergency

### Pull Cycle

Standard pull sequence:

1. Puller approaches mob
2. Engages and runs back
3. Tank holds position
4. DPS focuses target
5. Healer sustains tank

---

## Optimization Checklist

- [ ] Tune pull radius for camp geometry
- [ ] Set mana reserves appropriately
- [ ] Configure stagger for login
- [ ] Enable stuck recovery
- [ ] Monitor IPC latency
- [ ] Adjust healing thresholds

---

## Next Steps

- Review [Camp Configuration Guide](Configuration.md#camps)
- See [Command Reference](Command-Reference.md)
- Check [Troubleshooting](Troubleshooting.md)