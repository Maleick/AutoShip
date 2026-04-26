# Dreadlands Farming Validation

Validation ledger for the Dreadlands Primary Camp (Lost Valley, Ancient Combine Outpost).
Tracking issue: `#1771` (parent), `#3363` (this validation capture).

Related pages:

- [Dreadlands Primary Camp](Dreadlands-Primary-Camp.md) — camp geometry, waypoints, restriction zones
- [Camp Runbooks](Camp-Runbooks.md) — consolidated planning ledger

---

## Validation Status

| Category              | State             | Notes                                                       |
| --------------------- | ----------------- | ----------------------------------------------------------- |
| DPS expectations      | Research-baseline | No live EQ session available; derived from zone/mob data    |
| Loot-per-kill         | Research-baseline | Item table and coin data from P99/EQAtlas; not live-sampled |
| Pull-sequence behavior | Research-baseline | Documented from route plan; awaits live 6-box confirmation  |
| Spawn cadence         | Research-backed   | `6:40` outdoor baseline from Project 1999 timer sources     |

Live validation is **blocked** — no EQ clients or server access in this workspace.
This page captures the pre-validation planning contract for DPS, loot, and pull-sequence so that
the first live observer can fill in measured values and close `#3363`.

---

## DPS Expectations

### Mob Stats Reference

| Mob                  | Level range | HP estimate | Resists          | Notes                                              |
| -------------------- | ----------- | ----------- | ---------------- | -------------------------------------------------- |
| `greater plaguebone` | 45–48       | ~3,000–4,500 | Undead; low FR  | Most common pull; no slow immunity                 |
| `greater spurbone`   | 45–48       | ~3,000–4,500 | Undead; low FR  | Comparable to plaguebone; slight AI variation       |
| `wraithbone champion`| 49–51       | ~5,500–7,000 | Undead; moderate FR | Priority named; harder enrage; full burn target |

HP estimates are derived from published P99 mob data and spawn-level brackets.
**Replace with measured values once live sampling is available.**

### Expected Kill Times (6-box; G1 Driver composition SK/CLR/BRD/SHM/MNK/MNK)

| Mob                   | Expected kill time | Basis                                                      |
| --------------------- | ------------------ | ---------------------------------------------------------- |
| `greater plaguebone`  | 20–35 s            | Two MNK DPS + BRD ADPS + SHM slow; skeleton HP bracket    |
| `greater spurbone`    | 20–35 s            | Same composition; comparable HP/resists                    |
| `wraithbone champion` | 45–75 s            | Full-burn with Disciplines; champion HP and damage output  |

### DPS Targets

At level 45–51 with mid-era gear:

- **Monk (each):** ~80–130 DPS sustained; ~150–200 DPS with Disciplines active
- **SK driver (snap/snare only):** minimal DPS contribution; focus on aggro and FD recovery
- **BRD ADPS:** ~+15–20% effective group DPS via Anthem of Arms and relevant haste songs
- **SHM slow:** reduces mob DPS by ~50–70%; critical for safe med between pulls

**Observation column — fill in from live run:**

| Mob                   | Measured kill time | Group DPS (approx) | Observer | Date |
| --------------------- | ------------------ | ------------------ | -------- | ---- |
| `greater plaguebone`  |                    |                    |          |      |
| `greater spurbone`    |                    |                    |          |      |
| `wraithbone champion` |                    |                    |          |      |

---

## Loot-per-Kill Observations

### Expected Coin Drop

| Mob                   | Coin range (pp) | Notes                                    |
| --------------------- | --------------- | ---------------------------------------- |
| `greater plaguebone`  | 0–2 pp          | Undead outdoor; coin is low in this tier |
| `greater spurbone`    | 0–2 pp          | Same tier                                |
| `wraithbone champion` | 2–8 pp          | Named premium; occasionally higher       |

### Expected Item Drops (research-baseline)

Undead skeletons in the 45–51 tier drop primarily from the **Kunark undead common table**.
Item drops are low-frequency on individual kills; loot value accumulates across the spawn cycle.

| Item type                     | Drop frequency | Notes                                                        |
| ----------------------------- | -------------- | ------------------------------------------------------------ |
| Bone chips                    | ~40–60%        | Primary tradeskill drop; sellable or crafted into bone armor |
| Banded/Imbued armor pieces    | ~5–10%         | Common mid-tier drops; useful for fresh characters           |
| Rusty / low-tier weapons      | ~5–10%         | Vendor trash; minor coin supplement                          |
| Fine steel / tier weapons     | ~2–5%          | Occasional upgrade or trade value                            |
| `Darkboned Gauntlets` (champ) | Rare           | Champion-only; undead proc; tradeable                        |
| Planar component tier         | Not expected   | Dreadlands undead do not drop planar loot                    |

**Randomized loot note (Frostreaver TLP):** Teek/Mischief-style random loot tables can shift
drop weights. Treat these frequencies as Kunark-default baselines; confirm actual tables on launch.

### Loot-per-Hour Projection (research-baseline)

With a `6:40` cycle and ~8–12 mobs available in the outpost loop per cycle:

- **Cycle duration:** ~10 min including pull, kill, med, and reset
- **Cycles per hour:** ~6
- **Kills per hour:** ~48–72 (single-group outpost loop)
- **Bone chip income:** ~20–40 stacks/hour (rough estimate)
- **Coin income:** ~15–40 pp/hour (undead coin is minimal)

**Observation column — fill in from live run:**

| Metric                | Measured value | Observer | Date |
| --------------------- | -------------- | -------- | ---- |
| Kills per hour        |                |          |      |
| Bone chips / hour     |                |          |      |
| Plat / hour (coin)    |                |          |      |
| Notable item drops    |                |          |      |

---

## Multibox Pull-Sequence Behavior

### Standard Pull Sequence (research-baseline)

The pull sequence below is derived from the camp geometry in
[Dreadlands Primary Camp](Dreadlands-Primary-Camp.md) and the [Camp Runbooks](Camp-Runbooks.md) restriction zones.

#### Step 1 — Med check

- Group holds at `DL-16` (courtyard center, `[900, 9000, 0]`)
- Pull only when casters are above `rest_mana_pct = 70`
- SK driver checks `pull_mana_pct = 45` before engaging the next pull

#### Step 2 — First pull (fixed plaguebone anchor, `DL-17`)

- SK moves to `DL-17` (`[970, 9050, 0]`) — primary plaguebone anchor
- Tag one `greater plaguebone` with a snap-aggro ability (not FD yet)
- Return to camp at `DL-16`; group burns the mob down
- Confirm leash: mob must follow SK inward without widening to additional spawns

#### Step 3 — West wall skeleton line (`DL-12`)

- After courtyard mobs are clear, SK moves to `DL-12` (`[575, 9360, 0]`)
- Single-tag `greater spurbone` on the west wall
- Return to `DL-16`; same burn rotation
- West wall pull should not aggro the north wall roamers if approach stays tight

#### Step 4 — East handoff pull (`DL-18`)

- SK moves to `DL-18` (`[1100, 9190, 0]`) — east handoff point
- Single-tag `greater plaguebone` or `greater spurbone` on the east side
- FD after pull if second mob tags during the approach; abort and reset if train forms

#### Step 5 — Champion watch (`DL-14` / `DL-17` area)

- `wraithbone champion` is not a scheduled pull — treat as opportunistic
- When champion is up and the group is at full mana: full-burn with Monk Disciplines
- SK uses snap-aggro, CLR pre-heals, BRD ramps Melody, SHM lands Slow before DPS opens
- Do **not** pull champion into an active fight with plague/spurbone adds

#### Step 6 — Recovery and leash check

- After each kill, SK confirms no additional mobs followed
- If `leash_radius = 700.0` is exceeded, SK FDs and lets the mob reset before re-engaging
- CLR and SHM begin med immediately; BRD maintains mana-regen song
- Cycle repeats from Step 1 once group is above `rest_mana_pct`

### Pull Controls Confirmed by Route Plan

| Control                  | Value              | Effect                                              |
| ------------------------ | ------------------ | --------------------------------------------------- |
| `pull_radius`            | 500.0              | Tags within 500u of pull point; no valley-floor mob |
| `camp_radius`            | 60.0               | Camp stack tight in courtyard center                |
| `leash_radius`           | 700.0              | Mob resets if it wanders beyond 700u of camp center |
| `return_no_aggro`        | true               | SK does not snap back to camp while still hot       |
| Hard-ignore: Gorenaire   | active             | Dragon bypass — any approach to DL-20/21 is abort  |
| Hard-ignore: giant patr. | active             | Giants see invis; any giant sighting = full stop    |

### Known Pull-Sequence Risks (research-baseline)

1. **Roamer overlap** — Giants and yetis occasionally wander into Lost Valley from the main valley.
   If a roamer appears within `pull_radius`, SK must abort the pull and hold until the roamer clears.

2. **Train from northeast** — The rotting skeleton camp northeast of the outpost is a separate pull
   loop. Accidentally tagging a rotting skeleton on the north approach can create an uncontrolled train.
   Stay inside `DL-14` on northern approaches.

3. **Champion respawn timing** — If champion respawns mid-cycle while a plaguebone pull is active,
   hold the new pull until the current fight resolves. Do not split attention.

4. **Mana deficit** — Undead skeletons hit for moderate damage at 45–51; a SHM slow miss or CLR
   distraction (roamer) can push the group below the safe med threshold. If mana drops below 50%
   mid-cycle, hold pulls until 70% is restored.

### Observation Column — Fill in from Live Run

| Pull step            | Observed behavior | Issues seen | Observer | Date |
| -------------------- | ----------------- | ----------- | -------- | ---- |
| DL-17 plaguebone     |                   |             |          |      |
| DL-12 west spurbone  |                   |             |          |      |
| DL-18 east handoff   |                   |             |          |      |
| Champion burn        |                   |             |          |      |
| Leash / reset        |                   |             |          |      |
| Roamer abort         |                   |             |          |      |

---

## Acceptance Criteria Mapping

| Criterion                                       | State              |
| ----------------------------------------------- | ------------------ |
| DPS expectations recorded from live observation | Research-baseline; observation table ready for live fill-in |
| Loot-per-kill observations noted                | Research-baseline; observation table ready for live fill-in |
| Multibox pull-sequence behavior documented      | Done — full sequence with controls, risks, and observation column |
| `docs/wiki/Dreadlands.md` updated               | Done (this file)   |

Live observation values must be captured during the first 6-box Dreadlands run and entered into the
observation tables above. Once all three observation tables have at least one completed row, `#3363`
can be closed and `#1771` updated accordingly.
