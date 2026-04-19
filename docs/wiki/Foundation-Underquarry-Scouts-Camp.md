# Foundation Underquarry Scouts Camp

Research baseline for issue `#1722`. This page carries the richer Underfoot
waypoint, pull-sector, restriction, and validation notes for the loader-safe
runtime config in `config/camps/foundation_underquarry_scouts.toml`.

## Current Evidence State

| Claim | Evidence state | Repo basis | Notes |
| --- | --- | --- | --- |
| Map-data lineage for this route is anchored in the checked-in Brewall map pipeline. | Repo-backed | `config/maps/*.txt`, `docs/wiki/Navigation-and-Maps.md`, issue `#1396` | Underfoot waypoint anchors should keep citing the Brewall-style map lineage that `#1396` established for repo map assets rather than copying mutable runtime coordinates. |
| The Foundation northwest shelf below the Underquarry ramp is the most conservative first Underfoot anchor for unattended farming. | Research-backed | Brewall `foundation_1.txt`, Rasper's Foundation overview/tasks | It keeps the active camp next to the `Scouting for Land` farm marker `5`, preserves a short retreat to the Underquarry zoneline, and avoids the cross-zone east/south pathing that players warn about. |
| TextQuest can load a Foundation camp today without schema work. | Repo-backed | `config/camps/foundation_underquarry_scouts.toml`, `textquest/src/camp/config.rs` | The runtime config stays inside the existing center/pull/radius/mana schema. |
| The full waypoint lattice, named pull points, restriction geometry, and route branches are still documentation-only. | Repo-backed blocker | `textquest/src/camp/config.rs`, `docs/wiki/Configuration.md` | Those fields are intentionally deferred to issue `#1900`. |
| Foundation rares roam and often spawn by area rather than one fixed point. | Research-backed | Rasper's Foundation overview | The route must treat named pockets as approximate hazard zones, not exact placeholders. |
| Trash repops in this tier should be planned around an approximate 6-minute cadence. | Research-backed but unverified | Community Underfoot grinding notes, Rasper's scouting route density | Use this only as a planning baseline until live logs confirm the real cadence on the chosen server. |
| 6-box live validation is blocked in this session. | Blocked | This repository checkout only | No live EverQuest client/runtime is available here to prove waypoint traversal, leash behavior, or spawn cadence. |

## Runtime Camp Config

`textquest/src/camp/config.rs` currently loads only:

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

Current runtime baseline:

| Field | Value | Notes |
| --- | --- | --- |
| `camp_center` | `[-1263.0, 637.0, -205.0]` | Active 6-box stack on the northwest scout shelf at `Scouting for Land` marker `5`. |
| `pull_point` | `[-1235.0, 590.0, -205.0]` | Handoff on the east edge of the shelf, away from the quest NPC triangle. |
| `pull_radius` | `380` | Covers the scout shelf, Ragbeard line, and the north quest-hub approach without widening into the central/eastern basin. |
| `camp_radius` | `35` | Tight enough to keep the camp stacked off the ramp lip and away from wander-paths. |
| `leash_radius` | `520` | Long enough to recover to the shelf or Underquarry retreat line, short enough to avoid cross-zone drags. |
| `rest_mana_pct` | `75` | Conservative Underfoot med floor for spike damage and sloppy pathing resets. |
| `pull_mana_pct` | `55` | Do not begin another pull until the healer / crowd-control core is stable. |
| `level_range` | `[84, 85]` | Underfoot keeps the level cap at `85`; treat this camp as geared late-expansion group content only. |
| `return_no_aggro` | `true` | Prevents the puller from trying to snap back toward camp while still hot in bad pathing terrain. |

## Travel, Progression, Survivability, And Faction Constraints

- Treat this as a late-group Underfoot camp for geared `84-85` boxes. It is
  not an AFK stepping-stone zone for undergeared progression groups.
- Preferred travel path is `Brell's Rest -> Underquarry -> The Foundation`.
  The Underquarry zoneline sits directly north of the chosen shelf and is the
  primary wipe-recovery lane.
- This route explicitly avoids the eastern and southern scout markers from
  `Scouting for Land`; player reports on Foundation consistently call out poor
  pathing and fast repops, so the conservative answer is to stay inside the
  northwest shelf plus quest-hub corridor.
- The Foreman / Trullica / Dorillis / Dermott / Darott quest triangle is an
  operationally protected zone. If any box is kill-on-sight to that hub, do
  not treat this camp as unattended-safe.
- Do not use wide AE pulls through the quest shelf. This route depends on
  preserving neutral/friendly hub access and a clean retreat to Underquarry.
- No positive faction gain is assumed here. The camp is chosen for trash
  density, recovery options, and route clarity rather than for faction farming.

## Research Waypoint Lattice

The table below uses TextQuest `x, y, z` ordering. Exact nodes come from the
Brewall `foundation_1.txt` labels or Rasper's Foundation map markers. Derived
nodes are conservative connectors between those anchors.

| ID | Waypoint | Coordinates | Confidence | Purpose | Source |
| --- | --- | --- | --- | --- | --- |
| UF-01 | `underquarry_zoneline` | `(-1167.1, 1094.6, -212.3)` | Exact | Hard retreat and evac line back into Underquarry. | Brewall `foundation_1.txt`. |
| UF-02 | `underquarry_south_lip` | `(-1185.0, 980.0, -210.0)` | Derived | First regroup node south of the zoneline before re-entering the shelf. | Derived from Underquarry line and scout marker `5`. |
| UF-03 | `north_shelf_ramp` | `(-1215.0, 840.0, -208.0)` | Derived | Ramp shoulder between the zoneline and the active shelf. | Derived from Underquarry line and scout marker `5`. |
| UF-04 | `scout_shelf_east` | `(-1215.0, 700.0, -205.0)` | Derived | East entry to the active farm shelf. | Derived from scout marker `5` and quest hub. |
| UF-05 | `scout_shelf_center` | `(-1263.3, 637.0, -204.8)` | Exact | Primary `camp_center`. | Brewall `Scout_Farm` marker `5`. |
| UF-06 | `scout_shelf_west` | `(-1345.0, 640.0, -205.0)` | Derived | West edge of the same shelf before the named plateau. | Derived from scout marker `5` and Ragbeard. |
| UF-07 | `ragbeard_south_bend` | `(-1320.0, 360.0, -205.0)` | Derived | Pull transition between the shelf and quest-hub north lane. | Derived from Ragbeard and Foreman nodes. |
| UF-08 | `ragbeard_line` | `(-1290.7, 258.0, -203.9)` | Exact | Named pull point and west-most default route contact. | Brewall `Ragbeard the Morose`. |
| UF-09 | `quest_hub_north` | `(-1250.0, 320.0, -205.0)` | Derived | North edge of the quest NPC triangle; default regroup lane. | Derived from Foreman / Dermott / Ragbeard nodes. |
| UF-10 | `foreman_gribblebitz` | `(-1230.6, 269.1, -205.3)` | Exact | Protected hub anchor and fallback med shelf. | Brewall `Foreman_Gribblebitz`. |
| UF-11 | `dermott_supplier` | `(-1361.2, 237.1, -203.1)` | Exact | West side of the protected quest hub. | Brewall `Dermott Satllagger`. |
| UF-12 | `trullica_lane` | `(-1216.3, -208.8, -205.3)` | Exact | South quest lane and hard stop for unattended pulls. | Brewall `Trullica`. |
| UF-13 | `dorillis_lane` | `(-1248.0, -165.6, -204.8)` | Exact | South quest lane and no-pull geometry marker. | Brewall `Dorillis`. |
| UF-14 | `fifth_avatar` | `(-1196.7, -106.1, -202.0)` | Exact | Southern travel-only marker that confirms the group stayed on the quest shelf. | Brewall `Fifth_Avatar`. |
| UF-15 | `darott_junction` | `(-1728.7, 93.4, -209.3)` | Exact | Optional west sweep entry and named-risk warning point. | Brewall `Darott`. |
| UF-16 | `saunk_watch` | `(-1854.3, 242.6, -210.0)` | Exact | West watch point for social aggro and named drift. | Brewall `Saunk the Shaman`. |
| UF-17 | `tilda_pocket` | `(-1774.7, 229.6, -205.4)` | Exact | Optional driver-only named pocket. | Brewall `Tilda Grintwisdom`. |
| UF-18 | `trag_pocket` | `(-1803.5, 360.9, -211.4)` | Exact | Optional driver-only named pocket. | Brewall `Trag`. |
| UF-19 | `merl_south` | `(-1801.5, 391.6, -205.6)` | Exact | Optional driver-only northern west-shelf contact. | Brewall `Merl`. |
| UF-20 | `merl_north` | `(-2024.1, 389.8, -209.9)` | Exact | Northern extreme of the west shelf; use as a hard-stop, not a default pull. | Brewall `Merl`. |
| UF-21 | `brass_golem_watch` | `(-1855.6, 160.1, -204.4)` | Exact | Named hazard pocket below the west shelf. | Brewall `a brass golem`. |
| UF-22 | `genati_of_saunk` | `(-2101.0, 322.2, -210.1)` | Exact | Hard-stop on the far west side; do not auto-route beyond this point. | Brewall `Genati_of_Saunk`. |
| UF-23 | `quest_hub_regroup` | `(-1275.0, 150.0, -205.0)` | Derived | Default fallback stack when the north shelf is contaminated. | Derived from Foreman / Dermott / Ragbeard nodes. |
| UF-24 | `south_hub_exit` | `(-1235.0, -80.0, -204.0)` | Derived | Last controlled point before the route enters the southern basin. | Derived from Dorillis / Avatar nodes. |

Recommended 6-box movement order:

1. Enter from `UF-01`, move through `UF-03`, and stage the group on `UF-05`.
2. Run the default pull loop inside `UF-04` -> `UF-11`; do not widen south of
   `UF-12` or west of `UF-15` unless a driver is actively supervising.
3. If pathing breaks or a named drifts into camp, fall back from `UF-05` to
   `UF-10`, then retreat to `UF-01` through `UF-03`.

## Pull Points And Restriction Geometry

Named pull points:

- `UF-04` east shelf handoff
- `UF-08` Ragbeard line
- `UF-09` north quest-hub edge
- `UF-10` Foreman fallback stack

Restriction sectors:

| Restriction | Boundary | Why it exists |
| --- | --- | --- |
| `underquarry_travel_only` | `UF-01` through `UF-03` | This is the retreat line, not a farming lane. Keep it clear for wipes and med recovery. |
| `quest_hub_no_aoe` | `UF-09` through `UF-14` | Protected quest NPC triangle; the route depends on keeping this hub usable and social-free. |
| `west_named_driver_only` | `UF-15` through `UF-22` | Merl, Tilda, Trag, brass golem, and allied pockets roam here. Treat this as an optional driver-only sweep, not baseline unattended farming. |
| `south_basin_no_pull` | Anything south of `UF-12` / `UF-13` | The southern farms are part of the same task set but widen into the basin where Foundation pathing is worst. |
| `east_basin_no_pull` | Any route toward the eastern genari statues or scout markers `6-8` | Those markers are useful for documentation, but they are intentionally out of scope for this primary camp. |

Pull behavior notes:

- Keep the default runtime envelope inside the shelf plus quest-hub north lane
  even though the wiki documents west-side optional nodes.
- If a named roamer or social train crosses `UF-05`, do not attempt to "save"
  the pull by widening the route. Reset to `UF-10` or `UF-01`.
- The current runtime config intentionally leaves `pull_mob_names` empty and
  uses a heavy `ignore_mob_names` list instead. That keeps the loader-safe TOML
  conservative until the trash families and pathing are live-verified.

## Mana And Med Behavior

- `rest_mana_pct = 75` is intentionally high for Underfoot. This shelf should
  only restart after cleric and support mana has materially recovered.
- `pull_mana_pct = 55` keeps the puller from chaining into the west shelf or
  south hub while the group is still stabilizing.
- `return_no_aggro = true` is required because the scout shelf sits beside a
  ramp and multiple social pockets. Returning while hot is how a bad pathing
  branch becomes a wipe.
- Safe med hierarchy:
  1. `UF-05` active shelf
  2. `UF-10` Foreman fallback shelf
  3. `UF-01` Underquarry zoneline

## Multibox Route Notes

Default unattended route:

`UF-05 -> UF-06 -> UF-08 -> UF-09 -> UF-10 -> UF-09 -> UF-07 -> UF-05`

Recovery route:

`UF-05 -> UF-04 -> UF-03 -> UF-02 -> UF-01`

Optional driver-only west sweep:

`UF-05 -> UF-11 -> UF-15 -> UF-16 -> UF-17 -> UF-18 -> UF-19 -> UF-15 -> UF-11 -> UF-05`

Operational notes:

- Stay on one elevation shelf at a time. The camp becomes unstable when the
  puller alternates between the scout shelf, south hub, and west plateau.
- Do not rotate into the other `Scouting for Land` farms as part of the same
  unattended route. If you want southern/eastern farms, file a separate camp.
- Use the Underquarry line for corpse recovery and reset discipline; do not
  improvise a retreat through the center of Foundation.

## Spawn Patterns And Approximate Respawn Cadence

- `Scouting for Land` documents eight farm locations across the zone. This camp
  intentionally uses only farm marker `5` and the adjacent north hub corridor.
- Rasper's Foundation overview explicitly warns that rares in this zone roam
  and spawn in areas rather than fixed points. Treat Ragbeard / Tilda / Trag /
  Merl / brass golem markers as pockets, not guaranteed placeholders.
- Community Underfoot grinding notes describe Foundation as fast-respawn trash
  with an approximate `6-minute` cadence. Use that only as planning guidance
  until a real 6-box log confirms the timing.
- Because named movement is area-based, a dry scout shelf does not prove the
  west plateau is safe; west named pockets can still path inward after the
  baseline shelf has been cleared.

## Schema Gap

Issue `#1900` still tracks the camp-schema work needed to store:

- named waypoint arrays
- restriction polygons or sector metadata
- ordered multibox route branches
- source citations / confidence tags
- spawn windows or cadence metadata

Those fields are intentionally not added to
`config/camps/foundation_underquarry_scouts.toml` in this issue.

## Checked-in Sampling Template

Use [foundation-underquarry-validation-template.csv](assets/foundation-underquarry-validation-template.csv)
for future attended or live 6-box sampling. It is intentionally blank so this
repo does not invent waypoint, leash, mana, or spawn-cadence results ahead of a
real Foundation validation pass.

A 2026-04-18 repository audit found no other checked-in Underfoot operator note
or live-validation ledger for this camp, so this template is the canonical
first evidence surface for `#1722`.

The template captures the remaining acceptance-proof fields:

- waypoint traversal outcome
- pull leash outcome
- mana rest threshold behavior
- return-to-camp recovery outcome
- placeholder / named counts
- approximate respawn cadence
- operator mode and notes

Record all values as observed results only after a real Foundation run.

## Needs Live Proof Before This Issue Can Close

- Confirm the actual travel path `Brell's Rest -> Underquarry -> The Foundation`
  with real travel time, keying requirements, and corpse-recovery notes.
- Confirm that the default six-box route
  `UF-05 -> UF-06 -> UF-08 -> UF-09 -> UF-10 -> UF-09 -> UF-07 -> UF-05`
  can be traversed without widening into the blocked west or south sectors.
- Validate leash behavior from the active pull contacts (`UF-04`, `UF-08`,
  `UF-09`, `UF-10`) before promoting the current `pull_radius` or
  `leash_radius` as unattended-safe.
- Validate the current mana thresholds and med hierarchy in live recovery:
  `rest_mana_pct = 75`, `pull_mana_pct = 55`, then fallback from `UF-05` to
  `UF-10` to `UF-01` when the shelf destabilizes.
- Record placeholder counts, named counts, and mean respawn cadence for the
  scout shelf and adjacent north hub instead of relying on the current
  approximate `6-minute` planning baseline.
- Record whether the run stayed attended, semi-attended, or unattended, and
  treat unattended safety as unresolved until a live run proves the route is
  operator-safe.

## Validation Status

- In-repo config parsing can be validated.
- Wiki and config alignment can be validated.
- 6-box live validation for waypoint traversal, pull leash behavior, mana rest
  thresholds, return-to-camp recovery, and spawn cadence is blocked in this
  session because there is no live EverQuest runtime in the workspace.
- Until that blocker is cleared, this page is a planning contract rather than a
  proof that unattended Foundation farming is already safe.

## Sources

- `config/maps/*.txt` Brewall-style map lineage maintained after issue `#1396`
- Brewall `foundation_1.txt` zone map labels
- Rasper's Repository: The Foundation overview and `Scouting for Land` task map
- Fanra Underfoot overview for the level-85 / no-level-increase context
- Community Underfoot grinding notes describing Foundation's pathing and trash cadence
