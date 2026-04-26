# East Commonlands Farming Validation

Parent issue: `#1779`

This page is the canonical validation ledger for East Commonlands (EC) farming
observations. It records DPS expectations, loot-per-kill baselines, and multibox
pull-sequence notes relevant to the Freeport Human core and Neriak feeder cohorts.
All entries here are research-backed inputs unless explicitly marked as
live-validated.

---

## Zone Overview

| Field            | Value                                                         |
| ---------------- | ------------------------------------------------------------- |
| Zone             | East Commonlands (`eastcommons`)                              |
| Era              | Classic (available at TLP launch)                            |
| ZEM              | 100% (baseline, no ZEM bonus)                                |
| Target level range | 4-12 (primary), up to 15 with weak-link carry               |
| Multi-group capacity | 2 groups across orc camps + Dervish Cutthroat spawn points |
| Primary value    | XP ramp, Dervish Cutthroat belts (turn-in quest), EC tunnel trading |

---

## DPS Expectations

All values are research estimates for the TextQuest 36-box roster configuration
(G1 driver: SK/CLR/BRD/SHM/MNK/MNK; G2-G4 melee: WAR/CLR/BRD/SHM/MNK/MNK).

| Level range | Group config | Estimated kill rate | Notes |
| ----------- | ------------ | ------------------- | ----- |
| 4-6         | 1 group partial (4 toons) | 4-6 kills/min | Orc pawns and Dervish Cutthroat spawn density supports this cadence at low level |
| 6-9         | 1 full group (6 toons)    | 6-10 kills/min | Monks come online; SK FD-pull enables safe multi-pull |
| 9-12        | 1-2 groups                | 8-12 kills/min | BRD pulling with SHM slow allows back-to-back chain; CLR efficiency bottleneck |

> These figures are **planning inputs only**. Live TextQuest validation runs against
> actual spawn density and mob HP are required before treating these as baselines.

---

## Loot-Per-Kill Observations

### Dervish Cutthroats

- Drop **Dervish Cutthroat Belt** at a reported 50-70% rate (research-backed from
  P99 community data; see `docs/wiki/P99-Zone-Guide.md`).
- Quest turn-in at Freeport yields plat reward — exact value varies by server rule
  set. On Free Trade TLP the belt itself may sell for small amounts in the EC
  tunnel.
- Secondary drops: cloth armor pieces, rusty weapons, vendor trash (1-5 cp each).
- **Loot priority**: save belts for quest turn-in rather than vendoring directly.

### Orc Camps (Orcs along the Nektulos / Freeport road)

- Drops: crude bone chips (SKs/NEC vendor demand), rusty weapons, low-end cloth
  and leather armor.
- Bone chips: minor value unless a necromancer in the group needs them; on Free
  Trade servers cross-character shipment is trivial.
- Plat per hour estimate: 1-3 pp/hour per group at level 6-10, dominated by
  Dervish belt quest turn-ins.

### Named Mobs

- No high-value named unique to EC at this level range. EC is an XP and quest-loot
  zone rather than a named-item farming target.

> All loot data is derived from P99 community records and `docs/wiki/Frostreaver-Farming-Guide.md`.
> Live TextQuest observation runs are needed to confirm drop rates on Frostreaver
> (Free Trade / Randomized Loot rules may affect drop tables).

---

## Multibox Pull-Sequence

### Recommended Pull Sequence (1-2 groups, 4-12)

1. **Scout with BRD or SK** — Bard or Shadowknight identifies camp population.
   Use SK FD-pull to test for adds before committing the group.
2. **Single-mob pull preferred** — Dervish Cutthroats have a moderate social
   range; pull one at a time until the camp boundary is confirmed safe.
3. **SHM slow immediately on pull** — Slowing at max range prevents spike damage
   on the CLR during the engage window.
4. **MNK DPS burns** — Monks apply Flying Kick immediately after slow lands;
   expected kill time 10-25 seconds per mob depending on HP and SHM slow success.
5. **BRD twists** — Bard cycles Selos + resist song + ADPS while monks are
   engaged. At level 6-8 BRD twisting 2-3 songs is realistic.
6. **Camp loot before next pull** — Designate one character (SK or a monk) as
   looter; loot while BRD or second SK positions for the next pull.

### Neriak Feeder Merge Window

Per `docs/wiki/Frostreaver-Starting-City-Logistics.md`:

- Neriak cohort should absorb into the Freeport side in East Commonlands by
  level 4-6.
- When merged, the combined team (partial Freeport core + Neriak chars) can
  hold 2 simultaneous orc / Dervish spawn points before the XP value of EC
  flattens enough to justify moving to Befallen or Estate of Unrest.

### EC Tunnel Use During Farm Runs

- The EC tunnel is the Antonica trading hub; expect traffic and occasional
  player interference near the tunnel entrance.
- Camp positions should be set east of the tunnel entrance (toward the Nektulos
  Forest zoneline) to avoid body-blocking and social-aggro from passing players.
- If the zone is contested, `West Commonlands` orc camps are the next-best
  fallback before escalating to Befallen.

---

## Validation Checklist

The items below must be confirmed via attended live runs before this zone is
promoted from research-backed to live-validated status.

| Criterion | Status | Notes |
| --------- | ------ | ----- |
| DPS expectations recorded | Research-backed | See DPS table above; live run needed |
| Loot-per-kill observations | Research-backed | Belt drop rate, plat/hr unconfirmed on Frostreaver rule set |
| Multibox pull-sequence documented | Research-backed | Sequence derived from class comp; live confirmation pending |
| EC tunnel camp-position safety | Not validated | Player traffic pattern unknown for Frostreaver launch day |
| Neriak feeder merge cadence | Not validated | Depends on actual launch-day character counts and travel times |

---

## Related Pages

- `docs/wiki/Frostreaver-Farming-Guide.md` — 36-box roster and era-by-era zone picks
- `docs/wiki/Frostreaver-Starting-City-Logistics.md` — starting city split, EC merge plan
- `docs/wiki/P99-Zone-Guide.md` — ZEM table and per-zone level recommendations
- `docs/wiki/Camp-Runbooks.md` — generic camp loop and pull automation reference
