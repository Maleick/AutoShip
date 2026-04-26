# Sebilis Disco Camp

Research baseline for issue `#1718`. This page keeps the richer Sebilis
waypoint, restriction, route, and travel notes next to the loader-compatible
runtime config in `config/camps/sebilis_disco.toml`.

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
ordered route segments. Those stay documented here so `sebilis_disco.toml`
remains loader-safe. Follow-up issue `#1900` tracks runtime schema support for
those fields.

Current runtime baseline:

| Field | Value | Notes |
| --- | --- | --- |
| `camp_center` | `[-850.0, 700.0, 0.0]` | Safe med spot in the bartender / armsman hallway. |
| `pull_point` | `[-810.0, 620.0, 0.0]` | Corner handoff for right-wing pulls. |
| `pull_radius` | `380` | Covers bartender, Froggy, armorer, and ABC without reaching Trakanon space. |
| `camp_radius` | `35` | Tight enough to keep the 6-box stack off the roam path. |
| `leash_radius` | `160` | Short leash to drop bad pulls before the bar / ABC rooms collapse. |
| `rest_mana_pct` | `70` | Hold the camp at med until the healers recover. |
| `pull_mana_pct` | `40` | Do not start a new right-wing pull below this floor. |
| `level_range` | `[50, 57]` | Matches the lower Sebilis right-wing bracket from the farming guides. |
| `return_no_aggro` | `true` | Do not auto-snap back to camp while the puller is still hot. |

## Travel, Keying, And Faction Constraints

- Sebilis entry requires a `Trakanon Idol` from the `Key to Sebilis` quest.
- The zone exit is not the same as the entry portal. The P99 zone page calls
  teleporting out the safer default, which matters for unattended or corpse
  recovery assumptions.
- Sampled right-wing named pages all show `Trakanon (-30)` and
  `Legion of Cabilis (+10)` faction movement. Treat right-wing farming as a
  Trakanon-hostile loop until live logs prove a narrower faction profile.
- `docs/wiki/P99-Zone-Guide.md` already treats Sebilis as `53+` content, with
  right-wing `Disco 1+2` called out as the lower-risk entry point before crypt
  and juggernaut money routes.

## Research Waypoint Lattice

The table below mixes exact anchors from the P99 Old Sebilis or named NPC pages
with a few clearly marked derived corridor nodes. Use it as a research route
definition, not as a claim that the coordinates are already live-safe in
TextQuest.

| # | Waypoint | Coordinates | Confidence | Purpose | Source |
| --- | --- | --- | --- | --- | --- |
| 1 | `safe_med_hall` | `(-850, 700, 0)` | Derived | Primary med stack for the runtime `camp_center`. | Derived from bartender, armsman, and chef anchors. |
| 2 | `pull_handoff_corner` | `(-810, 620, 0)` | Derived | Runtime `pull_point`; line-of-sight handoff. | Derived from bartender and armorer anchors. |
| 3 | `armory_door` | `(-692, 537, 0)` | Exact | Armory named room and east edge of the short loop. | `froglok armorer` page. |
| 4 | `bar_room` | `(-742, 658, 0)` | Exact | Bartender room and shortest safe pull. | `froglok bartender` location on Old Sebilis page. |
| 5 | `armsman_bedroom` | `(-896, 647, 0)` | Exact | Named room on the north side of the main loop. | `froglok armsman` page. |
| 6 | `froggy_room` | `(-844, 901, 0)` | Exact | North-most right-wing named room. | `Froggy` page. |
| 7 | `abc_stairs` | `(-1010, 710, 0)` | Derived | Return stairs between the bar loop and ABC. | Derived from chef, repairer, and armsman anchors. |
| 8 | `chef_room` | `(-1120, 768, 0)` | Exact | ABC named room anchor. | `froglok chef` page. |
| 9 | `repairer_room` | `(-1130, 784, 0)` | Exact | Southern ABC named room anchor. | `froglok repairer` page. |
| 10 | `scarab_entry` | `(-576, 280, 0)` | Exact | Necrosis scarab room near the east edge of the right wing. | `a necrosis scarab` page. |
| 11 | `guardian_approach` | `(-888, 133, 0)` | Exact | Sebilite guardian branch checkpoint. | `sebilite guardian` page. |
| 12 | `crypt_split` | `(-1045, 158, 0)` | Exact | First crypt caretaker anchor and branch split. | `crypt caretaker` page. |
| 13 | `crypt_west_cube` | `(-1248, 135, 0)` | Exact | Western crypt caretaker anchor. | `crypt caretaker` page. |
| 14 | `pond_watch` | `(-910, 109, 0)` | Exact | Visual check on the myconid pond branch. | `myconid spore king` page. |
| 15 | `pond_ground_only` | `(-930, 120, 0)` | Derived | Ground-only fighting area away from the pond rock and water. | Derived from myconid pond notes. |
| 16 | `ostiary_corner` | `(-871, -315, 0)` | Exact | Disco-2 fork and lower-stair check. | `froglok ostiary` page. |
| 17 | `disco2_fork` | `(-900, -250, 0)` | Derived | Hallway split before the commander / pickler path. | Derived from ostiary and commander anchors. |
| 18 | `commander_east` | `(-369, -63, 0)` | Exact | Eastern commander wander point. | `froglok commander` page. |
| 19 | `commander_west` | `(-1199, -334, 0)` | Exact | Western commander wander point. | `froglok commander` page. |
| 20 | `pickler_room` | `(-1223, -467, 0)` | Exact | Pickler room anchor and south extension. | `froglok pickler` page. |
| 21 | `hidden_passage` | `(-1160, -600, 0)` | Derived | Hidden passage toward Brogg after Disco. | Derived from pickler and Brogg anchors. |
| 22 | `brogg_library` | `(-1109, -727, 0)` | Exact | Deepest right-wing extension worth documenting for recovery. | `Brogg` page. |
| 23 | `jugg_line_start` | `(-1413, 118, 0)` | Exact | Earliest juggernaut coordinate from the P99 list. | `sebilite juggernaut` page. |
| 24 | `protector_hard_stop` | `(-2085, -624, 0)` | Exact | Absolute no-pull stop near Trakanon space. | `sebilite protector` page. |

## Restriction Zones

| Restriction | Boundary | Why it exists |
| --- | --- | --- |
| `trak_no_pull` | Treat anything at or beyond `jugg_line_start` and especially `protector_hard_stop` as out of scope. | This path turns into juggernaut / Trakanon traffic and is not a single-group disco route. |
| `pond_ground_only` | Do not fight on the rock near the myconid pond and do not fight under the water. | The `myconid spore king` page explicitly calls out that kill location as a server-rule risk. |
| `crypt_soft_stop` | Do not drag `crypt caretaker` pulls past `guardian_approach` unless the driver is actively managing crowd control. | The cube summons, sees invis, and crosses a tighter hallway than the bar loop. |
| `entry_bridge_no_med` | Do not use the entry bridge or the first drop as a med point. | The P99 zone page still describes that approach as a frequent train lane, and the zone exit is not the same as the entry. |

## Mana And Med Behavior

- Keep `rest_mana_pct = 70` so cleric / shaman pairs settle before the next
  right-wing sweep.
- Keep `pull_mana_pct = 40` as the lower pull floor for right-wing trash.
- `return_no_aggro = true` is intentional: it prevents the puller from trying
  to snap back to the med hallway while still carrying a live train.
- If live 6-box testing shows right-wing pulls are stable, the first knob to
  revisit is `pull_radius`; do not widen it into juggernaut or protector space
  without a separate validation note.

## Multibox Route Notes

- Default loop for a single 6-box:
  `safe_med_hall -> pull_handoff_corner -> armory_door -> bar_room -> armsman_bedroom -> froggy_room -> abc_stairs -> chef_room -> repairer_room -> pull_handoff_corner -> safe_med_hall`
- Optional lower extension when the short loop is dry:
  `pull_handoff_corner -> guardian_approach -> crypt_split -> crypt_west_cube -> guardian_approach -> pull_handoff_corner`
- Optional disco-2 sweep only with an active driver and crowd control ready:
  `pull_handoff_corner -> ostiary_corner -> disco2_fork -> commander_east -> commander_west -> pickler_room -> hidden_passage -> brogg_library -> hidden_passage -> pull_handoff_corner`
- Recovery route always reverses the branch you entered. Do not cut across the
  pond, juggernaut hallway, or protector branch when returning to camp.

## Spawn Area Coverage Map

Reverse-engineered from the waypoint lattice above and the P99 Old Sebilis zone
page. Each spawn area is defined by a bounding polygon expressed as min/max
coordinate ranges (XY only; Z is effectively 0 for the right-wing flat floors).
Use these bounds for zone-tracker coverage assertions (parent issue `#1768`).

Evidence source for all areas: derived from named-mob anchor coordinates
listed in the waypoint lattice above, cross-referenced against the P99 Old
Sebilis zone page NPC lists. Confidence column follows the same scale as the
waypoint table: **Exact** = directly from a named-mob page, **Derived** = inferred
from two or more Exact anchors with no conflicting data, **Estimated** =
extrapolated from zone geometry with no anchor NPC on that wall.

| Spawn Area ID | Common Name | X min | X max | Y min | Y max | Waypoints Enclosed | Confidence | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `SEBA-01` | Bar Room cluster | -800 | -680 | 540 | 720 | `bar_room` (-742,658), `armory_door` (-692,537), `pull_handoff_corner` (-810,620) | Derived | Bartender + armorer spawn cluster; right-wing entry. |
| `SEBA-02` | Armsman bedroom | -960 | -820 | 580 | 730 | `armsman_bedroom` (-896,647), `safe_med_hall` (-850,700) | Derived | Armsman named and surrounding right-wing trash. Med stack overlaps north edge. |
| `SEBA-03` | Froggy room | -900 | -780 | 840 | 980 | `froggy_room` (-844,901) | Exact | North-most right-wing named room; single anchor, bounded conservatively. |
| `SEBA-04` | ABC corridor (chef/repairer) | -1200 | -1040 | 690 | 850 | `abc_stairs` (-1010,710), `chef_room` (-1120,768), `repairer_room` (-1130,784) | Derived | Three anchors tightly clustered; treated as one spawn area for coverage. |
| `SEBA-05` | Scarab entry | -640 | -520 | 200 | 370 | `scarab_entry` (-576,280) | Exact | Necrosis scarab room at east edge of right wing; pull-list mobs only. |
| `SEBA-06` | Guardian approach | -960 | -800 | 70 | 220 | `guardian_approach` (-888,133) | Exact | Sebilite guardian branch; optional crypt extension start. |
| `SEBA-07` | Crypt corridor | -1320 | -980 | 70 | 220 | `crypt_split` (-1045,158), `crypt_west_cube` (-1248,135) | Derived | Two anchors; caretaker cube range. Do not extend west past -1320. |
| `SEBA-08` | Myconid pond | -1000 | -840 | 60 | 200 | `pond_watch` (-910,109), `pond_ground_only` (-930,120) | Derived | Ground-only fighting area; pond rock and water are out-of-bounds. |
| `SEBA-09` | Disco-2 / ostiary fork | -980 | -820 | -380 | -190 | `ostiary_corner` (-871,-315), `disco2_fork` (-900,-250) | Derived | Lower-stair fork; soft boundary — active driver required. |
| `SEBA-10` | Commander wander path | -1260 | -300 | -400 | -20 | `commander_east` (-369,-63), `commander_west` (-1199,-334) | Exact | Very wide wander corridor; treat as a single patrol area, not a camp zone. |
| `SEBA-11` | Pickler / Brogg extension | -1280 | -1050 | -800 | -400 | `pickler_room` (-1223,-467), `hidden_passage` (-1160,-600), `brogg_library` (-1109,-727) | Derived | Deepest right-wing extension; active driver only per route notes. |

### Hard-Stop Exclusion Zones

The following coordinate ranges are **outside** all coverage areas and must
not appear in pull-radius or route definitions without a separate validation
pass:

| Exclusion ID | Label | Boundary | Reason |
| --- | --- | --- | --- |
| `SEBE-01` | Juggernaut / Trakanon approach | X < -1380, any Y | `jugg_line_start` (-1413,118) is the absolute entry; anything past it crosses into juggernaut and protector traffic. |
| `SEBE-02` | Protector hard-stop | (-2085,-624) ± 200 | `protector_hard_stop` anchor; 2h45m spawn; ignore-listed in runtime config. |
| `SEBE-03` | Pond rock and water | ~(-940,115) to water edge | `pond_ground_only` restriction; server-rule risk per P99 note. |
| `SEBE-04` | Entry bridge / first drop | Zone entry corridor | Frequent train lane per P99; not a safe med point. |

### Coverage Evidence for Zone Tracker (#1768)

This section summarises what the `sebilis` zone entry in the zone tracker can
assert based on the spawn areas above:

- **Right-wing spawn coverage**: areas `SEBA-01` through `SEBA-04` together
  cover the primary Disco 1+2 short loop from pull-handoff corner through ABC.
  All coordinates are Derived or Exact from P99 named-mob anchor pages.
- **Optional extension coverage**: areas `SEBA-05` through `SEBA-08` cover the
  scarab entry, guardian branch, and crypt corridor. These are flagged as
  optional in the multibox route notes and require active driver management.
- **Disco-2 / lower wing coverage**: areas `SEBA-09` through `SEBA-11` cover
  the ostiary fork, commander wander path, and Brogg extension. The commander
  wander path (`SEBA-10`) spans a very wide XY range and should not be treated
  as a static camp area.
- **Exclusion zones**: `SEBE-01` through `SEBE-04` are explicitly out of scope
  for the current Disco camp definition. Any future issue that expands routing
  into those areas must file a separate sub-issue under `#1768` with live
  evidence before updating this page.

All spawn area bounds in this section are **research-backed** (not live-validated).
Promote them to `verified` status only after a live sampling run records actual
mob positions against these bounding boxes.

## Spawn Pattern Notes

- The Old Sebilis zone page lists a `27:00` zone spawn timer and `23:00` for
  patrols.
- `froglok chef` is explicitly listed at `~30 min`.
- `sebilite guardian` is explicitly listed at `27 minutes`.
- `froglok commander` is a wander PH that moves between Disco 2 and Pickler's
  room, so its practical cadence includes hallway travel time even when the
  underlying spawn cycle is normal.
- `sebilite protector` is `2 hours 45 minutes` and belongs to the hard-stop
  area, not the runtime disco pull list.
- The checked-in `config/named_mobs/sebilis.toml` timers still matter for repo
  lineage, but these route notes remain research inputs until live sampling
  records actual right-wing respawns.

## Validation Status

- Repo work can document the route and keep `sebilis_disco.toml` loader-safe.
- Use [Sebilis Farming Validation](Sebilis-Farming-Validation.md) as the
  canonical ledger for live `6-box validation` proof before treating this camp
  as validated.
- Live `6-box` validation, pull leash proof, and spawn-cadence confirmation are
  still blocked in this session because there are no EverQuest clients or live
  server credentials available in the repository workspace.
- Treat every waypoint and restriction on this page as a research baseline until
  a live validation log confirms the route.
