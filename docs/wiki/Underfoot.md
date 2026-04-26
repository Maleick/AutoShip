# Underfoot Zone Tracker

Zone spawn-area coordinates and coverage evidence for the Underfoot expansion.
Parent tracking issue: `#1776`.

## Overview

Underfoot is the level-85 no-level-increase expansion. The primary gear-progression
area for TextQuest unattended farming is **The Foundation** zone, specifically the
northwest scout shelf adjacent to the Underquarry zoneline.

Dependencies satisfied by this page:

| Issue | Scope |
| --- | --- |
| `#1396` | Map lineage and Brewall map-asset pipeline |
| `#1559` | Zone-entry integrity and transition validation |
| `#1568` | Rare-spawn alert integration |
| `#1722` | Foundation Underquarry Scouts camp config and waypoint lattice |

## Primary Gear-Progression Area: The Foundation

The Foundation (`foundation`) is the canonical first Underfoot farming zone for a
geared 84–85 6-box. It provides dense trash, proximity to the Underquarry zoneline
for safe recovery, and a documented pull corridor that avoids the worst Foundation
pathing.

Detailed camp runbook: [Foundation-Underquarry-Scouts-Camp.md](Foundation-Underquarry-Scouts-Camp.md)

### Spawn-Area Coordinates

All coordinates are in TextQuest `x, y, z` ordering, sourced from Brewall
`foundation_1.txt` labels and Rasper's Foundation map markers. Confidence tiers
follow the same scheme used in `#1722`.

| Spawn Area | Coordinates (x, y, z) | Confidence | Notes |
| --- | --- | --- | --- |
| Northwest scout shelf (primary camp) | `(-1263.3, 637.0, -204.8)` | Exact | `Scouting for Land` farm marker `5`; primary `camp_center` for unattended farming. |
| Underquarry zoneline | `(-1167.1, 1094.6, -212.3)` | Exact | Hard retreat and evac anchor into Underquarry. |
| Ragbeard the Morose pull line | `(-1290.7, 258.0, -203.9)` | Exact | Default west contact on the pull loop. |
| Foreman Gribblebitz hub | `(-1230.6, 269.1, -205.3)` | Exact | Protected quest NPC hub; fallback med shelf. |
| Dermott Satllagger (west hub) | `(-1361.2, 237.1, -203.1)` | Exact | West boundary of the protected quest hub. |
| Trullica (south hard stop) | `(-1216.3, -208.8, -205.3)` | Exact | South quest lane; no-pull boundary. |
| Dorillis (south hard stop) | `(-1248.0, -165.6, -204.8)` | Exact | South quest lane; no-pull geometry marker. |
| Fifth Avatar (south travel marker) | `(-1196.7, -106.1, -202.0)` | Exact | Confirms group stayed on the quest shelf. |
| Darott junction (west entry) | `(-1728.7, 93.4, -209.3)` | Exact | Optional driver-only west sweep entry; named-risk warning point. |
| Saunk the Shaman (west watch) | `(-1854.3, 242.6, -210.0)` | Exact | West watch point for social aggro and named drift. |
| Tilda Grintwisdom pocket | `(-1774.7, 229.6, -205.4)` | Exact | Optional driver-only named pocket. |
| Trag pocket | `(-1803.5, 360.9, -211.4)` | Exact | Optional driver-only named pocket. |
| Merl south | `(-1801.5, 391.6, -205.6)` | Exact | Optional driver-only west-shelf contact. |
| Merl north (hard stop) | `(-2024.1, 389.8, -209.9)` | Exact | Northern extreme of west shelf; hard stop, not a default pull. |
| Brass golem hazard pocket | `(-1855.6, 160.1, -204.4)` | Exact | Named hazard below the west shelf. |
| Genati of Saunk (far west stop) | `(-2101.0, 322.2, -210.1)` | Exact | Absolute far-west hard stop; do not auto-route beyond this. |

Derived connector waypoints for the default 6-box route:

| Waypoint ID | Label | Coordinates (x, y, z) | Purpose |
| --- | --- | --- | --- |
| UF-02 | `underquarry_south_lip` | `(-1185.0, 980.0, -210.0)` | First regroup node south of zoneline. |
| UF-03 | `north_shelf_ramp` | `(-1215.0, 840.0, -208.0)` | Ramp shoulder between zoneline and active shelf. |
| UF-04 | `scout_shelf_east` | `(-1215.0, 700.0, -205.0)` | East entry to the active farm shelf. |
| UF-06 | `scout_shelf_west` | `(-1345.0, 640.0, -205.0)` | West edge of the shelf before the named plateau. |
| UF-07 | `ragbeard_south_bend` | `(-1320.0, 360.0, -205.0)` | Pull transition between shelf and north quest-hub lane. |
| UF-09 | `quest_hub_north` | `(-1250.0, 320.0, -205.0)` | North edge of quest NPC triangle; default regroup lane. |
| UF-23 | `quest_hub_regroup` | `(-1275.0, 150.0, -205.0)` | Default fallback stack when north shelf is contaminated. |
| UF-24 | `south_hub_exit` | `(-1235.0, -80.0, -204.0)` | Last controlled point before the southern basin. |

### Primary Spawn Zone Bounding Box

The verified unattended farming envelope (northwest scout shelf + north quest-hub corridor):

| Boundary | Value |
| --- | --- |
| Center | `(-1263.3, 637.0, -204.8)` |
| Pull radius | `380` units |
| Camp radius | `35` units |
| Leash radius | `520` units |
| Elevation range | `~-212` to `~-203` |
| Zone level range | `84–85` |

## Coverage Evidence

This page satisfies the zone spawn-area coordinate acceptance criteria for parent `#1776`.
All coordinate evidence originates from:

- `config/maps/foundation_1.txt` — Brewall map pipeline established by `#1396`
- Rasper's Repository Foundation overview and `Scouting for Land` task map markers
- `docs/wiki/Foundation-Underquarry-Scouts-Camp.md` — full camp runbook (`#1722`)
- `config/camps/foundation_underquarry_scouts.toml` — runtime camp config

Coordinate confidence classification (consistent with `#1722`):

| Tier | Meaning |
| --- | --- |
| **Exact** | Directly read from a named Brewall map label or Rasper map marker |
| **Derived** | Conservative connector interpolated from two or more exact anchors |

### What Remains Unverified

6-box live validation for waypoint traversal, pull leash behavior, mana rest
thresholds, return-to-camp recovery, and spawn cadence is blocked until a live
EverQuest runtime is available. See
[Foundation-Underquarry-Scouts-Camp.md § Needs Live Proof](Foundation-Underquarry-Scouts-Camp.md)
for the full validation checklist.

Zone spawn coverage for secondary Underfoot zones (Brell's Rest, Underquarry,
Cooling Chamber, Arthicrex, Kernagir, etc.) is out of scope for this issue and
should be filed as child issues under `#1776`.

## Zone Travel Reference

Preferred entry path: `Brell's Rest → Underquarry → The Foundation`

The Underquarry zoneline at `(-1167.1, 1094.6, -212.3)` is the primary recovery
lane. Do not improvise a retreat through Foundation's center.

## Sources

- `#1396` — Map lineage and Brewall asset pipeline
- `#1559` — Zone-entry integrity hook
- `#1568` — Rare-spawn alert system
- `#1722` — Foundation Underquarry Scouts camp config
- `#1776` — Parent zone tracker (this page links back)
- Brewall `foundation_1.txt` — zone map labels
- Rasper's Repository: Foundation overview and `Scouting for Land` task map
- Fanra Underfoot expansion overview (level-85 / no-level-increase context)
