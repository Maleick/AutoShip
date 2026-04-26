# Pick Threshold Artifact — Mid-to-High-Level Target Zones

**Issue:** #3356 (child of #1763, scaffold #1577)
**Status:** Scaffold populated — live proof required for all threshold measurements
**Last Updated:** 2026-04-26
**Requires:** Live EQ access on Frostreaver TLP

---

## Overview

EverQuest's pick (instanced zone copy) system creates duplicate zone instances when the primary zone reaches a population threshold. This artifact tracks:

- Whether a zone supports picks at all
- The observed population threshold that triggers a new pick
- Reliability at 36-account scale
- XP-rate and automation notes per zone

All entries without live measurement are marked **Needs Live Proof**.

---

## How to Measure a Pick Threshold

1. Enter target zone with a single character. Note instance number (default = 0).
2. Add characters one at a time (or in group increments) and record when the server assigns a new pick instance (instance > 0).
3. Log: characters in zone, pick triggered (Y/N), instance number assigned.
4. Repeat until 3 consistent measurements are obtained.
5. Test 36-account boundary: confirm all 36 accounts can be distributed without forced pick splits.

---

## Level 35–50: Mid-Game Dungeons

| Zone | Era | Pick Support | Observed Threshold | 36-Acct Reliable | XP Notes | Automation Notes | Proof Status |
|------|-----|-------------|-------------------|-------------------|----------|-----------------|-------------|
| City of Mist | Kunark | Unknown | — | — | FAST, ZEM 113-125% | 4+ distinct camps ideal for split groups | **Needs Live Proof** |
| The Hole (entrance) | Classic | Unknown | — | — | VERY FAST, ZEM 171% | Elementals summon; mez critical | **Needs Live Proof** |
| The Hole (deep) | Classic | Unknown | — | — | VERY FAST, ZEM 200% | Castle requires OT hammer for evac | **Needs Live Proof** |
| Kedge Keep | Classic | Unknown | — | — | VERY FAST, ZEM 217% | Enduring Breath req; underwater pathing complex | **Needs Live Proof** |
| Lower Guk (Dead Side) | Classic | Unknown | — | — | MEDIUM, ZEM 107% | FBSS/WGN camp; contested | **Needs Live Proof** |
| Nagafen's Lair / SolB | Classic | Unknown | — | — | MEDIUM, ZEM 100-115% | Efreeti summons; lava duct proc blocks regen | **Needs Live Proof** |
| Permafrost (spiders) | Classic | Unknown | — | — | VERY FAST, ZEM 225% | Best ZEM in Classic for level range | **Needs Live Proof** |
| Eastern Wastes | Velious | Unknown | — | — | MEDIUM, ZEM 100% | Outdoor; no pick on outdoor zones (likely) | **Needs Live Proof** |
| Frontier Mountains | Kunark | Unknown | — | — | MEDIUM, ZEM 100% | Giant fort; 2-group capacity | **Needs Live Proof** |
| Temple of Droga | Kunark | Unknown | — | — | MEDIUM, ZEM 100% | One-way drop post zone-in | **Needs Live Proof** |
| Solusek's Eye / Sol A | Classic | Unknown | — | — | FAST, ZEM 173% | 1-2 group capacity | **Needs Live Proof** |

---

## Level 50–60: Endgame Push

| Zone | Era | Pick Support | Observed Threshold | 36-Acct Reliable | XP Notes | Automation Notes | Proof Status |
|------|-----|-------------|-------------------|-------------------|----------|-----------------|-------------|
| Old Sebilis | Kunark | Unknown | — | — | FAST, ZEM 113% | 5-6 group capacity; key required | **Needs Live Proof** |
| Kael Drakkel | Velious | Unknown | — | — | FAST, ZEM 113% | Arena, armor drops; dire wolves dangerous | **Needs Live Proof** |
| Howling Stones / Charasis | Kunark | Unknown | — | — | FAST, ZEM 113% | Key required; South wing most profitable | **Needs Live Proof** |
| Chardok | Kunark | Unknown | — | — | FAST, ZEM 113% | Entrance ≠ exit; train risk | **Needs Live Proof** |
| Siren's Grotto | Velious | Unknown | — | — | FAST, ZEM 113% | CLR/Torpor required | **Needs Live Proof** |
| Velketor's Labyrinth | Velious | Unknown | — | — | FAST, ZEM 113% | Entrance area kobolds/spiders | **Needs Live Proof** |
| Permafrost (bear pits) | Classic | Unknown | — | — | VERY FAST, ZEM 225% | 56-59 best in slot | **Needs Live Proof** |
| The Hole (undead crypt) | Classic | Unknown | — | — | VERY FAST, ZEM 200% | CLR 55-60 anchor | **Needs Live Proof** |

---

## Borderline / Needs Retest

These zones have ambiguous pick support status or conflicting field reports. Retest until result is unambiguous.

| Zone | Ambiguity | Retest Priority |
|------|-----------|----------------|
| Eastern Wastes | Outdoor zones historically no-pick on classic servers; Frostreaver may differ | HIGH |
| Kael Drakkel | Outdoor + indoor split; which sections pick? | HIGH |
| The Hole | Multiple distinct sub-areas — does each area threshold separately? | MEDIUM |

---

## Known Pick Behavior (General EQ Rules)

- Picks are triggered server-side when zone population exceeds a threshold (typically 30–50 players on modern TLP, lower on classic rules).
- Outdoor zones (Eastern Wastes, Frontier Mountains open areas) historically do NOT support picks on P99/classic rulesets. Frostreaver TLP rules must be verified.
- Dungeon instances (City of Mist, Old Sebilis, Chardok, etc.) typically do support picks.
- At 36 accounts, the 36-account reliability column answers: "Can all 36 land in the same pick?" If threshold is e.g. 30, a 36-box operation will always split picks — which affects follow/assist chain automation.

---

## Data Collection Template (field use)

When measuring live, append rows here:

```
| Zone | Characters In | Pick Triggered | Instance # | Notes | Tester | Date |
|------|--------------|---------------|------------|-------|--------|------|
```

---

## References

- Parent epic: #1763
- Scaffold issue: #1577
- Zone ZEM data source: `docs/wiki/P99-Zone-Guide.md`
- Frostreaver ruleset: Free trade, randomized loot, encounter locking, no truebox, Classic+Kunark+Velious
