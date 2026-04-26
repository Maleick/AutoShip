# Velketor's Labyrinth

Zone spawn-area coordinates and evidence index for the TextQuest zone tracker.
Parent tracker: issue `#1774`. Implements issue `#3364`.

Dependencies satisfied before this page:
- `#1568` — camp config loader baseline
- `#1396` — zone-line coordinate research
- `#1559` — restriction zone documentation
- `#1720` — Frenzy camp route research

---

## Zone Entry and Exit Coordinates

| Point | Coordinates (EQ XYZ) | Confidence | Source |
| --- | --- | --- | --- |
| Great Divide → Velketor's entrance | `(-6770, 3130, 0)` in GD | Exact | Project 1999 Travel Guide |
| Velketor's → Great Divide zone-line | `(581, -65, 0)` in VL | Exact | Project 1999 zone page and Travel Guide |

---

## Spawn-Area Coordinate Summary

All coordinates are in Velketor's Labyrinth local EQ space (Y, X, Z or X, Y, Z as labelled on P99 maps).
"Exact" = verified from Project 1999 named-mob page. "Derived" = interpolated from named anchors and route notes.

### Frenzy Shelf (Primary Farm Area)

| # | Waypoint | Coordinates | Confidence | Purpose |
| --- | --- | --- | --- | --- |
| 1 | `safe_hall_med` | `(360, 40)` | Derived | Camp center / med stack for the Frenzy runtime camp |
| 2 | `frenzy_handoff` | `(405, 60)` | Derived | Pull-point handoff at the Frenzy lip |
| 3 | `crystal_eyes` | `(479, 2)` | Exact | Named spider — lower Frenzy shelf |
| 4 | `broodling_corner` | `(410, 70)` | Exact | `a Frenzied Velium Broodling` spawn |
| 5 | `stalker_corner` | `(485, 75)` | Exact | `a Frenzied Velium Stalker` spawn |
| 6 | `crystal_fang_ramp` | `(440, 154, 88)` | Exact | `Crystal Fang` upper-spider spawn (top of ramp) |
| 7 | `upper_spider_lip` | `(430, 120)` | Derived | Turnaround before steep Crystal Fang ramp |

### Pit Branch

| # | Waypoint | Coordinates | Confidence | Purpose |
| --- | --- | --- | --- | --- |
| 8 | `pit_lip` | `(315, -20)` | Derived | Check point before pit drop — no-med boundary |
| 9 | `pit_first_room` | `(255, -120)` | Derived | First room below the drop |
| 10 | `pit_second_room` | `(220, -170)` | Derived | Straight/right split |
| 11 | `pit_safe_room` | `(180, -230)` | Derived | Safe setup room in the pit branch |
| 12 | `pit_fourth_room` | `(155, -270)` | Derived | Corridor after safe room |
| 13 | `pit_fifth_room` | `(145, -300)` | Derived | Hallway before next junction |
| 14 | `pit_sixth_junction` | `(165, -335)` | Derived | Split toward Jelek or upper dogs |

### Upper Dogs (Attended-Only)

| # | Waypoint | Coordinates | Confidence | Purpose |
| --- | --- | --- | --- | --- |
| 15 | `jelek_room` | `(134, -322)` | Exact | Jelek Icepaw named room |
| 16 | `khelkar_dog_room` | `(227, -348)` | Exact | Khelkar Icepaw — five-dog ramp room |
| 17 | `marlek_room` | `(199, -361)` | Exact | Marlek Icepaw alternate named room |
| 18 | `gregendek_room` | `(212, -390)` | Exact | Gregendek Icepaw — CH-capable cleric named |
| 19 | `tpos_room` | `(190, -413)` | Exact | Tpos Icepaw — shadowknight named |
| 20 | `ular_room_west` | `(121, -220)` | Exact | Ular Icepaw west spawn |
| 21 | `ular_room_east` | `(308, -213)` | Exact | Ular Icepaw east spawn |

### Brood Branch (Hard-Stop Named)

| # | Waypoint | Coordinates | Confidence | Purpose |
| --- | --- | --- | --- | --- |
| 22 | `brood_master_west` | `(-36, 71)` | Exact | The Brood Master west room |
| 23 | `brood_master_east` | `(197, 66)` | Exact | The Brood Master east room |
| 24 | `brood_mother_south` | `(21, -98)` | Exact | The Brood Mother south room |
| 25 | `brood_mother_north` | `(183, 74)` | Exact | The Brood Mother north room |
| 26 | `lord_bob_hard_stop` | `(-30, -133)` | Exact | Hard stop before Lord Doljonijiarnimorinar |
| 27 | `bledrek_hard_stop` | `(-75, 253)` | Exact | Hard stop before Bled / Bledrek |

---

## Spawn Timer and Cadence

- Zone spawn timer: **32:50** (Project 1999 spawn-timer page)
- Frenzy shelf named cluster: `Crystal Eyes`, `a Frenzied Velium Broodling`, `a Frenzied Velium Stalker`, `Crystal Fang`
- All mobs in the zone are blue to a level 60 character; entrance/Frenzy bracket is `57-59`
- Spawn cadence has not been sampled live — see [Velketor's Labyrinth Validation](Velketors-Labyrinth-Validation.md) for exit criteria

---

## Key Zone Hazards

- No levitation — dungeon uses narrow slippery ledges and pit traps that drop players to lower levels
- Multiple see-invis mobs; spiders are rogue-types that can backstab
- Trains move constantly and aggro radius is inconsistent
- Spiders are not druid-charmable
- Lord Bob and Bled/Bledrek are hard-stop branches — never pull them in the default unattended route

---

## Evidence Links

| Evidence | File / Issue |
| --- | --- |
| Runtime camp baseline | `config/camps/velketors_labyrinth_frenzy.toml` |
| Frenzy camp research page | [Velketors-Labyrinth-Frenzy-Camp.md](Velketors-Labyrinth-Frenzy-Camp.md) |
| Validation ledger | [Velketors-Labyrinth-Validation.md](Velketors-Labyrinth-Validation.md) |
| Sampling template | `assets/velketors-validation-template.csv` |
| Camp Runbooks (Frenzy section) | [Camp-Runbooks.md](Camp-Runbooks.md#velketors-labyrinth-frenzy-camp) |
| Zone coordinate research | [Research-EQ-Maps.md](Research-EQ-Maps.md) |
| Camp config parser | `textquest/src/camp/config.rs` |
| Doc linkage test | `tests/test_velketors_labyrinth_camp_docs.py` |
| Loader schema gap | issue `#1900` |
| Zone tracker parent | issue `#1774` |

---

## Coordinate Evidence State

| Claim | State |
| --- | --- |
| Zone entry/exit coordinates | Research-backed (P99 Travel Guide + zone page) |
| Frenzy shelf named spawns | Research-backed (P99 named-mob pages, Exact confidence) |
| Derived corridor nodes | Research-backed (interpolated from named anchors) |
| Live spawn cadence on Frenzy shelf | Needs live proof — see Validation page |
| Unattended route safety | Needs live proof — see Validation page |
