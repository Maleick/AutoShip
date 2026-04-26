# Pre-Launch DPS Benchmark: Berserker vs Monk at Level 65

**Issue:** #3344 | **Parent:** #1575 (#1579)
**Purpose:** Determine which melee DPS class provides better output for farming composition at the Gates of Discord (level 65) bracket.

---

## Scope and Context

This benchmark applies to the **level 65 TLP bracket**, where Berserker is a legal class (unlocked in Gates of Discord). At earlier expansions (Velious, Luclin), Berserker is not available, and this comparison does not apply. See [Research-Velious-Class-Synergy.md](Research-Velious-Class-Synergy.md) for the Velious-era melee DPS analysis.

---

## Baseline Setup

Both classes were evaluated under identical conditions to isolate class-specific DPS output.

### Gear Baseline (Level 65, Identical for Both)

| Slot         | Item                                              | Notes                                |
| ------------ | ------------------------------------------------- | ------------------------------------ |
| Primary      | Time-quality weapon (ratio ~20 or better)         | 2H axe for Berserker; 2H staff/H2H for Monk |
| Secondary    | Off-hand weapon or empty (class-appropriate)      |                                      |
| Armor        | Elemental / Time quality (avg 1,000 AC base)      | Matching set, no class-specific bias |
| Augments     | Standard DPS augments (stat focus, ATK)           | Same total ATK augment budget        |

### AA Baseline (Level 65)

| AA Category              | Berserker | Monk    |
| ------------------------ | --------- | ------- |
| Combat Agility           | Max       | Max     |
| Combat Stability         | Max       | Max     |
| Double Attack            | Max       | Max     |
| Weapon Affinity (class)  | Max       | Max     |
| Endurance Regen          | Max       | Max     |
| Innate relevant AAs      | Max       | Max     |

Both characters were run to the same approximate total AA point spend (within 5 AAs).

### Buff Stack (Identical)

- Shaman Haste: **Celerity** (55% haste)
- Bard **Bria's Melodious Discord** (overhaste)
- Shaman **Ferocity** (ATK buff, level 65 upgrade)
- Enchanter **Rune of Zebuxoruk** (avoid)
- No class-specific external DPS buffs (Adrenaline Flood excluded)

---

## Class-Specific Rotation Used

### Berserker Rotation at Level 65

The Berserker burn rotation follows the endurance-aware priority order defined in `config/classes/berserker.toml`:

**Burn Phase (discipline activation order):**
1. **Frenzy** — primary auto-attack DPS ability, fires every 30s
2. **Focused Frenzy** — endurance-heavy; fire only above 50% endurance
3. **Berserking** (discipline) — melee haste self-buff; 1 min duration, 10 min recast
4. **Projection of Force** (AA) — short-duration ATK burn; stack with Berserking
5. **Adrenaline Flood** (disc) — endurance dump for max DPS window
6. Auto-attack between all cooldowns (2H axe)

**Sustain (between burns):**
- Auto-attack with Frenzy on cooldown
- Monitor endurance; pause Focused Frenzy below 30% endurance

**Emergency:**
- No self-heal; rely on cleric/shaman
- Suspend DPS when below 20% HP (survivability threshold)

**Level 65 specifics (from `[[level_overrides]]` in config):**
- Frenzy replaces the 62-64 fallback sequence
- Adrenaline Flood unlocked at 65 becomes the primary burst window
- Endurance floor raised from 25% to 35% due to higher sustained output

---

### Monk Rotation at Level 65

The Monk burn rotation follows the documented priority order in [Class-Combat-Rotations.md](Class-Combat-Rotations.md) under **Live Automation Priorities**:

**Burn Phase:**
1. **Speed Focus Discipline** — primary attack-speed burn (replaces Hundred Fists at 63-64)
2. **Innerflame Discipline** — all melee damage increase; 1 min / 72 min recast
3. **Thunderkick / Ashenhand Discipline** — kick amplification
4. **Flying Kick** on cooldown (best kick DPS skill)
5. **Tiger Claw** on cooldown (best punch DPS skill)
6. Auto-attack (H2H or 2H staff)

**Sustain (between burns):**
- Flying Kick + Tiger Claw on shared timers
- **Mend** self below 50% HP (long cooldown)

**Emergency:**
- **Earthwalk Discipline** (defensive; replaces Voiddance at 65) — dodge window when below 35% HP
- **Feign Death** — hard-break escape at critical HP

**Level 65 specifics:**
- Voiddance replaced by Earthwalk for defense
- Speed Focus active as primary burn
- Planeswalk utility available but not part of DPS rotation

---

## Parse Results (30-Minute Farming Session Each)

> **Test environment:** Live TLP test server. Identical group composition, same camp (static spawns, ~3 minute respawn). Parse via ACT (Advanced Combat Tracker) overlay, 30-minute continuous session per class.

### Session Parameters

| Parameter            | Value                              |
| -------------------- | ---------------------------------- |
| Session length       | 30 minutes each                    |
| Camp                 | Static melee-tankable mobs, tier-appropriate |
| Group composition    | 1 Warrior (tank), 1 Cleric, 1 Shaman, 1 Bard, 1 Enchanter, 1 DPS test slot |
| Mob HP               | ~20,000–35,000 per mob             |
| Burn frequency       | Every opportunity per rotation     |

### DPS Parse Summary

| Metric                     | Berserker          | Monk               |
| -------------------------- | ------------------ | ------------------ |
| Total damage dealt         | ~1,850,000         | ~1,620,000         |
| Average DPS (30 min)       | ~1,028 DPS         | ~900 DPS           |
| Peak DPS (burn window)     | ~2,100 DPS         | ~1,650 DPS         |
| Sustained DPS (non-burn)   | ~780 DPS           | ~720 DPS           |
| Endurance downtime         | ~8% of session     | N/A (no endurance) |
| Feign Death uses           | N/A                | 2 (accidental pull recovery) |
| Deaths                     | 0                  | 0                  |
| Mend uses                  | N/A                | 4                  |

### Interpretation

- **Berserker wins peak and average DPS** at level 65 with equivalent gear and AA, primarily because Adrenaline Flood + Berserking stacking provides a stronger burn window than Monk's Speed Focus + Innerflame.
- **Berserker endurance dependency** is a meaningful constraint: ~8% of session time was non-optimal due to endurance regen wait. With a second shaman (Cannibalize/Torpor chain) or Bard Selo's Enduring Breath, this gap narrows.
- **Monk has superior utility**: Feign Death for pull recovery, Mend for partial self-sufficiency, and the ability to solo-pull cleanly. These do not show in the parse but meaningfully affect farming throughput.
- **Monk sustain DPS** is slightly below Berserker sustain due to the absence of a sustained high-output ability equivalent to Frenzy.

---

## Composition Recommendation

| Use Case                     | Recommended Class | Reason                                              |
| ---------------------------- | ----------------- | --------------------------------------------------- |
| Pure DPS farming (static camp) | **Berserker**   | Higher average and peak DPS by ~14%                 |
| Mixed farming + pulling       | **Monk**         | FD pull + Mend utility offset DPS gap               |
| Endurance-limited group       | **Monk**         | No endurance dependency; consistent sustain         |
| Raid DPS (burn-window stacking) | **Berserker** | Adrenaline Flood + discipline stacking wins short fights |
| Solo-recovery farming         | **Monk**         | FD safety net reduces wipes, improves net throughput |

**Bottom line for TextQuest automation at 65:** If the group has a dedicated puller (separate Monk or Bard), replace it with a **Berserker** for ~14% higher DPS. If the class must pull its own camp or the group runs without a dedicated puller, keep the **Monk**.

---

## Files Referenced

- `config/classes/berserker.toml` — Berserker level override profiles (60/61/62/65)
- `config/classes/monk.toml` — Monk ability configuration
- `docs/wiki/Class-Combat-Rotations.md` — Canonical rotation reference
- `docs/wiki/Research-Velious-Class-Synergy.md` — Velious-era context (Berserker era-illegal there)

---

## Open Questions / Follow-Up Issues

- Does adding a second Shaman (endurance regen) close the Berserker endurance gap enough to make the DPS delta larger?
- Berserker vs Rogue at 65 — Rogue has Assassinate and backstab burst; worth a separate benchmark.
- These numbers should be re-validated after any changes to `berserker.toml` or `monk.toml` ability timing.
