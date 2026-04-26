# Howling Stones (Charasis)

Research baseline for issue `#3386` (child of `#1781`). This page is the
canonical farming validation ledger and pull-safety reference for Howling
Stones (also called Charasis). It records what the repository already supports,
what comes from published research, and what still needs live TextQuest proof.

Do not promote Howling Stones routing, spawn, DPS, or loot claims beyond the
evidence state recorded here.

---

## Zone Overview

| Field | Value | Source |
| --- | --- | --- |
| Zone short name | `charasis` | EQ zone registry |
| Also known as | Howling Stones | Common name |
| Era | Kunark | EQ release history |
| ZEM (XP modifier) | 113% | `docs/wiki/P99-Zone-Guide.md` |
| Level range | 51-59 | `docs/wiki/P99-Zone-Guide.md` |
| Multi-group capacity | 3 groups (entrance, basement, north/west/south wings) | `docs/wiki/P99-Zone-Guide.md` |
| Key required | Key to Charasis | Zone gating |
| Faction impact | Lowers Venril Sathir faction | `docs/wiki/P99-Zone-Guide.md` |
| Recommended group comp | SK/CLR/BRD/SHM/MNK/MNK (G1 driver setup) | `docs/wiki/Frostreaver-Farming-Guide.md` |

---

## Current Evidence State

| Claim | Evidence state | Repo basis | Notes |
| --- | --- | --- | --- |
| Howling Stones South Wing yields 400-700pp/hour from Hand of the Reaper and Fingerbone Hoop. | Research-backed | `docs/wiki/Frostreaver-Farming-Guide.md` | Planning input only; no live TextQuest kill or loot log confirms this baseline. |
| Zone XP modifier is 113%, placing it among the top Kunark group dungeons. | Research-backed | `docs/wiki/P99-Zone-Guide.md` | Consistent with public EQ server data; not yet confirmed via live TextQuest XP measurements. |
| South wing is the most profitable area. | Research-backed | `docs/wiki/P99-Zone-Guide.md`, `docs/wiki/Frostreaver-Farming-Guide.md` | Both sources agree; needs live per-wing DPS/loot comparison. |
| Key to Charasis is required for entry. | Research-backed | `docs/wiki/P99-Zone-Guide.md` | Key acquisition automation is not yet tracked in the repo; treat entry as driver-attended. |
| G1 group (SK/CLR/BRD/SHM/MNK/MNK) is the recommended formation for precise FD pulls here. | Research-backed | `docs/wiki/Frostreaver-Farming-Guide.md` | FD pull discipline is documented but not yet validated in a live Howling Stones pull log. |
| 1-2 groups should run Howling Stones while other groups hold Old Sebilis. | Research-backed | `docs/wiki/Frostreaver-Farming-Guide.md` | Multi-site split strategy; live concurrency test not yet performed. |
| Zone has three separable areas: entrance (51-56), basement (53-59), north/west/south wings (56-59). | Research-backed | `docs/wiki/P99-Zone-Guide.md` | Wing boundaries come from the zone guide; no TextQuest aggro radius or room boundary measurements on file. |
| Loot drops include Hand of the Reaper, Fingerbone Hoop, Enshrouded Veil, Golden Bracer. | Research-backed | `docs/wiki/P99-Zone-Guide.md`, `docs/wiki/Frostreaver-Farming-Guide.md` | Free-trade server values unconfirmed; no checked-in named config or drop-rate baseline exists for this zone yet. |
| No TextQuest camp config exists for Howling Stones. | Confirmed gap | `config/camps/` directory | `config/camps/` has no `charasis*.toml` or `howling_stones*.toml` file as of this issue. |

---

## DPS Expectations (Research Baseline)

The following are planning inputs from `docs/wiki/Frostreaver-Farming-Guide.md`
and `docs/wiki/P99-Zone-Guide.md`. None have been confirmed by live TextQuest
pull logs.

| Metric | Research estimate | Notes |
| --- | --- | --- |
| Plat/hour (South Wing) | 400-700pp | Hand of the Reaper + Fingerbone Hoop market value on free-trade TLP |
| XP rate | FAST (113% ZEM) | Equivalent to Velketor's, Chardok, Kael Drakkel |
| Primary XP level band | 51-59 | Entrance-to-south-wing sweep |
| Optimal group count | 1-2 groups | Part of a multi-zone split alongside Old Sebilis |
| Kill rate qualifier | FD pull discipline required | South and west wings punish sloppy AE aggro |

Live validation targets (not yet recorded):

- [ ] Kill rate (kills/hour) measured from TUI observability metrics
- [ ] XP-per-kill average captured from at least one attended session
- [ ] Plat-per-session compared to research estimate (400-700pp range)

---

## Loot-Per-Kill Observations (Research Baseline)

The following loot table is assembled from existing repo planning docs. No
live-drop or drop-rate data from TextQuest sessions exists yet.

| Item | Wing / Source | Est. value (free-trade TLP) | Notes |
| --- | --- | --- | --- |
| Hand of the Reaper | South wing | High | Primary named drop; exact mob source needs live confirmation |
| Fingerbone Hoop | South wing | Moderate-High | Slot-competitive; bazaar price varies by server pop |
| Enshrouded Veil | General | Moderate | Caster utility piece |
| Golden Bracer | General | Moderate | Common auction item |

Live validation targets (not yet recorded):

- [ ] Per-kill loot log from at least one full South Wing rotation
- [ ] Named spawn timer baseline for Hand of the Reaper source mob
- [ ] Drop rate sample (n ≥ 20 kills) for each primary loot item

---

## Multibox Pull-Sequence and Pull-Safety Notes

### Recommended Pull Formation (Research Baseline)

Based on `docs/wiki/Frostreaver-Farming-Guide.md` and zone guide notes:

- **Driver group:** G1 (SK/CLR/BRD/SHM/MNK/MNK) handles all Howling Stones pulls.
- **Pull method:** FD pull (SK or MNK Feign Death) to split dungeon mobs cleanly.
- **Med spot discipline:** Hold a tight camp stack while the puller is active.
  Do not advance until the pull lands at camp.

### Higher-Risk Pull Notes

The following situations are flagged as elevated-risk in the research baseline.
None have been confirmed or refined by live TextQuest pull logs.

| Scenario | Risk level | Notes |
| --- | --- | --- |
| South wing corridor pulls | High | Tight geometry; multiple roamers can link. FD required. |
| West wing (level 57-59) | High | Harder mob level band; healer mana drain risk on linked pulls. |
| North wing entry pulls | Moderate | More open than south/west but still punishes AE aggro. |
| Basement pulls (53-59) | Moderate-High | Basement has limited escape routes; bad pulls corner the group. |
| Entrance pulls (51-56) | Moderate | Lower mob level; safer for camp establishment, but roamers still link. |
| Any AE in tight corridors | High | AE nukes and procs risk pulling adjacent rooms. BRD songs should be range-managed. |
| Zone entry / keying | Driver-attended required | Key to Charasis must be in inventory; no automation for key-gated entry exists yet. |

### Suggested Pull-Safety Checklist (Research Baseline)

The following pre-pull checklist comes from general TextQuest dungeon camp
practice documented in `docs/wiki/Combat-and-Camp-Loop.md` and the Sebilis and
Velketor's runbooks. It has not been validated against a live Howling Stones
session.

1. Confirm `Key to Charasis` is in driver's inventory before zoning.
2. Establish med spot in the entrance hallway before pushing into basement or wings.
3. Use FD (SK or MNK) to split named or pathing mobs before committing the full group.
4. Set `leash_radius` conservatively on first session to avoid pulling through doorways.
5. Assign one healer to watch driver HP during the pull; do not start next pull below `rest_mana_pct`.
6. Do not run BRD AE songs inside tight south/west corridors until room is clear.
7. Treat any unexpected zone-wide social aggro as a camp-break condition; use evac (DRU/PAL) as the recovery path.

---

## Repository Gaps

| Gap | Tracking issue | Notes |
| --- | --- | --- |
| No `config/camps/charasis*.toml` camp config exists | `#3386` (this issue) | A future sub-issue should add a camp config once live pull-point measurements are available. |
| No named config in `config/named_mobs/` for Howling Stones | `#3386` (this issue) | Hand of the Reaper source mob and timer window need live sampling before a named config is worth checking in. |
| No live pull log or DPS measurement on file | `#3386` (this issue) | Research baseline only; requires attended TextQuest session to produce evidence. |
| Key-to-Charasis acquisition not automated | Open gap | Entry is driver-attended; no key-quest automation tracked in the repo. |

---

## Related Pages

- `docs/wiki/Frostreaver-Farming-Guide.md` — Multibox farming strategy, level 50-60 zone table, group formation
- `docs/wiki/P99-Zone-Guide.md` — Zone XP modifiers, level ranges, loot notes
- `docs/wiki/Sebilis-Farming-Validation.md` — Reference format for this validation ledger
- `docs/wiki/Velketors-Labyrinth-Frenzy-Camp.md` — Comparable dungeon camp with waypoint and restriction doc pattern
- `docs/wiki/Combat-and-Camp-Loop.md` — Camp loop mechanics and pull discipline
