# MQ2 Plugin Parity Matrix

Deep comparison of MacroQuest plugin capabilities vs TextQuest's current implementation. Organized by plugin priority for the WAR/CLR/SHM/MNK/BRD/BER melee-heavy 6-box composition.

Primary sources:

- [MQ2MoveUtils docs](https://docs.macroquest.org/plugins/community-plugins/mq2moveutils/)
- [MQ2Nav wiki](https://github.com/brainiac/MQ2Nav/wiki/Command-Reference)
- [MQ2Twist docs](https://docs.macroquest.org/plugins/community-plugins/mq2twist/)
- [MQ2Medley docs](https://docs.macroquest.org/plugins/community-plugins/mq2medley/)
- [MQ2Cast docs](https://docs.macroquest.org/plugins/community-plugins/mq2cast/)
- [MQ2Map /mapfilter](https://docs.macroquest.org/plugins/core-plugins/map/mapfilter/)
- [MQ2SpawnMaster](https://docs.macroquest.org/plugins/community-plugins/mq2spawnmaster/)

---

## 1. MQ2MoveUtils — Movement & Positioning (HIGHEST PRIORITY)

The core movement plugin for all multibox setups. Melee groups depend on `/stick` positioning and `/makecamp` scatter for every combat cycle.

### 1.1 /stick — Stick-to-Target Engine

| Feature                                     | MQ2 Capability                                                | TextQuest Status                                                                                                 | Gap                                    | Priority | Complexity |
| ------------------------------------------- | ------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | -------------------------------------- | -------- | ---------- |
| `/stick #` absolute distance                | Stick at N EQ units                                           | **Done** — `StickDistance::Absolute(f32)`                                                                   | None                                   | —        | —          |
| `/stick #%` percentage distance             | Stick at N% of default melee range                            | **Done** — `StickDistance::Percent(f32)`                                                                    | None                                   | —        | —          |
| `/stick mod #` distance modifier            | Additive delta to current distance                            | **Done** — `StickEngine::apply_mod()`                                                                       | None                                   | —        | —          |
| `/stick hold` lock target                   | Lock onto target at stick-start time                          | **Done** — `StickConfig.hold`, `locked_id`                                                                  | None                                   | —        | —          |
| `/stick always` auto-resume                 | Keep armed, auto-resume on next NPC                           | **Done** — `StickConfig.always`                                                                             | None                                   | —        | —          |
| `/stick id #` explicit spawn                | Stick to specific spawn ID                                    | **Done** — `StickConfig.id`                                                                                 | None                                   | —        | —          |
| `/stick behind`                             | Position at target's rear arc                                 | **Missing** — `positioning.rs` has `behind_target()` but not integrated into StickEngine                    | StickEngine has no arc/angle modes     | **P0**   | Medium     |
| `/stick behindonce`                         | Move behind initially, then just maintain distance            | **Missing**                                                                                                 | No one-shot behind mode                | P1       | Low        |
| `/stick !front`                             | Stay anywhere except frontal arc                              | **Missing**                                                                                                 | No arc exclusion logic                 | **P0**   | Medium     |
| `/stick pin`                                | Position at target's side (flank)                             | **Missing**                                                                                                 | No flank angle calc                    | **P0**   | Medium     |
| `/stick front`                              | Position in frontal arc                                       | **Missing**                                                                                                 | No front-arc mode                      | P2       | Low        |
| `/stick snaproll [left\|right\|face\|rear]` | Instant reposition: plot point behind mob, run straight there | **Missing**                                                                                                 | No snaproll logic                      | P1       | Medium     |
| `/stick moveback`                           | Back up if target approaches too close                        | **Missing**                                                                                                 | No reverse movement on proximity       | **P0**   | Low        |
| `/stick healer`                             | Stick without heading adjustments (for healers)               | **Missing**                                                                                                 | No heading-suppress mode               | P1       | Low        |
| `/stick loose`                              | Incremental turning (human-like)                              | **Partial** — `MovementPersonality` humanization exists in `nav/humanize.rs` but not wired to stick heading | Heading humanization not in stick path | P1       | Low        |
| `/stick truehead`                           | Actual keypress heading adjustments                           | **Missing**                                                                                                 | Stick uses direct heading writes       | P2       | Medium     |
| `/stick uw` (underwater)                    | Look up/down at target                                        | **Missing**                                                                                                 | No vertical angle adjustment           | P2       | Low        |
| `breakontarget`                             | Break stick if target changes                                 | **Missing**                                                                                                 | No break conditions on stick           | P1       | Low        |
| `breakongate`                               | Break if target gates                                         | **Missing**                                                                                                 | No gate detection                      | P2       | Medium     |
| `breakonwarp`                               | Break if target warps                                         | **Partial** — `WarpMonitor` exists in nav but for player warp detection, not target                         | P1                                     | Low      |
| `breakonaggro`                              | Break if player has aggro                                     | **Missing**                                                                                                 | No aggro-break on stick                | P1       | Low        |
| `breakongm`                                 | Break if GM detected                                          | **Missing**                                                                                                 | No GM detection system                 | P1       | Medium     |
| `pauseonwarp`                               | Pause (don't break) on target warp                            | **Missing**                                                                                                 | No pause-vs-break distinction          | P2       | Low        |
| `randomize`                                 | Randomize strafe direction                                    | **Missing**                                                                                                 | No strafe randomization                | P2       | Low        |
| `delaystrafe`                               | Delay before strafing                                         | **Missing**                                                                                                 | No strafe delay                        | P2       | Low        |
| `useback`                                   | Use backward movement key                                     | **Missing**                                                                                                 | No backward movement option            | P2       | Low        |
| `usefleeing`                                | Adjust for fleeing mobs                                       | **Missing**                                                                                                 | No flee-aware positioning              | P1       | Medium     |
| `strafewalk`                                | Use walk speed for strafing                                   | **Missing**                                                                                                 | No walk-speed strafe                   | P3       | Low        |
| `!frontarc #`                               | Configurable arc (1.1-260.0 degrees) for !front               | **Missing**                                                                                                 | No configurable arcs                   | **P0**   | Low        |
| `behindarc #`                               | Configurable arc (1.1-260.0 degrees) for behind               | **Missing**                                                                                                 | No configurable arcs                   | **P0**   | Low        |
| `mindelay # / maxdelay #`                   | Delay range before strafe movement                            | **Missing**                                                                                                 | No strafe timing                       | P2       | Low        |
| `backupdist #`                              | Distance before moveback engages                              | **Missing**                                                                                                 | No moveback system                     | P1       | Low        |
| `breakdist #`                               | Warp detection distance for break                             | **Partial** — WarpMonitor has distance thresholds                                                           | P2                                     | Low      |
| `snapdist #`                                | Distance past target before snaproll turns                    | **Missing**                                                                                                 | No snaproll                            | P1       | Low        |

**Arc angle calculation (MQ2 reference):**

- Default `behindarc` = 45.0 degrees (half-cone = 22.5 degrees each side of target's rear)
- Default `!frontarc` = 90.0 degrees (so "not front" = everything outside the front 90-degree cone)
- Configurable range: 5.1 to 259.9 degrees
- MQ2 uses `/calcangle` to dump the angular distance for debugging
- EQ heading is 512-unit circle (not 360 degrees); conversion: `eq_heading * 360 / 512`

### 1.2 /makecamp — Camp Positioning

| Feature                               | MQ2 Capability                                               | TextQuest Status                                                                            | Gap                                                        | Priority | Complexity |
| ------------------------------------- | ------------------------------------------------------------ | -------------------------------------------------------------------------------------- | ---------------------------------------------------------- | -------- | ---------- |
| `/makecamp` set at current pos        | Set camp at player's current location                        | **Partial** — `CampSpot` struct exists with position/heading/role, used in `Navigator` | Camp exists but no dynamic set-at-current-pos command path | P1       | Low        |
| `/makecamp on [#]` with radius        | Activate camp with configurable radius                       | **Missing**                                                                            | No camp radius enforcement in Navigator                    | **P0**   | Medium     |
| `/makecamp off`                       | Disable camp                                                 | **Partial** — Navigator has `camp: Option<CampSpot>` that can be cleared               | P1                                                         | Low      |
| `/makecamp loc Y X`                   | Camp at specific coordinates                                 | **Done** — CampSpot has position field                                                 | None                                                       | —        | —          |
| `/makecamp player [name]`             | Dynamic camp following a PC                                  | **Missing**                                                                            | No player-anchored camp                                    | P1       | Medium     |
| `/makecamp leash` / `leash #`         | Leash boundary preventing straying                           | **Partial** — `MAX_CAMP_DRIFT` (100.0) in positioning.rs is hardcoded                  | Not configurable, not in camp state                        | P1       | Low        |
| `/makecamp radius #`                  | Set camp radius                                              | **Missing**                                                                            | Hardcoded values only                                      | P1       | Low        |
| `/makecamp return`                    | Force immediate return                                       | **Missing**                                                                            | No force-return command                                    | P1       | Low        |
| `/makecamp altreturn`                 | Return to previous/alternate camp                            | **Missing**                                                                            | No camp history                                            | P2       | Low        |
| `mindelay # / maxdelay #`             | Auto-return timing (ms, min 125)                             | **Missing**                                                                            | No delayed return; return is immediate on drift detection  | P1       | Low        |
| `returnnoaggro`                       | Only return if not on aggro list                             | **Missing**                                                                            | No aggro-gate on camp return                               | **P0**   | Low        |
| `returnnotlooting`                    | Don't return while looting                                   | **Missing**                                                                            | No loot-gate on camp return                                | P1       | Low        |
| `returnhavetarget`                    | Allow return even if player has a target                     | **Missing**                                                                            | No target-aware return                                     | P2       | Low        |
| `realtimeplayer`                      | Real-time player camp updates                                | **Missing**                                                                            | No real-time camp anchor                                   | P2       | Medium     |
| `scatter`                             | Scatter positions instead of random in radius                | **Missing**                                                                            | No scatter system                                          | **P0**   | Medium     |
| `bearing # / scatdist # / scatsize #` | Scatter geometry: bearing from camp center, distance, radius | **Missing**                                                                            | No scatter geometry                                        | **P0**   | Medium     |

### 1.3 /moveto — Point-to-Point Movement

| Feature               | MQ2 Capability                     | TextQuest Status                                                                      | Gap                    | Priority | Complexity |
| --------------------- | ---------------------------------- | -------------------------------------------------------------------------------- | ---------------------- | -------- | ---------- |
| `/moveto loc Y X [Z]` | Move to coordinates                | **Done** — Navigator accepts waypoints via `WaypointQueue`                       | None                   | —        | —          |
| `/moveto id # \| id`  | Move to spawn ID or current target | **Partial** — no spawn-ID moveto, but nav can target waypoints                   | No spawn-ID shortcut   | P1       | Low        |
| `/moveto off`         | Stop movement                      | **Done** — Navigator.stop()                                                      | None                   | —        | —          |
| `precisey / precisex` | Single-axis arrival precision      | **Missing**                                                                      | Only 2D distance check | P2       | Low        |
| `dist # / mdist #`    | Configurable arrival distance      | **Partial** — `ARRIVAL_DISTANCE` constant exists but not configurable at runtime | P1                     | Low      |
| `breakonaggro`        | Break on aggro                     | **Missing**                                                                      | No aggro-break         | P1       | Low        |
| `breakonhit`          | Break on taking damage             | **Missing**                                                                      | No damage-break        | P1       | Low        |
| `usewalk`             | Use walk speed                     | **Missing**                                                                      | No walk-speed toggle   | P2       | Low        |
| `useback`             | Use backward movement              | **Missing**                                                                      | No backward nav        | P2       | Low        |
| `loose / truehead`    | Heading adjustment modes           | **Partial** — humanization exists but not wired to moveto heading                | P2                     | Low      |

### 1.4 /circle — Circular Pathing

| Feature           | MQ2 Capability                              | TextQuest Status | Gap                | Priority | Complexity |
| ----------------- | ------------------------------------------- | ----------- | ------------------ | -------- | ---------- |
| `/circle on [#]`  | Circle at current location, optional radius | **Missing** | No circle movement | P2       | Medium     |
| `drunken`         | Random turn intervals                       | **Missing** | —                  | P3       | Low        |
| `clockwise / ccw` | Direction control                           | **Missing** | —                  | P2       | Low        |
| `backward`        | Run backwards                               | **Missing** | —                  | P3       | Low        |

### 1.5 MoveUtils TLO (Top-Level Object) State Reporting

| TLO Member               | MQ2 Returns   | TextQuest Equivalent            | Gap                                    |
| ------------------------ | ------------- | -------------------------- | -------------------------------------- |
| `${Stick.Active}`        | bool          | `StickEngine::is_active()` | **Done**                               |
| `${Stick.Distance}`      | float         | `cached_stick_distance`    | **Done**                               |
| `${Stick.StickTarget}`   | SpawnID       | `cached_stick_target_id`   | **Done**                               |
| `${Stick.Behind}`        | bool          | —                          | **Missing** — no arc mode              |
| `${Stick.Pin}`           | bool          | —                          | **Missing** — no pin mode              |
| `${Stick.MoveBehind}`    | bool          | —                          | **Missing**                            |
| `${Stick.MoveBack}`      | bool          | —                          | **Missing**                            |
| `${Stick.Stopped}`       | bool          | InRange result             | **Partial**                            |
| `${MoveUtils.Stuck}`     | bool          | `StuckDetector` exists     | **Done**                               |
| `${MoveUtils.Command}`   | string        | `NavStatus` enum           | **Partial**                            |
| `${MakeCamp.Status}`     | ON/OFF/PAUSED | `camp: Option<CampSpot>`   | **Partial**                            |
| `${MakeCamp.Returning}`  | bool          | —                          | **Missing**                            |
| `${MakeCamp.Scatter}`    | bool          | —                          | **Missing**                            |
| `${MakeCamp.CampRadius}` | float         | —                          | **Missing** (hardcoded)                |
| `${MoveTo.Moving}`       | bool          | `State::Moving`            | **Done**                               |
| `${MoveTo.ArrivalDist}`  | float         | `ARRIVAL_DISTANCE`         | **Partial** (not runtime-configurable) |

---

## 2. MQ2Nav — Navmesh Navigation

### 2.1 Navigation Commands

| Feature                             | MQ2 Capability                        | TextQuest Status                                                             | Gap                                    | Priority | Complexity |
| ----------------------------------- | ------------------------------------- | ----------------------------------------------------------------------- | -------------------------------------- | -------- | ---------- |
| `/nav target`                       | Navigate to current target            | **Done** — Navigator FSM with waypoint queue                            | None                                   | —        | —          |
| `/nav id #`                         | Navigate to spawn by ID               | **Partial** — no direct spawn-ID nav command, but IPC can set waypoints | No convenience command                 | P1       | Low        |
| `/nav loc Y X Z`                    | Navigate to coordinates               | **Done** — waypoint queue                                               | None                                   | —        | —          |
| `/nav spawn <search>`               | Navigate to spawn by search text      | **Missing**                                                             | No spawn search nav                    | P2       | Medium     |
| `/nav door [name\|id] [click]`      | Navigate to door, optionally interact | **Missing**                                                             | No door/object navigation              | P2       | Medium     |
| `/nav item [click]`                 | Navigate to ground item               | **Missing**                                                             | No item navigation                     | P2       | Low        |
| `/nav waypoint <name>`              | Navigate to named waypoint            | **Missing**                                                             | No named waypoint system               | P1       | Medium     |
| `/nav recordwaypoint <name> <desc>` | Save waypoint at current position     | **Missing**                                                             | No waypoint persistence                | P1       | Medium     |
| `/nav pause`                        | Toggle pause                          | **Done** — `State::Paused(PauseReason)`                                 | None                                   | —        | —          |
| `/nav stop`                         | Halt navigation                       | **Done** — Navigator FSM stop                                           | None                                   | —        | —          |
| `/nav reload`                       | Force navmesh reload                  | **Missing**                                                             | No runtime navmesh reload              | P2       | Medium     |
| `/nav ui`                           | Toggle debug overlay                  | **Missing**                                                             | No in-game overlay (TUI has map panel) | P2       | High       |
| `/nav save / load`                  | Persist settings                      | **Missing**                                                             | No nav config persistence              | P2       | Low        |

### 2.2 Navigation State Signals

| Signal     | MQ2 Provides | TextQuest Equivalent                          | Gap                                     |
| ---------- | ------------ | ---------------------------------------- | --------------------------------------- |
| Active     | bool         | `State::Moving` / `State::Sticking` etc. | **Done**                                |
| MeshLoaded | bool         | —                                        | **Missing** — no navmesh state tracking |
| PathExists | bool         | —                                        | **Missing** — no path validation signal |
| PathLength | float        | —                                        | **Missing** — no path length reporting  |
| Velocity   | float        | `StuckDetector` tracks movement rate     | **Partial**                             |
| Paused     | bool         | `State::Paused(_)`                       | **Done**                                |

### 2.3 Zone Transitions

| Feature                           | MQ2 Capability                                 | TextQuest Status                                            | Gap                                                   | Priority | Complexity |
| --------------------------------- | ---------------------------------------------- | ------------------------------------------------------ | ----------------------------------------------------- | -------- | ---------- |
| Cross-zone navigation             | MQ2Nav v5 supports zone boundary paths         | **Partial** — `zone_graph.rs` exists with zone routing | Zone routing exists but not wired to live zone events | P1       | High       |
| Auto-reload on zone change        | MQ2Nav reloads mesh when zone changes detected | **Missing**                                            | No auto-reload on zone                                | P1       | Medium     |
| Waypoint persistence across zones | MQ2Nav stores waypoints per-zone               | **Missing**                                            | No waypoint storage                                   | P1       | Medium     |

---

## 3. MQ2Map — Zone Map Overlay

### 3.1 Map Rendering & Display

| Feature                            | MQ2 Capability                             | TextQuest Status                                                                   | Gap                                               | Priority | Complexity |
| ---------------------------------- | ------------------------------------------ | ----------------------------------------------------------------------------- | ------------------------------------------------- | -------- | ---------- |
| Zone map rendering                 | In-game overlay on EQ map window           | **Done** — TUI `ui/map.rs` renders Brewall maps in terminal                   | Different medium (TUI vs in-game), but functional | —        | —          |
| Brewall map parsing                | Uses Brewall `.txt` map format (L/P lines) | **Done** — `eq/map_parser.rs` parses L lines (MapLine) and P lines (MapPoint) | None                                              | —        | —          |
| Map layers (1/2/3)                 | Multiple layer files per zone              | **Missing** — loads single map file                                           | No multi-layer support                            | P1       | Low        |
| `/mapshow` spawn filter            | Show spawns matching search                | **Missing**                                                                   | No spawn search filter on map                     | P1       | Medium     |
| `/maphide` spawn filter            | Hide spawns matching search                | **Missing**                                                                   | No spawn hide filter                              | P1       | Low        |
| `/mapfilter NPC`                   | Show/hide NPC markers                      | **Partial** — TUI map shows spawn dots                                        | No toggle per spawn type                          | P1       | Low        |
| `/mapfilter PC`                    | Show/hide PC markers                       | **Partial** — shown in TUI                                                    | No toggle                                         | P1       | Low        |
| `/mapfilter Corpse`                | Show/hide corpses                          | **Missing**                                                                   | No corpse markers                                 | P2       | Low        |
| `/mapfilter Named`                 | Show/hide named NPCs                       | **Missing**                                                                   | No named-NPC filter                               | P1       | Low        |
| `/mapfilter Group`                 | Show/hide group members                    | **Partial** — group members shown                                             | No dedicated filter                               | P2       | Low        |
| `/mapfilter Target`                | Show target marker                         | **Partial**                                                                   | Needs dedicated target indicator                  | P1       | Low        |
| `/mapfilter TargetLine`            | Line from player to target                 | **Missing**                                                                   | No target line overlay                            | P1       | Low        |
| `/mapfilter TargetPath`            | Pathfinding path to target                 | **Missing**                                                                   | No path visualization                             | P1       | Medium     |
| `/mapfilter TargetMelee`           | Melee range ring on target                 | **Missing**                                                                   | No range ring                                     | P1       | Low        |
| `/mapfilter CastRadius #`          | Cast range circle around player            | **Missing**                                                                   | No radius overlays                                | P1       | Low        |
| `/mapfilter SpellRadius #`         | Spell range circle                         | **Missing**                                                                   | No radius overlays                                | P1       | Low        |
| `/mapfilter PullRadius #`          | Pull range circle                          | **Missing**                                                                   | No pull radius                                    | P1       | Low        |
| `/mapfilter CampRadius`            | Camp radius circle                         | **Missing**                                                                   | No camp radius on map                             | P1       | Low        |
| `/mapfilter NPCConColor`           | Color NPCs by con level                    | **Missing**                                                                   | No con-color mapping                              | P1       | Low        |
| `/highlight` with color/size/pulse | Highlight specific spawns                  | **Missing**                                                                   | No highlight system                               | P2       | Medium     |
| `/mapnames` label formatting       | Configurable spawn name labels             | **Partial** — TUI shows spawn names                                           | Not configurable                                  | P2       | Low        |
| `/mapclick` for movement           | Click map to navigate                      | **Missing**                                                                   | TUI is keyboard-driven                            | P2       | Medium     |

### 3.2 Map Data Format

MQ2Map and TextQuest both use Brewall's map format:

- **L lines**: `L x1, y1, z1, x2, y2, z2, r, g, b` — line segments (walls, boundaries)
- **P lines**: `P x, y, z, r, g, b, size, label` — labeled points (zone connections, POIs)
- Files named `<zoneshortname>.txt`, with optional `<zoneshortname>_1.txt`, `_2.txt`, `_3.txt` layers

TextQuest's `map_parser.rs` already handles both L and P line types with `MapLine` and `MapPoint` structs.

---

## 4. MQ2Cast — Spell Casting Framework

### 4.1 Casting Commands

| Feature                  | MQ2 Capability                     | TextQuest Status                                                       | Gap                            | Priority | Complexity |
| ------------------------ | ---------------------------------- | ----------------------------------------------------------------- | ------------------------------ | -------- | ---------- |
| `/casting "Spell" gem#`  | Cast spell from specific gem       | **Partial** — `eq::slash_command()` can issue `/cast #`           | No gem-aware casting framework | P1       | Medium     |
| `-targetid\|####`        | Cast on specific target ID         | **Missing**                                                       | No target-switch-and-cast      | **P0**   | Medium     |
| `-kill`                  | Recast until target dies           | **Missing**                                                       | No kill-loop mode              | P1       | Low        |
| `-recast\|#`             | Recast N times                     | **Missing**                                                       | No recast counter              | P1       | Low        |
| `-maxtries\|#`           | Max attempt count                  | **Missing**                                                       | No max-tries guard             | P2       | Low        |
| `-bandolier\|<#>`        | Equip bandolier set before cast    | **Missing**                                                       | No bandolier integration       | P2       | Medium     |
| `-invis`                 | Don't cast if invisible            | **Missing**                                                       | No invis check                 | P2       | Low        |
| `/interrupt`             | Interrupt current cast             | **Partial** — `combat/state.rs` has duck-to-interrupt for clerics | No general interrupt command   | P1       | Low        |
| `/memorize`              | Mem spells to gems                 | **Missing**                                                       | No spell memorization          | P2       | Medium     |
| `/sss / /ssm` spell sets | Save/load spell set configurations | **Missing**                                                       | No spell set management        | P2       | Medium     |

### 4.2 Cast State & Results

| Feature          | MQ2 Returns             | TextQuest Equivalent          | Gap                                        |
| ---------------- | ----------------------- | ------------------------ | ------------------------------------------ |
| `${Cast.Active}` | bool                    | —                        | **Missing** — no cast-active tracking      |
| `${Cast.Effect}` | spell name              | —                        | **Missing**                                |
| `${Cast.Ready}`  | bool                    | `GcdTracker::is_ready()` | **Partial** — GCD only, not full readiness |
| `${Cast.Result}` | 23 result codes         | —                        | **Missing** — no cast result enum          |
| `${Cast.Timing}` | ms remaining            | —                        | **Missing**                                |
| `${Cast.Taken}`  | spell didn't take hold  | —                        | **Missing**                                |
| `${SpellTimer}`  | remaining buff duration | —                        | **Missing** — no buff timer tracking       |

### 4.3 Cast Result Codes (MQ2 reference, needed for TextQuest)

```
CAST_SUCCESS, CAST_FIZZLE, CAST_INTERRUPTED, CAST_RESIST, CAST_IMMUNE,
CAST_NOTARGET, CAST_CANNOTSEE, CAST_OUTOFRANGE, CAST_OUTOFMANA,
CAST_COMPONENTS, CAST_STUNNED, CAST_ABORTED, CAST_CANCELLED,
CAST_COLLAPSE, CAST_DISTRACTED, CAST_INVISIBLE, CAST_NOTREADY,
CAST_OUTDOORS, CAST_PENDING, CAST_RECOVER, CAST_STANDING,
CAST_TAKEHOLD, CAST_UNKNOWN
```

---

## 5. MQ2Twist / MQ2Medley — Bard Song Rotation (CRITICAL for BRD)

### Current TextQuest Approach

TextQuest's `BardStrategy` (`textquest-dll/src/combat/classes/bard.rs`) delegates entirely to EQ's built-in `/melody` command. This is a deliberate simplification — `/melody` handles basic song cycling automatically. However, `/melody` has significant limitations compared to MQ2Twist/MQ2Medley:

### 5.1 MQ2Twist Features

| Feature                | MQ2 Capability                                    | TextQuest Status                           | Gap                                    | Priority | Complexity |
| ---------------------- | ------------------------------------------------- | ------------------------------------- | -------------------------------------- | -------- | ---------- |
| `/twist # # # #`       | Twist up to 10 songs in order                     | **Replaced** — uses `/melody` instead | `/melody` is simpler but less flexible | P1       | —          |
| `/twist once # # #`    | Execute sequence once, revert                     | **Missing**                           | No one-shot twist                      | P1       | Medium     |
| `/twist hold #`        | Focus single song temporarily                     | **Missing**                           | No song hold/priority                  | **P0**   | Low        |
| `/twist delay #`       | Inter-cast delay (1/10 sec, default 33 = 3.3s)    | N/A — `/melody` handles timing        | Not controllable                       | P1       | —          |
| `/twist adjust #`      | Early recast for long songs (ticks before expiry) | N/A                                   | Not controllable                       | P2       | —          |
| Interrupt recovery     | Detects "You miss a note" and re-queues           | **Missing**                           | `/melody` may not recover cleanly      | **P0**   | Medium     |
| Item click integration | Twist items (slots 21-29) between songs           | **Missing**                           | No item-click-in-rotation              | P1       | Medium     |
| `/twist stop / start`  | Pause/resume without clearing queue               | **Partial** — `/melody` toggle        | Less control                           | P1       | Low        |

### 5.2 MQ2Medley Features (Twist successor)

| Feature                  | MQ2 Capability                                    | TextQuest Status | Gap                        | Priority | Complexity |
| ------------------------ | ------------------------------------------------- | ----------- | -------------------------- | -------- | ---------- |
| Conditional songs        | `songif=condition` per song entry                 | **Missing** | No conditional song logic  | P1       | Medium     |
| Priority scheduling      | Songs 1-20 in priority order, skip if buff active | **Missing** | `/melody` is fixed-order   | **P0**   | High       |
| Duration tracking        | Tracks buff duration, recasts when <6s remain     | **Missing** | No buff duration awareness | P1       | High       |
| Queue with `-targetid`   | Queue mez on XTarget, auto-switch and return      | **Missing** | No bard-targeted-mez queue | **P0**   | High       |
| Dynamic medley switching | Switch medley names, preserve durations           | **Missing** | No named medley configs    | P2       | Medium     |
| `/medley delay #`        | Inter-cast delay (1/10 sec, default 3 = 0.3s)     | N/A         | Not controllable           | P2       | —          |

### 5.3 Twist Timing Mechanics (Reference)

**EQ song mechanics:**

- Standard song cast time: 3.0 seconds
- Song duration: 3 ticks (18 seconds) for most songs; some are longer (e.g., resist songs = 10+ ticks)
- MQ2Twist default delay: 33 (3.3 seconds in 1/10s units) — enough for cast + brief buffer
- Minimum twist delay: 30 (3.0 seconds) — the gem refresh timer
- "Long songs" (>3 ticks) get early recast via `adjust` parameter

**Interrupt handling:**

- MQ2Twist detects: "You miss a note, bringing your song to a close!", "You haven't recovered yet...", "Your spell is interrupted."
- On interrupt: resets current song index to re-attempt
- MQ2Medley: more sophisticated — skips to next song if conditions not met

**TextQuest recommendation:** For Classic/Kunark where bard songs are simpler, `/melody` may suffice. For Velious+, a proper twist engine with interrupt recovery and priority scheduling becomes critical (especially for CC-heavy encounters). The `/melody` approach should be the fallback, with a custom twist FSM as the upgrade path.

---

## 6. MQ2SpawnMaster — Named Spawn Tracking

| Feature                    | MQ2 Capability                         | TextQuest Status | Gap                         | Priority | Complexity |
| -------------------------- | -------------------------------------- | ----------- | --------------------------- | -------- | ---------- |
| `/spawnmaster add`         | Add spawn to watch list                | **Missing** | No spawn watch system       | P1       | Medium     |
| Case-sensitive exact match | Match by exact spawn name              | **Missing** | —                           | P1       | Low        |
| Case-insensitive substring | Match by partial name                  | **Missing** | —                           | P1       | Low        |
| Spawn alert command        | Execute arbitrary command on spawn pop | **Missing** | —                           | P1       | Medium     |
| Per-zone configuration     | Different watch lists per zone         | **Missing** | —                           | P1       | Low        |
| Last sighting tracking     | When spawn was last seen               | **Missing** | —                           | P2       | Low        |
| Visual/audio notification  | Beep, popup, speech on spawn           | **Missing** | TUI could show spawn alerts | P1       | Low        |

**TextQuest has a named spawn tracker panel in the TUI map screen** (`ui/map.rs` header: "named tracker panel"), but it's display-only — no alerting, no custom watch lists, no per-zone configuration.

---

## 7. Summary: P0 Gap List (Must-Have for Melee 6-Box)

These are the features without which the melee group comp (WAR/CLR/SHM/MNK/BRD/BER) cannot function effectively:

### Movement & Positioning (MQ2MoveUtils)

1. **Stick arc modes**: `/stick behind`, `/stick !front`, `/stick pin` with configurable `behindarc` and `!frontarc` — MNK and BER need to be behind, WAR needs to face front
2. **Stick moveback**: Back up when target walks into you — prevents stacking on the mob
3. **Makecamp scatter**: Scatter positions with bearing/scatdist/scatsize — 6 characters can't all stand on the same spot
4. **Makecamp radius enforcement**: Configurable camp radius with return logic
5. **Makecamp returnnoaggro**: Don't return to camp while tanking — WAR/MNK will walk away from mobs

### Casting (MQ2Cast)

6. **Cast with targetid**: CLR and SHM need to cast heals on specific group members without manual retargeting

### Bard (MQ2Twist/MQ2Medley)

7. **Song hold/priority**: BRD needs to hold a slow or mez when CC is needed
8. **Interrupt recovery**: BRD songs interrupted in combat need auto-retry
9. **Mez queue with target switch**: BRD is the CC class — needs to mez adds, then return to song rotation

---

## 8. Implementation Roadmap Recommendation

### Phase 1: Stick Arc Modes (P0, est. medium complexity)

Add `StickMode` enum to `StickConfig`: `Behind`, `NotFront`, `Pin`, `Front`, `None`. Implement arc calculation in `StickEngine::tick()` using `positioning.rs::behind_target()` as the foundation. Add configurable `behind_arc` and `not_front_arc` fields.

**Files:** `textquest-common/src/nav.rs` (StickConfig), `textquest-dll/src/nav/stick.rs` (StickEngine), `textquest-dll/src/combat/positioning.rs` (arc math)

### Phase 2: Stick Moveback + Break Conditions (P0-P1)

Add moveback logic when distance drops below minimum threshold. Add `BreakCondition` enum for target change, aggro, GM detection.

**Files:** `textquest-dll/src/nav/stick.rs`, `textquest-common/src/nav.rs`

### Phase 3: Camp Scatter + Radius (P0)

Add `CampConfig` with radius, scatter geometry (bearing/scatdist/scatsize), return delays, and aggro gates. Replace hardcoded `MAX_CAMP_DRIFT` in positioning.rs.

**Files:** `textquest-common/src/nav.rs`, `textquest-dll/src/nav/state.rs`, `textquest-dll/src/combat/positioning.rs`

### Phase 4: Cast Target Switching (P0)

Add cast-with-targetid to the combat framework: save current target, switch, cast, restore. Build on existing `eq::slash_command()` path.

**Files:** `textquest-dll/src/combat/state.rs`, `textquest-dll/src/hooks/casting.rs`

### Phase 5: Bard Twist Engine (P0)

Replace `/melody`-only approach with a proper twist FSM: song queue, interrupt detection, hold/priority, mez queue. Keep `/melody` as fallback for simple rotations.

**Files:** `textquest-dll/src/combat/classes/bard.rs` (new twist FSM), `textquest-common/src/combat.rs`

### Phase 6: Map Enhancements (P1)

Add filter toggles, radius overlays, target line, con colors to TUI map panel. Multi-layer map support.

**Files:** `textquest/src/tui/ui/map.rs`, `textquest/src/eq/map_parser.rs`

### Phase 7: Named Spawn Alerting (P1)

Add spawn watch list with per-zone config, substring matching, TUI alert panel.

**Files:** New module, `textquest/src/tui/ui/map.rs` (alert panel)

### Phase 8: Named Waypoints + Nav Enhancements (P1)

Add waypoint save/recall, spawn-ID navigation, zone-transition handling.

**Files:** `textquest-dll/src/nav/waypoint.rs`, `textquest-common/src/nav.rs`

---

## 9. TLP Content Focus: Classic -> Kunark -> Velious

### Classic (Level 50)

- **Key content:** Nagafen, Vox, Planes (Hate/Fear/Sky)
- **Melee group needs:** Basic stick behind (MNK backstab doesn't exist yet, but positioning still matters for riposte avoidance). Camp scatter for static camps in dungeons (LGuk, SolB, Unrest).
- **Bard needs:** Selo's, Haste, Resist songs. `/melody` may suffice.

### Kunark (Level 60)

- **Key content:** Kunark dragons (Trakanon, Gorenaire, Severilous), Sebilis, Chardok, Veeshan's Peak
- **Melee group needs:** Stick !front critical for monks (Flying Kick positioning). Scatter essential for AE encounters. Cast-with-targetid for CLR chain healing.
- **Bard needs:** AE slow becomes critical. Mez for Seb/Chardok. Twist engine starts to matter.

### Velious (Level 60, raid-heavy)

- **Key content:** NToV, ToV, Kael, Thurgadin
- **Melee group needs:** Full stick suite (behind/!front/pin) for DPS optimization. Break conditions for dragon AEs. Camp scatter for multi-group positioning.
- **Bard needs:** Full twist engine mandatory. Priority scheduling for resist songs + haste + AE slow. Mez queue for add management in ToV.
- **Raid needs:** This is where the 36-box setup matters most. Need cross-group coordination, CH chain support, and MA-assist broadcasting.

---

## 10. RedGuides Public Configs Reference

Common 6-box melee configs from RedGuides community:

- **WAR/CLR/SHM/MNK/BRD/BER** (TextQuest's exact comp) — considered strong for TLP
- WAR sticks front with `hold`, CLR uses `healer` stick on WAR
- SHM stands at 30 range for debuffs/heals, BRD in melee range
- MNK uses `!front` (Flying Kick doesn't require behind, but avoids riposte)
- BER uses `behind` or `!front` for Frenzy positioning
- Camp scatter: WAR at bearing 0, CLR at bearing 180 (behind camp center), SHM at bearing 90 scatdist 30, melee DPS scattered around mob with `!front`

Key KissAssist settings for this comp:

- AssistAt=98 (WAR), AssistAt=96 (DPS)
- DoMelee=TRUE for MNK/BER/BRD, StickMode=!front
- HealPoint for CLR at 70% (main heal), 50% (CH), panic at 30%
- SlowSpell and DebuffSpell for SHM
- MelodyIf for BRD: songs change based on combat/idle state
