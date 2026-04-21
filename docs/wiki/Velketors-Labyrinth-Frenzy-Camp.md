# Velketor's Labyrinth Frenzy Camp

Research baseline for issue `#1720`. This page keeps the richer Velketor
waypoint, restriction, route, and travel notes next to the loader-compatible
runtime config in `config/camps/velketors_labyrinth_frenzy.toml`.

## Runtime Camp Config

The runtime loader only consumes the baseline TOML fields today. It does not
yet model named waypoints, restriction polygons, or ordered route segments.
Those stay documented here so operators can document richer waypoint and
restriction notes without inventing untracked runtime keys.

Current runtime baseline:

| Field | Value | Notes |
| --- | --- | --- |
| `camp_center` | `[360.0, 40.0, 0.0]` | Derived "safe hall" med stack beside Frenzy and before the pit drop. |
| `pull_point` | `[405.0, 60.0, 0.0]` | Frenzy handoff ramp. |
| `pull_radius` | `165` | Covers Frenzy plus the upper spider lip without reaching Bled/Bledrek or Lord Bob. |
| `camp_radius` | `30` | Tight safe-hall med footprint. |
| `leash_radius` | `110` | Short leash before the pit or upper-dogs branch. |
| `rest_mana_pct` | `70` | Recover after spike spider pulls. |
| `pull_mana_pct` | `45` | Conservative restart threshold. |
| `level_range` | `[57, 60]` | Matches the Frenzy progression band. |
| `return_no_aggro` | `true` | Keep the puller from snapping back across slippery geometry while hot. |

The runtime loader does not yet model named waypoints, restriction polygons, or
ordered route segments. Those stay documented here so
`velketors_labyrinth_frenzy.toml` remains loader-safe.

## Travel, Level, And Faction Constraints

- Access is from Great Divide; treat the dungeon approach as driver-attended.
- The route notes assume a level range of `57-60`.
- Named Icepaw kobolds treat this as faction-neutral farming and list
  `Velketor (-0)`.
- Levitation does not save this route. The main hazards are slippery ramps,
  pit drops, see-invis spiders, and optional named branches.

## Research Waypoint Lattice

| # | Waypoint | Coordinates | Confidence | Purpose | Source |
| --- | --- | --- | --- | --- | --- |
| 1 | `safe_hall_med` | `(360, 40)` | Derived | Primary med stack before the pit. | Frenzy route notes. |
| 2 | `frenzy_handoff` | `(405, 60)` | Derived | Primary pull handoff. | Frenzy route notes. |
| 3 | `crystal_eyes` | `(479, 2)` | Exact | Named spider anchor. | `Crystal Eyes` page. |
| 4 | `broodling_corner` | `(430, 32)` | Derived | Frenzied broodling turn. | Frenzy route notes. |
| 5 | `stalker_corner` | `(448, 52)` | Derived | Frenzied stalker turn. | Frenzy route notes. |
| 6 | `upper_spider_lip` | `(430, 110)` | Derived | Optional upper-spider edge. | Frenzy route notes. |
| 7 | `crystal_fang_ramp` | `(440, 154, 88)` | Exact | Slippery ramp to Crystal Fang. | `Crystal Fang` page. |
| 8 | `pit_lip` | `(318, -15)` | Derived | Pit edge; no-med boundary. | Pit route notes. |
| 9 | `pit_drop` | `(280, -70)` | Derived | Pit descent. | Pit route notes. |
| 10 | `pit_first_room` | `(255, -120)` | Derived | First room after dropping into the pit. | Pit route notes. |
| 11 | `pit_second_room` | `(220, -170)` | Derived | Straight/right split room. | Pit route notes. |
| 12 | `pit_safe_room` | `(180, -230)` | Derived | Safe setup room in the pit branch. | Pit route notes. |
| 13 | `pit_fourth_room` | `(155, -270)` | Derived | Corridor room after the safe room. | Pit route notes. |
| 14 | `pit_fifth_room` | `(145, -300)` | Derived | Hallway room before the next junction. | Pit route notes. |
| 15 | `pit_sixth_junction` | `(165, -335)` | Derived | Split toward Jelek or upper dogs. | Pit route notes. |
| 16 | `jelek_room` | `(134, -322)` | Exact | Jelek room. | `Jelek Icepaw` page. |
| 17 | `khelkar_dog_room` | `(227, -348)` | Exact | Upper-dogs ramp room. | `Khelkar Icepaw` page. |
| 18 | `marlek_room` | `(199, -361)` | Exact | Alternate upper-dogs named room. | `Marlek Icepaw` page. |
| 19 | `gregendek_room` | `(212, -390)` | Exact | CH-capable cleric named room. | `Gregendek Icepaw` page. |
| 20 | `tpos_room` | `(190, -413)` | Exact | Upper-dogs shadowknight room. | `Tpos Icepaw` page. |
| 21 | `ular_room_west` | `(121, -220)` | Exact | Optional Ular branch. | `Ular Icepaw` page. |
| 22 | `ular_room_east` | `(308, -213)` | Exact | Alternate Ular room. | `Ular Icepaw` page. |
| 23 | `brood_master_west` | `(-36, 71)` | Exact | Brood Master branch. | `The Brood Master` page. |
| 24 | `brood_master_east` | `(197, 66)` | Exact | Alternate Brood Master room. | `The Brood Master` page. |
| 25 | `brood_mother_south` | `(21, -98)` | Exact | South Brood Mother room. | `The Brood Mother` page. |
| 26 | `brood_mother_north` | `(183, 74)` | Exact | North Brood Mother room. | `The Brood Mother` page. |
| 27 | `lord_bob_hard_stop` | `(-30, -133)` | Exact | Hard stop before Lord Bob. | `Lord Doljonijiarnimorinar` page. |
| 28 | `bledrek_hard_stop` | `(-75, 253)` | Exact | Hard stop before Bled/Bledrek. | `Bled` and `Bledrek` pages. |

## Restriction Zones

| Restriction | Boundary | Why it exists |
| --- | --- | --- |
| `pit_drop_no_med` | Do not med at `pit_lip`, the drop, or the first pit rooms. | Slippery ledges and pit traps make this unsafe. |
| `upper_spider_slip_limit` | Treat anything beyond `upper_spider_lip` as driver-only. | Ramp slips can widen pulls unpredictably. |
| `lord_bob_no_pull` | Hard stop at `lord_bob_hard_stop`. | Lord Bob is out of scope for the Frenzy route. |
| `bledrek_no_pull` | Hard stop at `bledrek_hard_stop`. | Bled/Bledrek is a bad unattended golem branch. |
| `upper_dogs_optional_only` | Do not auto-extend through `pit_sixth_junction` without an active driver. | Jelek, Gregendek, Tpos, and Khelkar are higher-risk named pulls. |

## Multibox Route Notes

- Default single-group Frenzy loop:
  `safe_hall_med -> frenzy_handoff -> crystal_eyes -> broodling_corner -> stalker_corner -> upper_spider_lip -> frenzy_handoff -> safe_hall_med`
- Optional upper-spider check:
  `frenzy_handoff -> upper_spider_lip -> crystal_fang_ramp -> upper_spider_lip -> frenzy_handoff`
- Optional pit/upper-dogs extension:
  `safe_hall_med -> pit_lip -> pit_first_room -> pit_second_room -> pit_safe_room -> pit_fourth_room -> pit_fifth_room -> pit_sixth_junction -> jelek_room -> khelkar_dog_room -> marlek_room -> gregendek_room -> tpos_room -> ular_room_west -> pit_sixth_junction -> pit_safe_room -> safe_hall_med`
- Recovery rule: always retrace the branch you entered. Never cut through Lord Bob or Bled/Bledrek on the return.

## Spawn Pattern Notes

- Project 1999's zone spawn-timer page lists Velketor's Labyrinth at `32:50`.
- The Frenzy shelf concentrates named coverage around `Crystal Eyes`,
  `a Frenzied Velium Broodling`, `a Frenzied Velium Stalker`, and `Crystal Fang`.
- `Lord Bob` and `Bled/Bledrek` are hard-stop named branches, not part of the
  Frenzy baseline.
- Upper dogs remain optional, attended-only extensions.
- Additional burn-on-sight branches remain documented for `Marlek Icepaw`,
  `Ular Icepaw`, and `Venar Icepaw`, but they do not promote the upper dogs
  route into the default unattended loop.

## Schema Gap

Issue `#1900` still tracks runtime schema support for:

- named waypoint arrays
- restriction-zone polygons or sectors
- ordered multibox route segments
- source confidence tags

## Validation Status

- This route is research-backed but still needs live proof.
- Use [Velketor's Labyrinth Validation](Velketors-Labyrinth-Validation.md) and
  [velketors-validation-template.csv](assets/velketors-validation-template.csv)
  as the canonical live-proof ledger.
- Treat every waypoint and restriction on this page as a research baseline until
  a live sample confirms route safety, leash behavior, and real spawn cadence.
