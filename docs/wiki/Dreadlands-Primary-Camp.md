# Dreadlands Primary Camp

This page is the canonical planning and validation ledger for issue `#1719`
and the source of truth for the first tracked Dreadlands camp definition.

The current camp loader still only accepts the minimal fields in
`config/camps/dreadlands_primary.toml`, so this page carries the richer
waypoint, pull-point, restriction-zone, route, and spawn-pattern notes until
the schema grows intentionally.

## Current Evidence State

| Claim | Evidence state | Repo basis | Notes |
| --- | --- | --- | --- |
| Lost Valley and the Ancient Combine Outpost are the safest first Dreadlands anchor for an outdoor camp. | Research-backed | `docs/wiki/P99-Zone-Guide.md`, Project 1999 Dreadlands page, EQ Atlas Dreadlands page | The Dreadlands guides call out the outpost and spires as a 45-51 undead-only route with a documented safe spot south of the wizard spires. |
| TextQuest can load a Dreadlands camp today without schema work. | Repo-backed | `config/camps/dreadlands_primary.toml`, `textquest/src/camp/config.rs` | The current config stays within the existing center/pull/radius/mana schema. |
| The route, waypoints, and restriction geometry are still documentation-only. | Repo-backed blocker | `textquest/src/camp/config.rs`, `docs/wiki/Configuration.md` | There are no TOML keys yet for `waypoints`, named pull points, or restriction zones. |
| Dreadlands outdoor respawns should be planned around a `6:40` cycle. | Research-backed | Project 1999 Zone Spawn Timers | This is the zone-wide outdoor baseline; named or placeholder exceptions still need live confirmation. |
| Giants, drolvargs, and open-zone roamers make the main valley unsafe for unattended pull loops. | Research-backed | Project 1999 Dreadlands page, EQ Atlas Dreadlands page | Both references warn that giants and drolvargs see invis and that disoriented or trained mobs occasionally enter Lost Valley. |
| 6-box live validation is still blocked in this session. | Blocked | This repository checkout only | No live EverQuest client/runtime is available here to prove waypoint traversal, leash behavior, or spawn cadence. |

## Primary Camp Definition

- Camp file: `config/camps/dreadlands_primary.toml`
- Primary anchor: Ancient Combine Outpost courtyard in Lost Valley
- Camp center: `camp_center = [900, 9000, 0]`
- Pull handoff: `pull_point = [970, 9050, 0]`
- Pull controls: `pull_radius = 500`, `camp_radius = 60`, `leash_radius = 700`
- Mana controls: `rest_mana_pct = 70`, `pull_mana_pct = 45`
- Recommended level band: `level_range = [45, 51]`
- Primary targets: `greater plaguebone`, `greater spurbone`, `wraithbone champion`
- Hard ignores: `Gorenaire`, `a mountain giant patriarch`, `a dread widow`, `rotting skeleton`
- Burn target: `wraithbone champion`
- Return posture: `return_no_aggro = true`

Why this anchor:

- The Ancient Combine Outpost / spires route is already called out as a
  45-51 undead-only hunting spot.
- Skeletons do not flee, can be single-pulled, and do not carry faction hits.
- Lost Valley gives a conservative fallback to the wizard spires safe med spot
  instead of forcing the group to med in the open main valley near Karnor's
  Castle, the drachnid forest, or the giant huts.

## Waypoint Route

Waypoint coordinates below use TextQuest `x, y, z` ordering and remain planning
inputs until a live 6-box run confirms the exact geometry and `z` values.

| DL-ID | Purpose | XYZ | Notes |
| --- | --- | --- | --- |
| DL-01 | Safe med perch south of wizard spires | `[2806, 9565, 0]` | Repo-safe fallback when roamers bleed into Lost Valley. |
| DL-02 | Wizard spires center | `[3050, 9650, 0]` | Port-in landmark and emergency regroup reference. |
| DL-03 | Wizard spires south lip | `[2925, 9480, 0]` | First waypoint on the return-to-camp recovery line. |
| DL-04 | Wizard spires west shoulder | `[2625, 9480, 0]` | Keeps the group off the east lip before turning into Lost Valley. |
| DL-05 | Lost Valley east mouth | `[2200, 9300, 0]` | Transition point from safe med perch into the outpost route. |
| DL-06 | Lost Valley east approach | `[1800, 9120, 0]` | Do not hold here; use as a movement-only checkpoint. |
| DL-07 | Lost Valley south approach | `[1500, 8850, 0]` | Begin cautious pull posture here; occasional trains cross the mouth. |
| DL-08 | Outpost east wall outside | `[1280, 9040, 0]` | Outer staging point before entering the courtyard loop. |
| DL-09 | Outpost southeast corner | `[1180, 8740, 0]` | South pull turn-in; use for caster stack realignment. |
| DL-10 | Outpost south gate | `[980, 8660, 0]` | Safe med line if the courtyard is clear. |
| DL-11 | Outpost southwest corner | `[720, 8720, 0]` | Preferred healer-safe edge when the north wall is busy. |
| DL-12 | Outpost west wall outside | `[575, 9360, 0]` | Known skeleton pull point from the per-level hunting guide. |
| DL-13 | Outpost northwest corner | `[650, 9580, 0]` | West recovery turn when returning from spires. |
| DL-14 | Outpost north wall | `[820, 9520, 0]` | Conservative north-most hold before the haunt loop. |
| DL-15 | Outpost courtyard west | `[800, 8600, 0]` | Known outpost skeleton point; lower-risk pull for weaker groups. |
| DL-16 | Outpost courtyard center | `[900, 9000, 0]` | Primary camp center. |
| DL-17 | Outpost courtyard north | `[970, 9050, 0]` | Fixed plaguebone reference from the hunting guide. |
| DL-18 | Outpost courtyard east | `[1100, 9190, 0]` | Primary handoff pull point for single skeleton tags. |
| DL-19 | Outpost east fallback | `[1320, 9200, 0]` | Regroup point if the courtyard east side gets crowded. |
| DL-20 | Druid ring south path | `[2250, 8200, 0]` | Travel-only waypoint; do not idle because Gorenaire can cross this lane. |
| DL-21 | Druid ring center | `[2500, 7725, 0]` | Port landmark and hard no-pull zone. |
| DL-22 | Druid ring north watch | `[2350, 7900, 0]` | Visual check for dragon / giant movement before returning north. |

Recommended 6-box traversal order:

1. Stage and recover at `DL-01` if the zone is hot after port-in or corpse
   recovery.
2. Move through `DL-03` -> `DL-08` and establish the group at `DL-16`.
3. Pull only within the outpost loop `DL-09` through `DL-19`.
4. Use `DL-12`, `DL-15`, `DL-17`, and `DL-18` as the named pull points for the
   four major skeleton angles.
5. If a roamer or train contaminates the loop, retreat by reversing the route
   to `DL-01`; do not cut through the druid ring or main valley to save time.

## Restriction Zones and Pull Controls

Named pull points:

- `DL-12` west wall skeleton line
- `DL-15` southwest courtyard line
- `DL-17` fixed plaguebone anchor
- `DL-18` east wall handoff

Restriction sectors:

- **No-pull: druid ring / Gorenaire lane** — anything south or east of
  `DL-20` is out of bounds. The druid ring is a known Gorenaire landmark and
  should stay travel-only.
- **No-pull: Lost Valley mouth** — `DL-05` through `DL-07` are movement
  checkpoints only. Giants, yetis, and trained roamers can leak in from the
  main zone here.
- **No-pull: main Dreadlands valley** — do not path toward Karnor's Castle,
  the FV tunnel, the giant huts, or the drachnid woods while this camp is
  active.
- **No-pull: rotting skeleton triangle** — the northeast Dreadlands skeleton
  camp is a separate named/medallion route and should not be mixed into this
  camp's leash geometry.

Pull behavior notes:

- Keep the actual pull envelope inside the outpost ruins even though the safe
  med fallback is further east at the spires.
- `pull_radius = 500` and `leash_radius = 700` are intentionally conservative
  so a single bad tag does not drag the puller into the main valley.
- `return_no_aggro = true` is required because open-zone roamers can cross the
  approach line while the group is still resetting.
- A `wraithbone champion` should be burned immediately, but only if it spawns
  inside the outpost loop. If it appears near the spires instead, treat it as a
  travel hazard and reset.

## Travel, Faction, and Level Notes

- Recommended level range is `45-51`. This matches the existing research call
  for the Ancient Combine Outpost / spires undead route.
- Skeleton targets here are undead-only and do not carry faction hits.
- Giants, drolvargs, and many main-zone roamers see invis. Do not treat invis
  as a safe travel guarantee into the camp.
- Wizard and druid ports into Lost Valley are the preferred access path.
- The Firiona Vie tunnel is a worse approach for evil characters because the FV
  side includes hostile guards before the pass into Dreadlands.
- Karnor's Castle is adjacent, but exterior drolvargs and castle traffic make
  it a separate route. This camp should not share leash space with Karnor pulls.

## Spawn Timers and Patterns

- Project 1999's outdoor Dreadlands baseline is `6:40`.
- The Ancient Combine Outpost skeletons should be treated as a shared `6:40`
  static cycle until live logs prove otherwise.
- The fixed plaguebone at `DL-17` is the lowest-risk first pull.
- `wraithbone champion` is the priority named interference case inside this
  route. The wiki notes that it can also appear around the giant wizard spires,
  so named sightings are not confined to the courtyard.
- Occasional trained or disoriented giants and yetis can enter Lost Valley even
  when the local static cycle is clear. Those are roamer events, not proof that
  the ruin loop itself has repopped.

## Validation Status

- Config loading can be validated in-repo.
- Documentation alignment can be validated in-repo.
- 6-box live validation for waypoint traversal, pull leash behavior, mana rest
  thresholds, return-to-camp recovery, and real spawn cadence is currently
  blocked because this session has no live EverQuest client/runtime.
- Until that blocker is cleared, this page should be treated as a planning
  contract rather than as proof that unattended Dreadlands farming is safe.

## Schema and Data Gaps

- `textquest/src/camp/config.rs` has no fields for waypoint lists, named pull
  points, restriction polygons, route ordering, or spawn windows.
- The repo currently has no checked-in `config/maps/dreadlands*.txt` source to
  anchor this route to a TextQuest map-data lineage file.
- `z` values for the route are placeholders and must be confirmed in a live
  6-box pass before any automation path consumes them as authoritative.
- If this camp needs machine-readable waypoints or restriction geometry, open a
  separate schema issue instead of silently extending the TOML format here.

## Sources

- Project 1999 Dreadlands zone page
- Project 1999 Per-Level Hunting Guide (`45-51 Dreadlands Ancient Combine Outpost/Spires`)
- Project 1999 Zone Spawn Timers
- EQ Atlas Dreadlands and Lost Valley maps
