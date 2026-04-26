# Estate of Unrest — Farming Validation Notes

> **Status:** Validation notes captured from Frostreaver TLP context. Live testing required to confirm multibox pull-sequence timing and DPS floor numbers on Scars-era servers.
> **Parent:** #1778 — Frostreaver Farming & XP Strategy

---

## Zone Overview

| Property         | Value                              |
| ---------------- | ---------------------------------- |
| Zone ID          | `unrest`                           |
| Era              | Classic (available at launch)      |
| ZEM              | 173%                               |
| Level Range      | 10–34 (farming validation: 10–30)  |
| Multi-Group Cap  | 3 groups (yard + keep + basement)  |
| Primary Pull     | Shadowknight FD-pull or Monk sneak |
| Loot quality     | Moderate plat; strong XP economy   |

---

## DPS Expectations

These are practical floor estimates for a standard 6-box group (WAR/CLR/BRD/SHM/MNK/MNK) at Velious launch with era-appropriate gear.

| Sub-Zone         | Level Range | DPS Floor (group) | Notes                                                                  |
| ---------------- | ----------- | ----------------- | ---------------------------------------------------------------------- |
| Yard / Courtyard | 10–15       | ~300 DPS          | Mixed humanoid + undead; low resist mobs; monks shine here             |
| Inside Keep      | 15–20       | ~450 DPS          | Tighter corridors reduce pull count; undead skeletons and zombies      |
| Basement         | 20–30       | ~550 DPS          | Festering Hags, undead knights; highest density; bard pull control key |

> **Note:** These are unvalidated estimates derived from class composition and ZEM. Live testing should record actual kills-per-hour and compare against the XP-per-hour target needed to hit level 20 within the first 12 hours of launch.

---

## Loot-Per-Kill Observations

### Consistent Vendor Trash (plat baseline)

- Bone Chips — vendor ~1–3pp stacked, high drop rate from skeletons
- Cured Silk / Patchwork gear — low vendor value but high volume
- Light Stone / Peridot — scribe/sell; moderate drop rate from casters

### Named / Notable Drops

| Named Mob             | Sub-Zone  | Drops                                    | Approx Spawn Timer  |
| --------------------- | --------- | ---------------------------------------- | ------------------- |
| Garanel Rucksif       | Basement  | Shining Metallic Robe (SMR)              | ~6 hr placeholder   |
| a festering hag       | Basement  | Netted Kelp, Bloodstained Mantle (rare)  | Roamer / rare pop   |
| Cauldronbubble        | Keep      | Minor loot; kill for progression         | ~30 min             |
| Jack Shadowbane       | Keep      | Slightly better vendor loot              | ~20 min             |
| Yvolfe Scorchedtail   | Yard      | Random drops                             | ~20 min             |

> **Randomized Loot (Frostreaver):** On Frostreaver free-trade + randomized-loot ruleset, named drops are not static. SMR/Netted Kelp may drop from any named in the zone. Validate actual drop tables on first clear.

### Plat-Per-Hour Estimate (10–25 range, 1 group)

- ~100–300 pp/hr from vendor trash + drops at 1 group clearing the yard/keep
- SMR (if drops) trades 2,000–5,000 pp on free-trade servers at Classic launch
- Netted Kelp trades 500–1,500 pp depending on demand
- Loot-per-kill plat value is secondary to XP economy here; do not delay XP for loot camps unless SMR is tracked

---

## Multibox Pull-Sequence

### Group Setup for Unrest (3 Groups)

| Group   | Assignment     | Camp Area    | Notes                                       |
| ------- | -------------- | ------------ | ------------------------------------------- |
| G1      | Puller + Tank  | Courtyard    | SK FD-pull, bard speed, 2 monks cleave      |
| G2      | Tank + sustain | Inside Keep  | WAR + CLR + BRD + SHM; steady chain pulls   |
| G3      | Tank + DPS     | Basement     | WAR + CLR + BRD + SHM + 2 MNK; boss zone   |

### Pull Sequence — Courtyard (10–15)

1. BRD jogs ahead, sings pull song to lock mob attention
2. SK snare + FD if add detected → mobs return to spawn
3. MNK feign-dump if BRD-assist train risks adds
4. Clear yard left-to-right; re-pull from spawn points every 2–4 min
5. Encounter lock fires as soon as first hit lands — no KS risk from other players

### Pull Sequence — Inside Keep (15–20)

1. BRD pulls single from doorway using song range
2. SHM slow immediately on engage; MNKs begin DPS rotation
3. CLR CH timer: cast at ~50% health for WAR tank
4. Stagger BRD re-pulls; do not chain-pull faster than CLR mana recovery (~30s between pulls)
5. Named spawn locations: Cauldronbubble near fireplace; Jack Shadowbane near stairs

### Pull Sequence — Basement (20–30)

1. BRD or MNK pulls from basement entrance ramp
2. SK or WAR positions mob with back to wall (MNK positional bonus)
3. SHM slow mandatory — festering hags hit hard (250+ per swing pre-slow)
4. BRD twist: haste song + mana song on rotation during fight
5. CLR downtime after festering hag packs — allow mana recovery before next named attempt
6. Do not rush Garanel Rucksif spawn; assign SK to watch placeholder location and FD-wait

### Timing Notes

- Average pull cycle (keep): ~90 seconds per mob at this level range
- Average pull cycle (basement): ~3–5 minutes per pack including SHM slow delay
- Camp is stable for AFK-light automation once CLR/SHM mana loop is dialed in
- BRD is the critical dependency: losing the bard significantly reduces pull speed and group mana

---

## Multibox Automation Considerations

- **Encounter locking** eliminates KS risk; pull freely without scouting other players
- **AFK safety:** Keep + yard can run with minimal supervision once pull cycle is stable
- Basement requires active monitoring — festering hag aggro radius is wide; adds kill groups if puller missteps
- SHM slow should be automated via TextQuest assist logic; manual slow = 20–30% throughput loss
- CLR Complete Heal threshold: set auto-CH trigger at 60% health for WAR, 70% for PAL/SK
- BRD pull automation: configure song-pull radius to avoid breaking adjacent room spawns

---

## Validation Checklist

> These items need live confirmation on Frostreaver before treating this doc as finalized.

- [ ] DPS floor numbers confirmed via kill timer logging (kills/hr at each sub-zone)
- [ ] Loot-per-kill values validated (randomized loot ruleset — which named drop SMR?)
- [ ] Pull cycle timing recorded under TextQuest automation vs. manual
- [ ] SHM slow success rate logged vs. festering hag (resist baseline unknown)
- [ ] CLR mana recovery loop confirmed stable for 3+ hour unattended sessions
- [ ] Garanel Rucksif placeholder spawn behavior documented (FD-wait viable?)
- [ ] Basement named spawn timers validated (placeholders vs. true timers)

---

## Related Pages

- [Frostreaver Farming Guide](Frostreaver-Farming-Guide.md) — Zone priority tables by level range
- [P99 Zone Guide](P99-Zone-Guide.md) — ZEM values and mob reference
- [Camp Runbooks](Camp-Runbooks.md) — Pull mechanics and automation runbooks
- [Class Combat Rotations](Class-Combat-Rotations.md) — SHM slow, CLR CH chain, BRD twist reference

---

*Last updated: 2026-04-26 | Issue #3378 | Parent #1778*
