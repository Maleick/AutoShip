# Dreadlands Primary Camp

Research baseline for issue `#1719`. This page keeps the richer Dreadlands
waypoint, restriction, route, and travel notes next to the loader-compatible
runtime config in `config/camps/dreadlands_primary.toml`.

## Current Evidence State

| Claim | Evidence state | Repo basis | Notes |
| --- | --- | --- | --- |
| Lost Valley and the Ancient Combine Outpost are the safest first Dreadlands anchor for an outdoor camp. | Research-backed | `docs/wiki/P99-Zone-Guide.md`, external Dreadlands references | The outpost and spires are the current safest planning anchor for a 45-51 undead loop. |
| TextQuest can load a Dreadlands camp today without schema work. | Repo-backed | `config/camps/dreadlands_primary.toml`, `textquest/src/camp/config.rs` | The current config stays inside the existing center/pull/radius/mana schema. |
| Dreadlands outdoor respawns should be planned around a `6:40` cycle. | Research-backed | `docs/wiki/P99-Zone-Guide.md` | Treat `6:40` as the shared outdoor baseline until live logs prove otherwise. |
| Giants, drolvargs, and open-zone roamers make the main valley unsafe for unattended pull loops. | Research-backed | Dreadlands route notes in `docs/wiki/Camp-Runbooks.md` | The safe loop should stay inside Lost Valley and away from the exposed valley floor. |
| The documented route is safe for unattended 6-box operation. | Needs live proof | Issue `#1719` | This remains blocked on live route, leash, and roam sample data. |

## Primary Camp Definition

The runtime loader only consumes the baseline TOML fields today, so the richer
operator contract lives here.

| Field | Value | Notes |
| --- | --- | --- |
| `camp_center` | `[900.0, 9000.0, 0.0]` | Ancient Combine Outpost courtyard in Lost Valley. |
| `pull_point` | `[970.0, 9050.0, 0.0]` | First handoff north-east of the courtyard. |
| `pull_radius = 500.0` | Runtime default | Covers the outpost loop without deliberately widening into the valley floor. |
| `camp_radius = 60.0` | Runtime default | Keeps the stack on the safe med footprint. |
| `leash_radius = 700.0` | Runtime default | Long enough for the outpost loop, short enough to drop bad roamer pulls. |
| `rest_mana_pct = 70` | Runtime default | Safe med threshold before another undead pull. |
| `pull_mana_pct = 45` | Runtime default | Conservative start threshold for the next pull. |
| `level_range = [45, 51]` | Runtime default | Matches the Lost Valley progression bracket. |
| `return_no_aggro` | `true` | Do not snap back to camp while the puller is still hot. |

The intended safe med location is the Ancient Combine Outpost itself. If the
yard is hot, fall back toward the wizard spires safe med noted in the route
references south of the spire lane.

## Waypoint Route

The runtime schema still cannot store named waypoint arrays, so the route stays
documented here as a research baseline.

| DL-ID | Waypoint | Coordinates | Purpose |
| --- | --- | --- | --- |
| DL-01 | `outpost_courtyard` | `[900, 9000, 0]` | Primary camp anchor. |
| DL-02 | `courtyard_handoff` | `[970, 9050, 0]` | Primary pull point. |
| DL-03 | `north_wall_watch` | `[1080, 9150, 0]` | Check northern wall roamers. |
| DL-04 | `north_ruins` | `[1180, 9300, 0]` | Outer ruin edge before the open valley. |
| DL-05 | `west_ruins` | `[760, 9190, 0]` | Western loop anchor for plaguebone pulls. |
| DL-06 | `west_hilltoe` | `[620, 9330, 0]` | Do not med here; keep moving through. |
| DL-07 | `south_gate` | `[860, 8820, 0]` | South return gate into the courtyard. |
| DL-08 | `spire_safe_med` | `[2806, 9565, 0]` | Emergency safe med near the wizard spires. |
| DL-09 | `spire_lane_entry` | `[2300, 9300, 0]` | Travel-only checkpoint toward the spires. |
| DL-10 | `lost_valley_east` | `[1300, 9100, 0]` | East edge of the outpost loop. |
| DL-11 | `lost_valley_west` | `[500, 9050, 0]` | West edge of the outpost loop. |
| DL-12 | `skeleton_corner` | `[1110, 9440, 0]` | Typical greater plaguebone turn. |
| DL-13 | `spurbone_corner` | `[820, 9440, 0]` | Typical greater spurbone turn. |
| DL-14 | `champion_lane` | `[1240, 9000, 0]` | Wraithbone champion watch lane. |
| DL-15 | `courtyard_return` | `[930, 8920, 0]` | Preferred no-aggro return. |
| DL-16 | `west_recovery` | `[690, 8900, 0]` | Recovery line if west pulls widen. |
| DL-17 | `east_recovery` | `[1260, 8880, 0]` | Recovery line if east pulls widen. |
| DL-18 | `north_recovery` | `[1010, 9520, 0]` | Recovery line if the northern ruins get hot. |
| DL-19 | `combine_wall_south` | `[930, 8740, 0]` | Southern courtyard edge. |
| DL-20 | `druid_ring_lane` | `[2250, 8200, 0]` | Travel-only waypoint; Gorenaire lane starts beyond here. |
| DL-21 | `fv_tunnel_watch` | `[1600, 8700, 0]` | Hard stop before the broader valley traffic. |
| DL-22 | `giant_lane_watch` | `[1500, 9600, 0]` | Hard stop before giant or widow roamers. |

## Restriction Zones and Pull Controls

| Restriction | Boundary | Why it exists |
| --- | --- | --- |
| `gorenaire_no_pull` | Nothing south or east of `DL-20`. | This is the druid ring and Gorenaire lane. |
| `valley_floor_no_pull` | Do not leave the Lost Valley / outpost loop toward Karnor's, FV, or giant huts. | Open-zone roamers make unattended pulls unsafe. |
| `roamer_reset_only` | If a pull widens outside the courtyard loop, recover via `DL-15` through `DL-18`, then drop aggro. | Protect the safe med footprint from trains. |
| `skeleton_triangle_no_med` | Do not med in the northeast skeleton triangle. | It is a separate route with more exposure and roamer overlap. |

Current pull list comes from `config/camps/dreadlands_primary.toml`:

- `greater plaguebone`
- `greater spurbone`
- `wraithbone champion`

Current hard ignores:

- `Gorenaire`
- `a mountain giant patriarch`
- `a dread widow`
- `rotting skeleton`

## Travel, Faction, and Level Notes

- Target range is `45-51`, matching the checked-in `level_range = [45, 51]`.
- The outpost route is intended as a safe med and outdoor undead loop, not as a
  giant, drolvarg, or dragon route.
- `Gorenaire` is a hard ignore and a routing hazard, not a burn target.
- "safe med" in this context means the courtyard stack first, with the spire
  fallback only for resets or travel staging.
- Roamers from the wider valley remain the main reason this route is still
  blocked on live validation.

## Validation Status

- This route is documented and loader-safe, but it still needs `6-box live validation`.
- Live validation is currently blocked because this workspace has no EverQuest
  clients, server access, or recorded samples.
- Treat the route as a planning contract until a live sample confirms pull
  overlap, leash behavior, safe med stability, and roamer handling.

## Schema and Data Gaps

The camp loader still cannot store:

- named waypoint arrays
- restriction polygons or sectors
- ordered route segments
- source confidence tags

Those richer fields stay here beside `config/camps/dreadlands_primary.toml`
until the runtime schema grows intentionally.
