# Velketor's Labyrinth Frenzy Camp

Research baseline for issue `#1720`. This page keeps the richer Velketor
waypoint, restriction, route, and travel notes next to the loader-compatible
runtime config in `config/camps/velketors_labyrinth_frenzy.toml`.

## Runtime Camp Config

`textquest/src/camp/config.rs` currently loads only the baseline TOML fields:

- `zone`
- `camp_center`
- `pull_point`
- `pull_radius`
- `camp_radius`
- `leash_radius`
- `rest_mana_pct`
- `pull_mana_pct`
- `level_range`
- `pull_mob_names`
- `ignore_mob_names`
- `burn_mob_names`
- `return_no_aggro`
- `next_camp` / `prev_camp`

The runtime loader does not yet model named waypoints, restriction polygons, or
ordered route segments. Those stay documented here so
`velketors_labyrinth_frenzy.toml` remains loader-safe.

Current runtime baseline:

| Field | Value | Notes |
| --- | --- | --- |
| `camp_center` | `[360.0, 40.0, 0.0]` | Derived "safe hall" med stack beside Frenzy and before the pit drop. |
| `pull_point` | `[405.0, 60.0, 0.0]` | Derived Frenzy handoff that keeps the named spider cluster in line-of-sight. |
| `pull_radius` | `165` | Covers Frenzy plus the upper spider lip without reaching Bled/Bledrek or Lord Bob. |
| `camp_radius` | `30` | Tight enough to keep a 6-box stack out of the slippery ramp lane. |
| `leash_radius` | `110` | Short leash so bad spider pulls reset before the pit or upper-dogs ramps. |
| `rest_mana_pct` | `70` | Hold med until the cleric / support pair is stable after a spider burst. |
| `pull_mana_pct` | `45` | Do not start a new Frenzy pull below this floor. |
| `level_range` | `[57, 60]` | Matches the Project 1999 hunting guidance for the Velketor entrance and Frenzy progression band. |
| `return_no_aggro` | `true` | Prevents the puller from trying to snap back across slippery geometry while still hot. |

## Travel, Level, And Faction Constraints

- Project 1999 lists Velketor's Labyrinth access from Great Divide at
  `-6770, +3130`, and the Travel Guide marks the zone line back out at
  `581, -65`.
- The zone page explicitly says levitation does not work here, the dungeon uses
  narrow slippery ledges, and several pit traps drop players into lower levels.
- The same page warns that trains move constantly, aggro radius is inconsistent,
  and spiders can path off random corners and return with extra mobs.
- The Project 1999 per-level guide and zone page both place the entrance /
  Frenzy progression in the `57-59` bracket; all mobs in the zone are described
  as blue to level 60.
- Named Icepaw kobolds such as `Ular Icepaw`, `Venar Icepaw`, and
  `Khelkar Icepaw` all list `Velketor (-0)` with no opposing faction. Treat the
  Frenzy route as faction-neutral farming rather than a camp that gains a
  useful positive faction foothold.
- Spiders in this zone are rogue-types that can backstab, many mobs can see
  invis, and the zone page specifically says spiders are not druid-charmable.

## Research Waypoint Lattice

The table below mixes exact anchors from Project 1999 zone or named pages with
clearly marked derived corridor nodes. Use it as a research route definition,
not as a claim that the coordinates are already live-safe in TextQuest.

| # | Waypoint | Coordinates | Confidence | Purpose | Source |
| --- | --- | --- | --- | --- | --- |
| 1 | `great_divide_zoneline` | `(581, -65)` | Exact | Return path anchor back to Great Divide. | Project 1999 Travel Guide. |
| 2 | `safe_hall_med` | `(360, 40)` | Derived | Primary med stack for the runtime `camp_center`. | Derived from Frenzy named-spawn cluster and map label `Safe hall`. |
| 3 | `frenzy_handoff` | `(405, 60)` | Derived | Runtime `pull_point`; handoff at the Frenzy lip. | Derived from Crystal Eyes and Frenzied Broodling positions. |
| 4 | `crystal_eyes` | `(479, 2)` | Exact | Named spider on the lower Frenzy shelf. | `Crystal Eyes` page. |
| 5 | `broodling_corner` | `(410, 70)` | Exact | Frenzied Velium Broodling spawn. | `a Frenzied Velium Broodling` page. |
| 6 | `stalker_corner` | `(485, 75)` | Exact | Frenzied Velium Stalker spawn. | `a Frenzied Velium Stalker` page. |
| 7 | `crystal_fang_ramp` | `(440, 154, 88)` | Exact | Upper spider spawn at the top of the slippery ramp. | `Crystal Fang` page. |
| 8 | `upper_spider_lip` | `(430, 120)` | Derived | Turnaround before the steepest part of the Crystal Fang ramp. | Derived from Crystal Fang and Frenzy named positions. |
| 9 | `pit_lip` | `(315, -20)` | Derived | Safe check before dropping into the pit. | Derived from Jelek route notes and map label `The Pit`. |
| 10 | `pit_first_room` | `(255, -120)` | Derived | First room after dropping into the pit. | Derived from Jelek route notes. |
| 11 | `pit_second_room` | `(220, -170)` | Derived | Second room with straight / right exits. | Derived from Jelek route notes. |
| 12 | `pit_safe_room` | `(180, -230)` | Derived | Third room described as a safe setup room. | Jelek directions call this the pit safe room. |
| 13 | `pit_fourth_room` | `(155, -270)` | Derived | Corridor room after the safe room. | Derived from Jelek route notes. |
| 14 | `pit_fifth_room` | `(145, -300)` | Derived | Hallway room before the next junction. | Derived from Jelek route notes. |
| 15 | `pit_sixth_junction` | `(165, -335)` | Derived | Junction that splits left to Khelkar and up to Jelek. | Derived from Jelek route notes plus upper-dogs coordinates. |
| 16 | `jelek_room` | `(134, -322)` | Exact | Left-side upper-dogs room for Jelek. | `Jelek Icepaw` page. |
| 17 | `khelkar_dog_room` | `(227, -348)` | Exact | Ramp-down room with five dogs and Khelkar. | `Khelkar Icepaw` page. |
| 18 | `marlek_room` | `(199, -361)` | Exact | Alternate upper-dogs named room on the same branch. | `Marlek Icepaw` page. |
| 19 | `gregendek_room` | `(212, -390)` | Exact | CH-capable cleric named in upper dogs. | `Gregendek Icepaw` page. |
| 20 | `tpos_room` | `(190, -413)` | Exact | Shadowknight named in upper dogs. | `Tpos Icepaw` page. |
| 21 | `ular_room_west` | `(121, -220)` | Exact | Upper-dogs / BM-side named anchor. | `Ular Icepaw` page. |
| 22 | `ular_room_east` | `(308, -213)` | Exact | Alternate Ular spawn in the same branch. | `Ular Icepaw` page. |
| 23 | `brood_master_west` | `(-36, 71)` | Exact | Brood Master room on the BM side. | `The Brood Master` page. |
| 24 | `brood_master_east` | `(197, 66)` | Exact | Alternate Brood Master spawn in the adjacent room. | `The Brood Master` page. |
| 25 | `brood_mother_south` | `(21, -98)` | Exact | South Brood Mother room. | `The Brood Mother` page. |
| 26 | `brood_mother_north` | `(183, 74)` | Exact | North Brood Mother room. | `The Brood Mother` page. |
| 27 | `lord_bob_hard_stop` | `(-30, -133)` | Exact | Hard-stop boundary before Lord Bob and Icy Guardians. | `Lord Doljonijiarnimorinar` page. |
| 28 | `bledrek_hard_stop` | `(-75, 253)` | Exact | Hard-stop boundary before Bled/Bledrek golem pocket. | `Bled` and `Bledrek` pages. |

## Restriction Zones

| Restriction | Boundary | Why it exists |
| --- | --- | --- |
| `pit_drop_no_med` | Do not use `pit_lip`, `pit_first_room`, or the drop itself as a med point. | The zone page calls out slippery ledges and pit traps; a stalled med stack here is one bump away from lower dogs. |
| `upper_spider_slip_limit` | Treat anything beyond `upper_spider_lip` as driver-only unless the route is actively being watched. | `Crystal Fang` sits on a slippery ramp and the zone page warns spiders can path off corners unpredictably. |
| `lord_bob_no_pull` | Hard-stop at `lord_bob_hard_stop`; never drag mobs through Lord Bob's undead wing. | Lord Bob is level 65, protected by Icy Guardians, and sits behind see-invis undead shades. |
| `bledrek_no_pull` | Hard-stop at `bledrek_hard_stop`; never widen Frenzy pulls into the Bled/Bledrek golem room. | Bledrek summons and immediately respawns into Bled when killed, which is a bad unattended branch. |
| `upper_dogs_optional_only` | Do not auto-extend through `pit_sixth_junction` unless a driver is ready for CH, dispels, and higher named damage. | Jelek, Gregendek, Tpos, and Khelkar all hit harder than Frenzy trash and introduce healer or summon risk. |

## Mana And Med Behavior

- Keep `rest_mana_pct = 70` so the cleric and support healer recover after
  spike spider pulls or a bad pathing bounce.
- Keep `pull_mana_pct = 45` because Frenzy can backload damage when rogue
  spiders land backstabs at the handoff.
- `return_no_aggro = true` is intentional: it stops the puller from trying to
  cut back across slippery geometry while still carrying spider aggro.
- The first knob to revisit after live validation is `pull_radius`; do not
  widen it into the pit, upper dogs, or Lord Bob side without a separate note.

## Multibox Route Notes

- Default single-group Frenzy loop:
  `safe_hall_med -> frenzy_handoff -> crystal_eyes -> broodling_corner -> stalker_corner -> upper_spider_lip -> frenzy_handoff -> safe_hall_med`
- Optional upper-spider check when the lower Frenzy shelf is dry:
  `frenzy_handoff -> upper_spider_lip -> crystal_fang_ramp -> upper_spider_lip -> frenzy_handoff`
- Optional pit / upper-dogs extension only with an active driver:
  `safe_hall_med -> pit_lip -> pit_first_room -> pit_second_room -> pit_safe_room -> pit_fourth_room -> pit_fifth_room -> pit_sixth_junction -> jelek_room -> khelkar_dog_room -> marlek_room -> gregendek_room -> tpos_room -> ular_room_west -> pit_sixth_junction -> pit_safe_room -> safe_hall_med`
- Optional BM scouting path only with crowd control ready:
  `pit_sixth_junction -> brood_master_east -> brood_master_west -> brood_mother_north -> brood_mother_south -> pit_safe_room`
- Recovery always retraces the branch you entered. Do not cut through
  `lord_bob_hard_stop` or `bledrek_hard_stop` when returning to camp.

## Spawn Pattern Notes

- Project 1999's zone spawn-timer page lists Velketor's Labyrinth at `32:50`.
- The zone page explicitly calls the zone "quick spawns," which matches the
  Frenzy / upper-dogs reputation but is still not a substitute for live timing.
- `Keljemor` is explicitly documented at `33 minutes`, which is useful as a
  nearby timing reference even though the castle-side gargoyle is outside this
  camp's pull list.
- `Crystal Guardian` is explicitly documented at `1 Hour`, which reinforces why
  the castle and Lord Bob side should stay out of the Frenzy baseline.
- Frenzy named coverage is concentrated around `Crystal Eyes`,
  `a Frenzied Velium Broodling`, `a Frenzied Velium Stalker`, and
  `Crystal Fang`; upper dogs adds Jelek, Khelkar, Marlek, Gregendek, Tpos, and
  Ular once the pit route is proven.
- Treat the precise Frenzy, pit, and upper-dogs cadence as research-backed
  expectations until a live sample records actual placeholder turnover.

## Schema Gap

This issue still cannot store the richer route data in the runtime TOML because
the camp loader has no fields for:

- named waypoint arrays
- restriction-zone polygons or sector metadata
- ordered multibox route segments or branch priorities
- source citations or confidence tags for provisional coordinates

Those fields are tracked in issue `#1900` instead of being silently added to
`velketors_labyrinth_frenzy.toml`.

## Validation Status

- Repo work can document the route and keep
  `velketors_labyrinth_frenzy.toml` loader-safe.
- Use [Velketor's Labyrinth Validation](Velketors-Labyrinth-Validation.md) and
  [velketors-validation-template.csv](assets/velketors-validation-template.csv)
  as the canonical live-proof ledger once a real 6-box session is available.
- Live `6-box` validation, pull leash proof, and spawn-cadence confirmation are
  still blocked in this session because there are no EverQuest clients or live
  server credentials available in the repository workspace.
- Treat every waypoint and restriction on this page as a research baseline until
  a live validation log confirms the route.
