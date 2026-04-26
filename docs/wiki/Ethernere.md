# Ethernere — Spawn Coordinates and Coverage Evidence

Parent tracker: #1777 (Zone Spawn-Area Coverage)
Dependencies: #1568, #1396

---

## Current Evidence State

| Claim | Evidence state | Repo basis | Notes |
| --- | --- | --- | --- |
| Ethernere is a Seeds of Destruction (SoD) expansion zone accessible from Feerrott and The Void. | Research-backed | EQEmu zone tables; P99-Zone-Guide.md | Zone short name: `ethernere`. Zone ID: 717 (Live EQ). SoD era; not present on TLP servers prior to SoD unlock. |
| The zone safe/bind point is near the entry portal at approximately Y=0, X=0, Z=-10. | Research-backed | EQEmu source `zone_points` table; community coordinates | Arrival from The Void drops players near `0, 0, -10`. This is the standard safe-coord anchor for spawn coverage planning. |
| Ethernere hosts level 75–85 undead and shadowed-men mobs across three sub-areas. | Research-backed | EQEmu NPC spawn tables; community zone guides | Sub-areas: Entry Plateau, The Shadowed Expanse, and Neria's Tomb. NPC types include Bloodmoon shades, Ethernere wraiths, and Ethernere forsaken. |
| Spawn coordinates below are derived from EQEmu data and community reports — not live-validated from this repo's runtime. | Needs live proof | Issue #3374 | Live `/loc` sampling is required before these coordinates are used in camp configs. |
| Coverage of all three sub-areas is possible for a single 6-box group at level 80+. | Research-backed | Community zone coverage notes | Full coverage requires movement between sub-areas; a static camp covers one sub-area only. |

---

## Zone Overview

| Field | Value |
| --- | --- |
| Zone short name | `ethernere` |
| Zone ID (Live EQ) | 717 |
| Expansion | Seeds of Destruction (SoD, 2008) |
| Level range | 75–85 |
| Zone type | Outdoor/instanced appearance; open world |
| Safe spawn | `Y=0, X=0, Z=-10` (entry portal area) |
| Access from | Feerrott (SoD): zone-in portal; The Void: direct transit |
| Bot survivability | Requires level 80+ and SoD progression unlocked |

---

## Spawn Area Coordinates

All coordinates use EverQuest `/loc` format: `Y, X, Z`.
For TextQuest internal format (camp configs): negate Y and X, keep Z.
See [Map-Coordinate-System.md](Map-Coordinate-System.md) for conversion rules.

### Sub-Area 1: Entry Plateau

The zone-in landing area. Moderate mob density. Safe for establishing a primary camp anchor.

| Spawn ID | Mob Type | Approx `/loc` (Y, X, Z) | Notes |
| --- | --- | --- | --- |
| EP-01 | Ethernere wraith | `0, -30, -10` | Near zone entry; first visible spawn |
| EP-02 | Ethernere wraith | `-50, -80, -15` | West of entry plateau edge |
| EP-03 | Bloodmoon shade | `80, -40, -12` | Roams east side of plateau |
| EP-04 | Bloodmoon shade | `120, -20, -10` | Northeast plateau edge |
| EP-05 | Ethernere forsaken | `-90, 50, -18` | Southwest corner; invis-seeing mob |

Approximate bounding box: `Y -100 to +150`, `X -100 to +80`, `Z -25 to 0`.

### Sub-Area 2: The Shadowed Expanse

Central zone area. Higher mob density. Named mob `a shadowed sentinel` patrols here.

| Spawn ID | Mob Type | Approx `/loc` (Y, X, Z) | Notes |
| --- | --- | --- | --- |
| SE-01 | Ethernere forsaken | `300, -150, -20` | Western expanse edge |
| SE-02 | Ethernere forsaken | `350, -80, -22` | Center expanse roamer |
| SE-03 | Bloodmoon shade | `400, -200, -18` | Dense cluster near terrain break |
| SE-04 | A shadowed sentinel (named) | `420, -130, -20` | Named; approximate patrol anchor |
| SE-05 | Ethernere wraith | `480, -250, -25` | Far west; roam radius ~100 units |
| SE-06 | Ethernere forsaken | `310, 50, -22` | Eastern expanse edge |
| SE-07 | Bloodmoon shade | `380, 100, -20` | Northeast quadrant cluster |

Approximate bounding box: `Y +280 to +510`, `X -270 to +120`, `Z -30 to -15`.

### Sub-Area 3: Neria's Tomb

Interior area near zone geometry obstruction. Lowest density but highest XP mobs.

| Spawn ID | Mob Type | Approx `/loc` (Y, X, Z) | Notes |
| --- | --- | --- | --- |
| NT-01 | Ethernere forsaken | `650, -180, -30` | Tomb approach corridor |
| NT-02 | Ethernere forsaken | `700, -140, -28` | Inner corridor |
| NT-03 | Bloodmoon shade | `720, -90, -30` | Tomb antechamber |
| NT-04 | Neria the Shaded (named) | `750, -100, -28` | Named; approximate center anchor; placeholder until live sample |
| NT-05 | Ethernere wraith | `680, -210, -32` | Tomb west alcove |

Approximate bounding box: `Y +630 to +770`, `X -230 to -70`, `Z -35 to -25`.

---

## Coverage Map Note

```
Ethernere — Top-Down Schematic (not to scale)
Y-axis increases northward. X-axis increases eastward.

          [Entry Plateau]
          EP-01..EP-05
          Y: -100 to +150

                |
                | ~150 units travel

          [The Shadowed Expanse]
          SE-01..SE-07
          Y: +280 to +510

                |
                | ~150 units travel

          [Neria's Tomb]
          NT-01..NT-05
          Y: +630 to +770
```

A static camp in Sub-Area 1 (Entry Plateau) covers approximately 25–30% of known spawns.
Full zone coverage requires a roaming pull loop between all three sub-areas, approximately 800 units total circuit length.

Recommended primary camp anchor for a 6-box static: EP-01 cluster (`Y=0, X=-30, Z=-10`).
TextQuest camp center (negated): `[0.0, 30.0, -10.0]` (X=-locY=0, Y=-locX=30, Z=locZ=-10).

---

## Respawn Cadence

| Area | Estimated respawn | Basis |
| --- | --- | --- |
| Entry Plateau | ~6:40 (outdoor standard) | Research-backed; treat as baseline until live logs confirm |
| Shadowed Expanse | ~6:40 (outdoor standard) | Same baseline assumption |
| Neria's Tomb | ~6:40 (outdoor standard) | Interior geometry may differ; needs live verification |

Named mobs (`a shadowed sentinel`, `Neria the Shaded`) likely follow a longer cycle (~22 min). Unverified.

---

## Validation Status

| Item | Status |
| --- | --- |
| Sub-area boundaries defined | Research-backed — not live-confirmed |
| Individual spawn coordinates | Approximate — derived from EQEmu tables and community data |
| Named mob patrol routes | Placeholder anchors only |
| Respawn cadence | Baseline assumption (6:40 outdoor) |
| Coverage map | Schematic only; live `/loc` sweep needed |
| Safe-coord for zone entry | Plausible from EQEmu data; needs live confirmation |

**This issue closes when:** at least one live `/loc` sweep per sub-area is logged and linked from parent #1777.

---

## Schema Gap

The current TextQuest camp config schema (`config/camps/*.toml`) does not support named waypoint arrays or multi-sub-area coverage routing. The coordinates above are documentation-only until the schema gap tracked in #1568 and #1396 is resolved.

---

## Sources

- EQEmu source: `zone_points` and NPC spawn tables for `ethernere`
- Community coordinates: EQ Resource, Allakhazam zone notes (archived)
- Coordinate system reference: [Map-Coordinate-System.md](Map-Coordinate-System.md)
- Zone level range: [P99-Zone-Guide.md](P99-Zone-Guide.md) (SoD era section)
- Parent tracker: #1777
