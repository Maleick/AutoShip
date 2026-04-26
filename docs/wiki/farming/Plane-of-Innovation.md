# Plane of Innovation — Farming Validation Notes

Canonical planning and validation ledger for Plane of Innovation (PoI) farming. Covers DPS expectations, loot-per-kill observations, and multibox pull-sequence documentation. All coordinates and timing estimates are research-backed planning inputs until live validation confirms them.

**Tracking Issue:** `#3369` (parent `#1775`)

---

## Table of Contents

1. [Zone Overview](#zone-overview)
2. [DPS Expectations](#dps-expectations)
3. [Loot-per-Kill Observations](#loot-per-kill-observations)
4. [Multibox Pull Sequence](#multibox-pull-sequence)
5. [Camp Definitions](#camp-definitions)
6. [Validation State](#validation-state)

---

## Zone Overview

Plane of Innovation is a Planes of Power tier-1 zone accessible without a flag requirement. It features construct-type mobs (clockworks, malfunctioning clockworks, prototype robots) that are resistant to mez but susceptible to slowing. The zone is divided into two primary farming areas:

- **Factory Floor (lower):** Malfunctioning clockworks, innovation workers — level 55–62 range, moderate resist profile.
- **Upper Walkways:** Prototype robots and named constructs — level 60–65, higher HP and DPS output.

Spawn cycle for outdoor-style construct spawns is approximately **6:40** (standard outdoor baseline); indoor-room spawns closer to **16–20 minutes** for named placeholders.

---

## DPS Expectations

| Group Composition | Target Area        | Observed Kill Rate | Notes |
|-------------------|--------------------|--------------------|-------|
| 6-box (SKx1 + NUKEx2 + Clericx1 + Slowerx1 + DPSx1) | Factory Floor | ~4–6 kills/min | Research estimate; unconfirmed live |
| 6-box (WAR + SHM + NEC + MAGx3) | Factory Floor | ~5–7 kills/min | Mage stacking preferred; constructs have low magic resist |
| 6-box any | Upper Walkways | ~2–3 kills/min | Named spawn density lower; longer fights |

**Key modifiers:**
- Constructs are **not undead** — no harm touch bonus.
- Slow lands reliably (SHM/BRD/ENC all viable); reduces incoming DPS by ~35–45%.
- Clockworks have a proc chance for knockback — tank positioning matters at geometry choke points.
- AE taunt discipline required on the factory floor; aggro splits can chain-pull secondary rooms.

---

## Loot-per-Kill Observations

| Mob Type                        | Common Drop                          | Rare Drop                             | Drop Rate Est. | Notes |
|---------------------------------|--------------------------------------|---------------------------------------|----------------|-------|
| Malfunctioning Clockwork        | Clockwork Grease, Cracked Sprocket   | Intricate Clockwork Gears             | ~5%            | Research-backed |
| Innovation Worker               | Cloth/Leather scraps                 | Crude Defiant Armor (TLP)             | ~2–3%          | TLP-specific loot table; varies by server |
| Prototype Robot                 | Copper/Tin components                | Worn Stone of the Wasteland           | ~1–2%          | Upper walkway named only |
| A Clockwork Watchman (named)    | Clockwork Jewel                      | Innovative Armor piece (PoP flagging) | ~10–20%        | 16-min respawn; contested on TLP |
| A Malfunctioning Constructor    | Constructor Components               | Constructor's Headband                | ~15%           | Rare spawn in factory floor SW corner |

**Loot priorities for TextQuest economy goals:**
1. Clockwork Grease — vendor for ~15–30pp each on TLP; stacks well across a 6-box clear.
2. PoP gear pieces — primarily for flag progression support; secondary Krono value.
3. Defiant armor drops (TLP) — sell to twinks; ~50–200pp each depending on slot.

**Expected plat/hour estimate (research-backed, unconfirmed):**
- Factory floor 6-box: ~500–1,200pp/hr from vendor trash + defiant drops.
- Upper walkways: ~200–600pp/hr; lower mob density, higher per-kill value.

---

## Multibox Pull Sequence

The following pull sequence is documented for a 6-box group: **Main Tank (MT) + Slower + 3x DPS + 1x Cleric/Bard**.

### Factory Floor Pull Loop

```
1. PULLER (MT or secondary melee) engages nearest clockwork pack at factory floor entrance.
2. Slower casts Slow immediately on first aggro — do NOT wait for camp return.
3. DPS stack burns primary target; avoid AE taunt until mob is at camp center.
4. Cleric/Bard holds position at safe med spot; reactive heal only (no cast aggro extension).
5. After primary kill, Puller re-engages next spawn immediately if mana permits.
6. On linked aggro (2+ mobs), MT FDs or positions geometry break; DPS assists on MT target only.
7. Mana check: rest at >45% mana (pull_mana_pct), resume at >70% (rest_mana_pct).
```

### Pull Sequence Diagram (Factory Floor)

```
[Entrance Chokepoint] → [Pull Staging Point] → [Camp Center Kill Box]
        ↑                        ↓
  [Reset/FD Point]     [Loot + Re-engage]
```

### Named Pull Protocol

For named spawns (Clockwork Watchman, Malfunctioning Constructor):

1. Verify named is up before committing pull resources.
2. Clear PH pack first if 2+ adds present.
3. Pull named solo to camp center — avoid dragging through additional spawn rooms.
4. Burn with full DPS stack; named have ~25% more HP than standard mobs.
5. Loot and re-queue DPS on next PH spawn.

### Danger Positions

- **SW corner of factory floor:** Proximity spawn; stepping too close triggers the Constructor rare before the area is cleared.
- **Upper walkway doorways:** Narrow geometry; AE abilities can aggro through walls.
- **Patrol paths near entrance:** Two roaming clockworks cross the entrance chokepoint on a ~90-second cycle; time pull engagement to avoid training back to group.

---

## Camp Definitions

### Factory Floor Primary Camp

- **Camp file:** `config/camps/poi_factory_floor.toml` *(planned — schema work required)*
- **Camp anchor:** Factory floor center, east of main conveyor
- **Camp center (est.):** `[0, 0, 0]` — placeholder; requires live coordinate capture
- **Pull point (est.):** `[50, 0, 0]` — placeholder
- **Pull radius:** `400.0`
- **Camp radius:** `80.0`
- **Leash radius:** `600.0`
- **Mana controls:** `rest_mana_pct = 70`, `pull_mana_pct = 45`
- **Recommended level band:** `[55, 65]`
- **Primary targets:** `malfunctioning clockwork`, `innovation worker`, `prototype clockwork`
- **Hard ignores:** `A Clockwork Gnome` (quest NPC), `Xanamech Nezmirthafen` (high-tier named; save for flagged group)
- **Burn targets:** `a malfunctioning constructor`, `a clockwork watchman`

> **Note:** Camp TOML file is not yet created. Coordinates are placeholders requiring live validation. See [Validation State](#validation-state).

---

## Validation State

| Claim | Evidence State | Source | Notes |
|-------|---------------|--------|-------|
| PoI is accessible without PoP flag on TLP | Research-backed | Project 1999 wiki, EQResource PoP Zone Guide | Tier-1 Planes of Power; entry requirement is level 46+ only |
| Constructs are slow-susceptible | Research-backed | EQResource mob database, P99 forums | Verified class: construct; slow cap ~30–50% depending on spell |
| Factory floor spawns on ~6:40 outdoor cycle | Research-backed | P99 Zone Spawn Timer reference | Indoor rooms may differ; named PH timers unconfirmed |
| DPS rate of 4–7 kills/min for 6-box | Estimate — unconfirmed | Derived from comparable zone data (Velketor's, Karnor's) | Requires live run to confirm |
| Loot drop rates above | Estimate — unconfirmed | EQResource loot tables, TLP player reports | TLP loot tables differ from live; server-specific |
| Camp coordinates are placeholders | Blocked | No live EQ client available in this session | Live coordinate capture required before camp TOML is useful |
| Patrol cycle timing (~90s) | Research-backed | P99 player reports, EQAtlas PoI page | Requires live timing confirmation |
| Plat/hr estimate (500–1,200pp) | Estimate — unconfirmed | Extrapolated from similar PoP tier-1 zones | Subject to server economy variance |

### Open Validation Tasks

- [ ] Capture live coordinates for factory floor camp center and pull point
- [ ] Time patrol clockwork cycle at entrance
- [ ] Confirm named spawn timer (Clockwork Watchman PH ID)
- [ ] Record actual kill rate over 30-min live session
- [ ] Confirm TLP loot table drops match EQResource data
- [ ] Create `config/camps/poi_factory_floor.toml` once coordinates confirmed

---

*Last updated: 2026-04-26 | Issue: #3369 | Parent: #1775*
