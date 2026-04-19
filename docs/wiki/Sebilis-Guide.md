# Sebilis Farming & Validation Guide

Consolidated planning and validation reference for Old Sebilis automation, covering both the Disco 1-2 camps and the underground myconid/jug farming areas. This guide integrates camp definition, farming strategy, and live validation tracking.

---

## Table of Contents

1. [Disco Camp Overview](#disco-camp-overview)
2. [Farming & XP Routing](#farming--xp-routing)
3. [Live Validation & Farming Proof](#live-validation--farming-proof)
4. [Zone & Faction Notes](#zone--faction-notes)

---

## Disco Camp Overview

_Disco camp research and planning phase — content to be added as waypoints, camp geometry, and pull restrictions are documented from external sources and validated through live testing._

### Campaign Setup

Old Sebilis is one of the highest-XP zones available in the Kunark era and can absorb 4-6 groups across its multiple camp locations.

- **Primary camping:** Disco 1 (right wing frogloks, disco dancers)
- **Secondary camping:** Disco 2 (continuation of right wing spawn)
- **Underground alternative:** Juggs and myconids (for plat farming or mixed-group splits)
- **Recommended level range:** 50-60
- **Spawn timer baseline:** Research-backed (pending live confirmation)

---

## Farming & XP Routing

### Right Wing Camps (Disco 1-2)

The right wing of Old Sebilis contains the Disco 1 and Disco 2 camp areas, which are ideal for leveling because:

- Dense spawn rates with good ZEM (zone experience modifier)
- Primarily froglok trash that provides consistent XP
- Multiple camp points to distribute groups and reduce spawn contention
- Relatively safe pull angles with clear boundaries
- Clean faction implications for farming purposes

**Camp Assignments:**

- Groups 1-3 typically occupy Disco 1 and surrounding area
- Groups 4-5 can use Disco 2 or adjacent spawn rooms
- Group 6 (caster utility) can support or farm alternative areas

### Underground Myconids & Jugs

The underground section of Old Sebilis contains myconids and jug-dropping mobs that are valuable for:

- **Plat farming:** Jug drops generate 500-1000pp per hour per group
- **XP alternative:** Myconids provide decent XP while yielding valuable loot
- **Split-group flexibility:** Lower-risk farming when Disco areas are congested or respawn-locked

---

## Live Validation & Farming Proof

Old Sebilis remains research-backed and still needs live proof for:

- **Scars-launch access:** Confirm zone is available and accessible at launch window
- **Spawn cadence:** Confirm the actual respawn timers for Disco 1, Disco 2, and underground mobs
- **Camp overlap:** Verify that 4-6 groups can farm simultaneously without spawn contention
- **Nodding Blue Lily forage rate:** If applicable, confirm forage rates for tradeskill components
- **Automation risk assessment:** Validate waypoint traversal, leash behavior, and patrol patterns before unattended farming

**Validation Checklist:**

- [ ] Zone accessible at launch; no gating or quest requirements
- [ ] Disco 1 spawn timer confirmed via live logs
- [ ] Disco 2 spawn timer confirmed via live logs
- [ ] Underground myconid spawn pattern confirmed
- [ ] Jug drop frequency validated across 4+ group sample
- [ ] Pull leash behavior validated for Disco camps
- [ ] Waypoint navigation tested in actual zone geometry
- [ ] Unattended solo-group farm run for 30+ minutes without wipe
- [ ] Multi-group simultaneous farm run for 60+ minutes without spawn contention or camp conflicts

**Related Documentation:**

- Record all camp waypoints, pull points, restriction boundaries, and spawn confirmations in this guide before treating the zone as a solved overnight farm

---

## Zone & Faction Notes

### Access & Travel

- Old Sebilis is located in Kunark and is accessible via standard Kunark travel routes
- Faction requirements: Standard froglok faction (for safe pull and minimal train risk)
- Recommended pre-farm faction work: Complete enough Kunark quests to avoid aggro from non-combat NPCs

### Mob Composition

- **Right wing:** Primarily frogloks (warriors, priests, shamans) and disco dancers
- **Underground:** Myconids, jugs, and related undead/fungal mobs
- **Named targets:** Trakanon and related named frogloks; confirm randomized loot implications if applicable
- **Hazards:** Large lizards and drolvargs that patrol between main areas; avoid pulling into adjacent zones

### Loot & Economy

- **Tradeskill components:** Nodding Blue Lilies (forageable), leather, and bone drops
- **Vendor trash:** Consistent raw plat from mob loot
- **Named drops:** Variable based on server loot mechanics; on randomized loot servers (e.g., Frostreaver), any same-tier named can potentially drop any same-tier loot
- **Market value:** Early-expansion Sebilis items typically hold value for 2-4 weeks post-zone-unlock

---

## Related Resources

- **Zone Guide:** [P99 Zone Guide - Old Sebilis](P99-Zone-Guide.md)
- **Farming Guide:** [Frostreaver Farming Guide - Old Sebilis Section](Frostreaver-Farming-Guide.md#old-sebilis)
- **Validation Ledger:** This guide serves as the canonical ledger for live testing results
- **Camp Configuration:** `config/camps/sebilis_*.toml` (once defined)

---

## Notes for Operators

- Do not treat this page as proof that unattended Sebilis farming is safe until the live validation checklist above is complete
- Underground farming should be validated separately from Disco camps before treating it as a primary farming route
- Randomized loot server considerations: Adjust target selection and priority based on current server economy and demand
- Consider rotating groups through Sebilis vs. other Kunark zones to maximize overall farm throughput and reduce gear bottlenecks
