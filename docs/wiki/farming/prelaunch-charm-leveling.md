# Prelaunch: Charm-Based Leveling Group Testing

**Parent Issue:** #1575
**Status:** Documentation filed — live test server validation pending
**Last Updated:** 2026-04-26

---

## Overview

Charm-based leveling is a high-XP-rate strategy in EverQuest TLP servers that leverages an Enchanter's ability to charm powerful mobs and use them as temporary pets against other targets. This document captures group composition, expected mechanics, known risk factors, and acceptance criteria for pre-Frostreaver-launch validation.

---

## Group Composition

### Core Setup

| Slot | Class | Role |
|------|-------|------|
| 1 | Enchanter | Charm, CC, Haste, Mana regen |
| 2 | Cleric | Healing, CH chain anchor |
| 3 | Tank | Off-tank / safety if charm breaks |
| 4–6 | DPS (optional) | Passive assist, loot |

**Minimum viable group:** Enchanter + Cleric only (duo). All other slots are efficiency multipliers, not requirements.

### Charmed Mob Selection Criteria

- Target mobs 2–5 levels above the Enchanter's charm cap when possible (higher XP bonus)
- Prefer mobs with melee-only attacks (avoid casters with AoE or self-buff mobs that resist charm on refresh)
- Target zones: areas with clustered singles or controlled pulls (e.g., Mistmoore, HHK, Velketor's Labyrinth depending on era)

---

## Charm Mechanics Reference

### Charm Break Rate

- Baseline charm duration: 18–36 seconds (varies by mob level delta and Enchanter resist modifiers)
- Break rate increases significantly when:
  - Charmed mob takes damage from group
  - Mob level exceeds Enchanter level by more than 3
  - Enchanter is interrupted during re-charm cast
- Enchanter should maintain **Tash** and **Slow** on charmed mob at all times to reduce break risk and mitigate break damage

### Auto Re-Charm Behavior (TextQuest Implementation)

- The DLL agent monitors `charm_break` game event
- On break: Enchanter macro triggers `mez_target` → wait 1 game tick (6s) → re-cast charm
- Fallback: if mez resists, off-tank taunts; Cleric heals; Enchanter retries charm on next tick
- Auto re-charm fires within 1–2 frames of break detection (sub-100ms latency target)

**Known gap (test required):** confirm DLL charm_break event fires reliably when charm breaks mid-combat vs. when mob is at rest.

---

## XP Rate Benchmarks

### Baseline Targets (to be validated on test server)

| Configuration | Expected XP/hr | Notes |
|---------------|----------------|-------|
| Standard 6-person group (same level mobs) | ~15% level/hr | Baseline reference |
| Charm duo (Enc + Clr, +3 level mobs) | ~25–35% level/hr | Estimated; needs live measurement |
| Charm 6-person (Enc + Clr + 4 DPS) | ~40–55% level/hr | Theoretical upper bound |

**Test protocol:**
1. Log into test server with Enchanter + Cleric
2. Identify target zone and mob population
3. Run 60-minute session with XP logging enabled (`/log on`)
4. Record starting and ending XP %
5. Note charm break frequency per hour
6. Compare against baseline standard-group XP run in same zone

---

## Acceptance Criteria

- [ ] Group composition documented (this document)
- [ ] Charm break rate measured on test server (target: break < once per 45s average)
- [ ] Auto re-charm behavior confirmed: DLL re-charms within 2 ticks of break
- [ ] XP rate per hour recorded for charm duo and charm full group configurations
- [ ] Results appended to this document under **Test Results** section below
- [ ] Issue linked as native sub-issue under #1575

---

## Test Results

*This section to be filled in after live test server validation.*

| Date | Tester | Zone | Group Config | XP/hr | Charm Breaks/hr | Notes |
|------|--------|------|--------------|-------|-----------------|-------|
| TBD | — | — | — | — | — | Pending Frostreaver test server access |

---

## Risk Factors and Mitigations

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Charm break kills Cleric | Medium | Keep Cleric at max range; off-tank positioned between Enc and charmed mob |
| Enchanter OOM from repeated re-charms | High | Bard mana song or KEI; schedule mana breaks every 10–15 min |
| Zone congestion disrupts charm mob supply | Low | Pre-scout zone population windows; document backup zones |
| DLL charm_break event fires late | Unknown | **Test required** — see auto re-charm notes above |

---

## References

- Parent issue: #1575 (Frostreaver prelaunch validation)
- Camp Runbooks: `docs/wiki/Camp-Runbooks.md`
- Class Combat Rotations: `docs/wiki/Class-Combat-Rotations.md`
- Combat Loop: `docs/wiki/Combat-and-Camp-Loop.md`
