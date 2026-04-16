# Frostreaver TLP Farming & XP Guide

## Velious-Start | Free Trade | Randomized Loot | Encounter Locking | No Truebox

### 36-Box Setup (6 Groups of 6)

Detailed early-planar raid research now lives in [Research: Plane of Hate and Plane of Sky Early Raid Targets](Research-Hate-and-Sky-Early-Raid-Targets.md). Use that page for lockout planning, epic-component mapping, and Krono-value assumptions.

---

## Table of Contents

1. [Finalized 36-Box Roster Reference](#finalized-36-box-roster-reference)
2. [Leveling Zones by Level Range](#leveling-zones-by-level-range)
3. [Plat Farming Locations by Era](#plat-farming-locations-by-era)
4. [Raid Targets Available at Velious Launch](#raid-targets-available-at-velious-launch)
5. [Encounter Locking Strategy](#encounter-locking-strategy)
6. [Randomized Loot Meta & Lessons from Mischief/Teek](#randomized-loot-meta)
7. [Group Composition Notes](#group-composition-notes)

---

## Finalized 36-Box Roster Reference

> Finalized from the `TextQuest#1521` owner decision recorded on 2026-04-14. The farm roster and the raid roster are intentionally the same 36-character stable, so leveling, gearing, and AA time spent farming flows directly into raid readiness.

| Group      | Tank | Healer | Support 1 | Support 2 | DPS 1 | DPS 2 |
| ---------- | ---- | ------ | --------- | --------- | ----- | ----- |
| G1 Driver  | SK   | CLR    | BRD       | SHM       | MNK   | MNK   |
| G2 Melee   | WAR  | CLR    | BRD       | SHM       | MNK   | MNK   |
| G3 Melee   | WAR  | CLR    | BRD       | SHM       | MNK   | MNK   |
| G4 Melee   | WAR  | CLR    | BRD       | SHM       | MNK   | MNK   |
| G5 Utility | PAL  | CLR    | BRD       | DRU       | BST   | RNG   |
| G6 Caster  | PAL  | CLR    | BRD       | ENC       | WIZ   | MAG   |

### Class Totals (36)

| Class | Count | Notes |
| ----- | ----- | ----- |
| Warrior | 3 | Defensive Discipline rotation for raid main-tank duty |
| Shadowknight | 1 | Driver character, snap aggro, FD pulls, and snare coverage |
| Paladin | 2 | Utility tanks for raid support and safer split-camp farming |
| Cleric | 6 | One per group; raid Complete Heal chain backbone |
| Bard | 6 | One per group; movement, pull control, ADPS, and resist coverage |
| Shaman | 4 | Slow, buffs, spot heals, and AFK alchemy backbone |
| Monk | 8 | Launch-era default melee DPS because they are effective before weapons stabilize |
| Druid | 1 | Ports, evac, snare, and backup heals |
| Enchanter | 1 | CC, mana utility, and jewelry crafting |
| Beastlord | 1 | Utility melee DPS with slow backup and pet support |
| Ranger | 1 | Tracking, outdoor pull utility, and ranged DPS |
| Wizard | 1 | Burst caster DPS and port support |
| Magician | 1 | Summons, Call of the Hero utility, and extra caster DPS |

### Farming-First Takeaway

This final roster does **not** maintain a separate raid-only bench. Instead, it favors six self-sufficient camp teams that can split across the world and then collapse into a full six-group raid without dead slots or pet-tank dependencies.

- Groups 1-4 are interchangeable melee farm teams: tank + cleric + bard + shaman with two monks for steady dungeon clearing.
- Group 5 is the outdoor and travel utility team: paladin durability, druid ports/evac, beastlord slow backup, and ranger tracking.
- Group 6 is the caster and logistics team: paladin safety, enchanter CC, wizard ports, and magician summon/vendor utility.
- AFK tradeskill scaling remains present but capped on purpose: four shamans cover alchemy, while the enchanter carries jewelry crafting. The roster gives up the original 6-8 shaman / 4-6 enchanter idea so more slots stay raid-viable.

### Farming/Raiding Overlap

| Role | Farming job | Raid job | Flex verdict |
| ---- | ----------- | -------- | ------------ |
| Warriors | Named-camp tanks for the three hardest groups | Main tanks and Defensive rotation anchors | Mandatory in both modes |
| Shadowknight | Driver, FD pulls, snap aggro, solo utility | Pull tank, aggro preload, pickup tank | High overlap, but not a warrior replacement |
| Paladins | Utility tanks for safer camps and recovery | Off-tanks, stun utility, backup heals | Strong flex tanks |
| Clerics | Group sustain and wipe recovery | Complete Heal chain backbone | Fixed-role backbone |
| Shamans | Slow, buffs, alchemy, backup healing | Slow, buffs, hybrid heals | Highest overlap support class |
| Bards | Pull speed, mana/song support, travel | Per-group ADPS, resists, movement | Mandatory in both modes |
| Monks | Weapon-light launch DPS and backup pulling | Primary melee DPS | Cleanest pure flex DPS slot |
| Druid / Enchanter | Travel, evac, tracking, CC, crafting | Utility healing, mana control, CC | Specialty flex slots |
| Beastlord / Ranger / Wizard / Mage | Outdoor utility, tracking, summon, vendor support | Remaining DPS and utility flex | Raid-safe, but not core tank/heal infrastructure |

### Raid-First Takeaway

- Three warriors are the real tank floor. The shadowknight plus two paladins cover pickup, add control, and lower-risk farm content, but raid tanking is built around the warrior trio.
- Six clerics are the hard commitment. Shamans and the druid supplement heals; they do not replace the cleric chain.
- Pet tanking is optional in farm content and explicitly unnecessary in raid planning. Mage and beastlord pets are bonus DPS and utility, not the main mitigation plan.
- The total raid target is the full 36-character roster. The composition is deliberately stable across one-group progression, six-group farming, and full-raid nights.

### Automation Survivability Core

For unattended camp automation, TextQuest treats `BRD + CLR + second healer/support-healer` as the minimum survivability core. The second healing slot is a deliberate trade: it gives up one higher-throughput DPS slot so the group can survive bad pulls, named pops, healer desync, and pathing mistakes without operator rescue.

- Preferred core: `tank + CLR + BRD + SHM + DPS + DPS`. Shaman is the best second healer because slow reduces incoming damage before the cleric has to spend mana catching up.
- Lower-throughput fallback: `tank + CLR + BRD + PAL + DPS + flex`. Paladin does not match shaman's slow value, but it adds pickup, stun, and emergency healing for recovery windows.
- Utility exception: `BRD + CLR` by itself is acceptable only for travel, caster logistics, or lower-risk camps. It is not the default unattended template for named-heavy or unstable camps.

Applied to the locked roster:

- Groups 1-4 cleanly satisfy the survivability core with `CLR + BRD + SHM`.
- Group 5 is a controlled exception: druid covers the second-healer role for travel and evac-heavy utility work, but it is still less robust than the shaman groups.
- Group 6 is the clearest exception. It keeps the universal `BRD + CLR` floor, but it should be treated as a support/logistics group unless adjacent support or lower-risk content keeps the recovery burden low.

### Partner-Contingent Planning

> `TextQuest#1588` reviewed the repository and GitHub issue history on 2026-04-15. No tracked evidence currently confirms Dave's participation or any other named partner roster commitment, so the finalized `TextQuest#1521` 36-box baseline stays unchanged until a partner's attendance and class mix are explicitly recorded.

| Scenario | Current evidence state | Planning rule | Composition impact |
| -------- | ---------------------- | ------------- | ------------------ |
| No confirmed partners | No GitHub/repo confirmation for Dave or any other partner | Keep the locked 36-box baseline exactly as documented above | No changes |
| Stable tank/heal-heavy partner | Partner attendance is confirmed and the partner's class mix is recorded for full-raid nights | Re-open roster planning before changing any owned accounts; do not make speculative cuts while confirmation is still missing | Candidate future cuts are redundant recovery slots, but only after the partner proves reliable enough to replace them |
| Stable DPS/utility-heavy partner | Partner attendance is confirmed, but they are not replacing the tank/heal backbone | Preserve the current tank/heal floor and treat partner DPS as bonus throughput first | Usually no baseline changes; partner accounts act as overflow |
| Irregular or one-off partner attendance | Partner may join some nights but is not a dependable every-raid presence | Treat partner accounts as bonus bench only and keep the owned roster self-sufficient | No permanent changes |

- The documented baseline remains the default because `TextQuest#1593` and `TextQuest#1594` both reinforce survivability-first automation over speculative DPS optimization.
- If a partner roster is later confirmed in tracked evidence, update the linked planning issue first and only then revise the canonical roster docs.

---

## Leveling Zones by Level Range

For pre-launch rehearsal routes that intentionally avoid the most-camped mainstream hotspots, see [Research: Test-Server Leveling Paths and Overlooked Zones](Research-Test-Server-Leveling-Paths.md).

### Level 1-10: Starter Zones (1-3 hours at fastest XP rate)

| Zone                    | Level Range | Why                                    | Multi-Group?             | Notes                                                                                                      |
| ----------------------- | ----------- | -------------------------------------- | ------------------------ | ---------------------------------------------------------------------------------------------------------- |
| **Crescent Reach**      | 1-15        | Tutorial zone, quest XP, centralized   | Yes - spread across zone | Best for TLP if available; all races can start here                                                        |
| **Racial Newbie Zones** | 1-10        | Guaranteed spawns, no competition      | Split groups by race     | Qeynos Catacombs (Human), Steamfont (Gnome), Butcherblock (Dwarf), Nektulos (DE), Misty Thicket (Halfling) |
| **Field of Bone**       | 1-15        | High ZEM, dense spawns, Kunark content | Yes - large outdoor zone | Excellent for multibox; pit area for 5-10, skeletons/scaled wolf for 1-5                                   |
| **Crushbone**           | 5-15        | Dense humanoid spawns, group friendly  | 1-2 groups max           | Entrance mobs 5-10, throne room 10-15                                                                      |

**Multibox Strategy (1-10):** Split your 6 groups across 2-3 newbie zones to avoid spawn contention. Field of Bone can handle 2-3 groups easily. With fastest XP rate, expect to blow through this range in under an hour.

---

### Level 10-20: Dungeons & Outdoor Camps

| Zone                 | Level Range | Why                                       | Multi-Group? | Notable Loot                                             | Notes                                                                            |
| -------------------- | ----------- | ----------------------------------------- | ------------ | -------------------------------------------------------- | -------------------------------------------------------------------------------- |
| **Unrest**           | 10-25       | Best dungeon ZEM in Classic, dense spawns | 2-3 groups   | Netted Kelp, Short Sword of Morin, Shining Metallic Robe | Courtyard 10-15, inside Keep 15-20, basement 20-25; THE premier leveling dungeon |
| **Crushbone**        | 10-15       | Continued from earlier                    | 1-2 groups   | Cracked Darkwood Shield, DVinn loot                      | Emperor DVinn camp at 13+                                                        |
| **Upper Guk**        | 10-20       | Good ZEM, frogloks                        | 1-2 groups   | Various spell components                                 | Transition to Lower Guk at 20                                                    |
| **Paludal Caverns**  | 10-25       | Highest raw XP/hour if available          | 2-3 groups   | Bandit Sashes (quest turn-in)                            | May or may not be available at Velious start                                     |
| **Lake of Ill Omen** | 10-20       | Outdoor kiting, Kunark zone               | 2 groups     | Sarnak loot                                              | Good for kiter alts, less ideal for your group comp                              |

**Multibox Strategy (10-20):** Run 2-3 groups in Unrest (courtyard + keep + yard), remainder in Crushbone or Field of Bone. With encounter locking, you own every pull -- no KS risk.

---

### Level 20-35: Mid-Level Grind

| Zone                       | Level Range | Why                                           | Multi-Group? | Notable Loot                                         | Notes                                                              |
| -------------------------- | ----------- | --------------------------------------------- | ------------ | ---------------------------------------------------- | ------------------------------------------------------------------ |
| **Unrest Basement**        | 20-30       | Continued from above                          | 1-2 groups   | Shining Metallic Robe, Netted Kelp                   | Push deeper into basement and yard                                 |
| **Lower Guk**              | 25-45       | Massive level range, incredible loot          | 3-4 groups   | FBSS, Bag of Sewn Evil-Eye, SMR, Mithril-Runed Tunic | Tunnels/Frenzy 25-30, purple camp 30-35, deep areas 35-45          |
| **Crystal Caverns**        | 25-40       | Velious zone, good ZEM                        | 2-3 groups   | Velious gems, armor drops                            | New with Velious unlock                                            |
| **Tower of Frozen Shadow** | 25-45       | Multi-floor dungeon, Velious                  | 2 groups     | Floor-specific named loot                            | Each floor = different level range; excellent for splitting groups |
| **Crypt of Dalnir**        | 25-35       | Amazing ZEM, rarely contested                 | 1-2 groups   | Dalnir-specific drops                                | Kunark dungeon, great for avoiding crowds                          |
| **Great Divide**           | 25-40       | Outdoor Velious zone                          | 2-3 groups   | Velious gems, coldain quest items                    | Giants and Dervishes                                               |
| **Ocean of Tears**         | 15-25       | Undead island, gargoyle eyes sell 8-10pp each | 1-2 groups   | Gargoyle Eyes (vendor), Cyclops gems                 | Undead camp is solid                                               |

**Multibox Strategy (20-35):** Lower Guk can absorb 3-4 groups across its many camps (frenzy, live side, dead side). Tower of Frozen Shadow spreads groups across floors. Crystal Caverns handles 2-3 groups. This range takes the most time -- lean into dungeons with good ZEM.

---

### Level 35-50: Kunark & Velious Dungeons

| Zone                      | Level Range | Why                             | Multi-Group? | Notable Loot                                             | Notes                                                                 |
| ------------------------- | ----------- | ------------------------------- | ------------ | -------------------------------------------------------- | --------------------------------------------------------------------- |
| **City of Mist**          | 35-50       | Amazing for boxers AND groupers | 3-4 groups   | Wraith Bone Hammer, Jade Reaver, various quest items     | Simply amazing for this range; spread across courtyard and inner city |
| **Lower Guk**             | 35-45       | Continued deep camps            | 1-2 groups   | FBSS, Bag of Sewn Evil-Eye                               | Purple and deeper camps                                               |
| **Temple of Droga**       | 35-45       | Challenging but valuable        | 1-2 groups   | Droga-specific gear                                      | Kunark goblin dungeon                                                 |
| **Karnor's Castle**       | 40-55       | Dense spawns, good XP           | 3-4 groups   | KC-specific drops, Venril Sathir loot                    | Excellent dungeon crawl; can absorb multiple groups                   |
| **Nagafen's Lair (SolB)** | 35-50       | Classic dungeon farming         | 1-2 groups   | Cloak of Shadows, Golden Efreeti Boots, Mithril armor    | Fire giants and bats                                                  |
| **The Hole**              | 40-55       | Antisocial boxing heaven        | 2-3 groups   | Loam Encrusted gear, Withered Leather, Idol of Underking | Relatively peaceful even on populated servers                         |
| **Velketor's Labyrinth**  | 40-55       | Velious dungeon crawl           | 2-3 groups   | Spider Fur Belt/Collar/Cloak, Lute of the Howler         | Trash kobolds drop valuable Spider Fur items regularly                |

**Multibox Strategy (35-50):** City of Mist is the all-star here -- spread 3-4 groups across the zone. Karnor's Castle is the overflow. For plat, rotate 1-2 groups through SolB for Efreeti camps. The Hole for your antisocial groups that want zero competition.

---

### Level 50-60: Endgame Leveling (Velious Cap)

Old Sebilis remains research-backed and still needs live proof for Scars-launch
access, spawn cadence, camp overlap, Nodding Blue Lily forage rate, and
automation risk. Use [Sebilis Farming
Validation](Sebilis-Farming-Validation.md) as the canonical ledger before
treating the zone as a solved overnight farm.

| Zone                          | Level Range | Why                                | Multi-Group? | Notable Loot                                                                                         | Notes                                                                                                              |
| ----------------------------- | ----------- | ---------------------------------- | ------------ | ---------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| **Old Sebilis**               | 50-60       | Best XP in the game for Kunark     | 4-6 groups   | Box of Nil Space, Cone of Mystics, Froglok Bonecaster's Robe, Hierophant's Cloak, Runebranded Girdle | Right wing (Disco 1+2) for lower; juggs/myconids underneath for money AND XP. Can easily absorb your entire 36-box |
| **Chardok**                   | 50-60       | Dungeon crawl, incredible loot     | 3-4 groups   | Incarnadine BP/Greaves, Vibrating Gauntlets of Infuse, Fancy Velvet Mantle                           | Entrance is light blue at 60; deeper = harder + better loot                                                        |
| **Kael Drakkel**              | 48-60       | Velious endgame, armor quest drops | 3-6 groups   | Thurgadin armor templates, quest gems, named drops                                                   | Iceshard Manor (48-53), Arena (55+), King Tormax area (58+). Giants drop Thurgadin group armor pieces              |
| **Howling Stones (Charasis)** | 50-60       | Antisocial boxing                  | 2-3 groups   | Hand of the Reaper, Fingerbone Hoop, Enshrouded Veil, Golden Bracer                                  | South wing most profitable                                                                                         |
| **Siren's Grotto**            | 50-60       | Velious dungeon                    | 2-3 groups   | Spiked Seahorse Hide Belt, Drums of the Beast, Sleeves of Kelpmaidens, tradeskill gems               | Fellspine camp; zone-wide ink drops are bazaar profitable                                                          |
| **Dragon Necropolis**         | 52-60       | Antisocial boxing, Velious         | 2-3 groups   | Phase Spider Carapace, Queen Raltaas drops                                                           | Phase spiders/snakes/Chetari rats; trash is easy, nameds are hard                                                  |
| **Velketor's Labyrinth**      | 50-60       | Velious dungeon, AoE spot          | 2-3 groups   | Spider Fur items, kobold drops                                                                       | Primary AoE leveling spot in Velious (heavily camped on normal servers, less so with 36-box)                       |
| **Western Wastes**            | 55-60       | Dragon farming, rare spells        | 2-3 groups   | Burnout IV, Protection of the Glades, dragon Talisman necklaces (+4-7 all stats)                     | Not best XP but incredible drops; con every named before pulling                                                   |
| **Cobalt Scar**               | 50-60       | Wyvern farming outdoor             | 2-3 groups   | Wyvern hides (tailoring), armor gems (~40pp each, 800pp/stack)                                       | Southeast corner near Dragon Circle; ~250-300pp per session per group                                              |

**Multibox Strategy (50-60):** Old Sebilis is your home base -- it can genuinely absorb all 6 groups across Disco 1+2, left wing, crypt, and juggs/myconids. Run 2-3 groups in Seb, 2 in Kael (for armor quest drops), 1-2 in Chardok or Howling Stones. Rotate groups to Western Wastes for rare spell farming.

---

## Plat Farming Locations by Era

### Classic Era Plat Farms

| Location                        | Est. Plat/Hour | Method                                     | Notes                                                                                                  |
| ------------------------------- | -------------- | ------------------------------------------ | ------------------------------------------------------------------------------------------------------ |
| **Rathe Mountains Hill Giants** | 400-600pp      | Vendor trash raw plat                      | "Classic EQ's ATM bank"; 10k+ daily potential                                                          |
| **Everfrost Giants**            | 400-600pp      | Same loot table as Hill Giants             | Outside Permafrost; highly trafficked                                                                  |
| **Lower Guk**                   | Variable       | FBSS + rare named drops                    | FBSS sells extremely well in free trade; with randomized loot, any rare in zone can drop off any named |
| **Nagafen's Lair (SolB)**       | Variable       | Golden Efreeti Boots, Cloak of Shadows     | High-value rare drops for free trade                                                                   |
| **Ocean of Tears Cyclops**      | 200-400pp      | Gems + Ancient Cyclops Ring (JBoots quest) | Good for kiting                                                                                        |
| **Gorge of King Xorbb**         | 150-300pp      | Polished Bone Bracelets, pelts, gems       | ~2k vendor trash + 20-22 bracelets over a session                                                      |
| **Castle Mistmoore**            | Variable       | Hooded Black Cloak (rare)                  | High competition for HBC                                                                               |
| **The Hole**                    | 300-500pp      | Loam Encrusted gear, Idol of Underking     | Low competition, long-term returns                                                                     |

### Kunark Era Plat Farms

| Location                         | Est. Plat/Hour | Method                                             | Notes                                     |
| -------------------------------- | -------------- | -------------------------------------------------- | ----------------------------------------- |
| **Old Sebilis (Juggs/Myconids)** | 500-1000pp     | Jug drops + named loot                             | Underground section is where the money is |
| **Chardok**                      | 400-800pp      | Named drops: Incarnadine gear, Vibrating Gauntlets | Deeper = harder = better loot             |
| **Howling Stones South Wing**    | 400-700pp      | Hand of the Reaper, Fingerbone Hoop                | Most profitable wing                      |
| **Burning Woods**                | 200-400pp      | Wurm drops, giant drops                            | Good vendor trash                         |

### Velious Era Plat Farms

| Location                | Est. Plat/Hour  | Method                                                                | Notes                                                        |
| ----------------------- | --------------- | --------------------------------------------------------------------- | ------------------------------------------------------------ |
| **Kael Drakkel**        | 500-1500pp      | Thurgadin armor templates + quest gems + named drops                  | Every frost giant drops armor pieces; gems sell well         |
| **Western Wastes**      | Variable (high) | Rare spell drops (Burnout IV, Protection of Glades), dragon Talismans | Spells are worth thousands; low XP but insane loot potential |
| **Cobalt Scar Wyverns** | 250-400pp       | Wyvern hides (sell to tailors), armor gems (~40pp each)               | Consistent, steady income; ~800pp per stack of gems          |
| **Siren's Grotto**      | 300-600pp       | Quest gems, tradeskill gems, ink drops, Spiked Seahorse Hide Belt     | Zone-wide ink drops for bazaar profit                        |
| **Velketor's Lab**      | 200-500pp       | Spider Fur items (Belt, Collar, Cloak)                                | Even trash kobolds drop these regularly                      |
| **Dragon Necropolis**   | 200-500pp       | Phase Spider Carapace, Chetari drops                                  | Easy trash, hard nameds                                      |
| **Iceclad Ocean**       | Variable        | Lodizal Shell Shield, Lodizal Shell Boots, Belt of Great Turtle       | Lodizal is timed spawn; shield is very valuable              |
| **ToV West Wing Trash** | 800-2000pp      | Kael quest armor unmade pieces drop here                              | Raid-tier plat farming                                       |

### Free Trade Multiplier

On a free trade server like Frostreaver, every item is tradeable. This means:

- Raid drops that are normally NO TRADE can be sold
- Gear cascades down as guilds upgrade, creating a healthy economy
- Even "bad" random loot rolls produce tradeable items worth selling
- Focus on volume of named kills rather than specific camps

---

## Raid Targets Available at Velious Launch

With 36 characters (6 full groups), you have a significant force. Here is what is feasible:

### Classic Raid Targets (Trivial with 36 at 60)

| Target                   | Zone           | Difficulty @36 | Key Loot                           | Notes                                                                                   |
| ------------------------ | -------------- | -------------- | ---------------------------------- | --------------------------------------------------------------------------------------- |
| **Lord Nagafen**         | Nagafen's Lair | Trivial        | Cloak of Flames, various           | Normally needs 25-35; trivial with 36 at 60. **Level 52 cap removed on TLP at Velious** |
| **Lady Vox**             | Permafrost     | Trivial        | McVaxius' Horn of War, various     | Normally needs 18-25; trivial at 60                                                     |
| **Cazic-Thule**          | Plane of Fear  | Easy           | Planar armor, class-specific drops | Full break-in needed; your 36-box handles it easily                                     |
| **Plane of Fear Golems** | Plane of Fear  | Easy           | Random planar armor pieces         | Jello golems drop random class planar armor                                             |
| **Innoruuk**             | Plane of Hate  | Easy           | Planar armor, hate-specific gear   | Can clear entire zone                                                                   |
| **Plane of Sky**         | Plane of Sky   | Easy-Moderate  | Quest armor, island drops          | Multi-island; systematic clear with 36                                                  |
| **Phinigel Autropos**    | Kedge Keep     | Trivial        | Various underwater drops           | Can solo-group at 60                                                                    |

### Kunark Raid Targets

| Target                     | Zone              | Difficulty @36 | Key Loot                     | Notes                                                 |
| -------------------------- | ----------------- | -------------- | ---------------------------- | ----------------------------------------------------- |
| **Trakanon**               | Old Sebilis       | Easy           | Trakanon's Teeth, various    | With randomized loot, could drop any Kunark raid item |
| **Venril Sathir**          | Karnor's Castle   | Easy           | VS-specific drops            |                                                       |
| **Gorenaire**              | Dreadlands        | Easy           | Dragon loot                  | Open-world dragon                                     |
| **Severilous**             | Emerald Jungle    | Easy           | Dragon loot                  | Open-world dragon                                     |
| **Talendor**               | Skyfire Mountains | Easy           | Dragon loot                  | Open-world dragon                                     |
| **Faydedar**               | Timorous Deep     | Easy           | Dragon loot                  | Island dragon                                         |
| **Wuoshi**                 | Wakening Land     | Easy           | Dragon loot                  | Crossover Velious/Kunark                              |
| **Kelorek'Dar**            | Cobalt Scar       | Easy           | Dragon loot                  |                                                       |
| **Chardok Royals**         | Chardok           | Easy           | Royal loot                   | Multiple named in deep Chardok                        |
| **Veeshan's Peak Dragons** | Veeshan's Peak    | Moderate       | VP-tier loot, best in Kunark | Requires key quest; multiple dragons inside           |

### Velious Raid Targets

| Target                     | Zone              | Difficulty @36 | Key Loot                                                      | Notes                                                    |
| -------------------------- | ----------------- | -------------- | ------------------------------------------------------------- | -------------------------------------------------------- |
| **Klandicar**              | Western Wastes    | Easy           | Dragon talisman, rare drops                                   | Open-world named dragon                                  |
| **Sontalak**               | Western Wastes    | Easy           | Dragon talisman, rare drops                                   | Near ToV entrance                                        |
| **Zlandicar**              | Dragon Necropolis | Easy-Moderate  | Zlandicar-specific gear                                       | Underground dragon                                       |
| **Velketor the Sorcerer**  | Velketor's Lab    | Easy           | Velketor loot                                                 | End boss of Velks                                        |
| **Dain Frostreaver IV**    | Icewell Keep      | Moderate       | Dain-specific loot                                            | Requires Coldain faction work                            |
| **Derakor the Vindicator** | Kael Drakkel      | Moderate       | Derakor loot, giant faction drops                             | Named giant boss                                         |
| **King Tormax**            | Kael Drakkel      | Moderate-Hard  | King Tormax loot                                              | Requires clearing to throne room                         |
| **Lord Yelinak**           | Skyshrine         | Moderate       | Yelinak loot                                                  | Dragon lord                                              |
| **Tunare**                 | Plane of Growth   | Moderate-Hard  | Nature-themed raid loot                                       | Full raid clear needed                                   |
| **ToV Halls of Testing**   | Temple of Veeshan | Moderate       | HoT dragon drops, Dozekar quest items                         | Mid-tier ToV content; doable with 36                     |
| **ToV West Wing**          | Temple of Veeshan | Moderate       | Kael quest armor (unmade pieces)                              | CoV-faction aligned; safe if ally faction                |
| **ToV North Wing (NToV)**  | Temple of Veeshan | Hard           | Best loot in Velious era                                      | See NToV section below                                   |
| **Vulak'Aerr**             | NToV              | Very Hard      | Abashi's Rod, Do'Vassir's Gauntlets, Crystasia's Ring (AC 30) | Final boss of Velious; 36 well-geared chars may be tight |
| **Sleeper's Tomb**         | Sleeper's Tomb    | Hard-Very Hard | ST-specific loot, warders                                     | Ancient dragons; significant challenge                   |

### NToV Dragon Difficulty Ranking (for your 36-box)

From easiest to hardest within NToV:

1. **Sevalak** (Paladin, slowable) - Easiest of the triplets
2. **Zlexak** (Rogue, slowable) - AE every ~18 sec, Diseased Cloud
3. **Cekenar** (Monk, gates) - Electric Blast, belly caster
4. **Eashen of the Sky** (Wizard, unslowable) - Rain of Molten Lava, manageable
5. **Lady Mirenilla** (Warrior, slowable) - Call of the Zero, Frost Strike
6. **Jorlleag** (Cleric, slowable, gates, CHs) - Must burn before CH lands
7. **Lady Nevederia** (Enchanter, perma-rooted, 300k HP) - Fearsome but stationary
8. **Ikatiar the Venom** (Rogue, unslowable) - High sustained damage
9. **Lord Feshlak** (Warrior, gates) - Wave of Cold + Wave of Fire
10. **Lord Kreizenn** (Monk) - Wave of Flame, cold DD + stun AE
11. **Dagarn the Destroyer** (Level 70 Wurm, slowable, rampages) - Call of Zero, Rain of Molten Lava
12. **Lord Vyemm** (Paladin) - "Arguably the most difficult dragon in ToV" even harder than Vulak solo
13. **Vulak'Aerr** (Level 70, final boss) - Reigning lord of ToV, premium loot

**36-Box Feasibility:** With 6 CLR, 6 BRD, 4 SHM, a dedicated ENC, and a real tank core of 3 WAR + 1 SK + 2 PAL, your force can handle most NToV content. Lord Vyemm and Vulak'Aerr will still require strong Kael/Kunark gear and disciplined healing rotations, but the roster has enough real tanks, healers, and monk DPS to clear the zone without leaning on pet tanking.

---

## Encounter Locking Strategy

### How It Works on Frostreaver

- **Lock trigger:** First player to get on an NPC's hate list (attack it or get attacked by it) locks the encounter
- **Visual:** Locked NPC names show GREEN if you own the lock, GREY if you don't
- **Spawn protection:** NPCs have a short random-duration lock on spawn during which nobody can attack
- **Unlock:** If NPC's hate list empties and it leaves combat, it unlocks after a short countdown
- **Reset:** If someone on the lock list attacks before unlock completes, countdown resets
- **Group ownership:** Lock belongs to the individual player, but extends to their group/raid
- **/yell unlock:** Targeting a locked NPC and using /yell semi-unlocks it for anyone; for rare/hard NPCs, only group/raid leader can do this

### Multibox Pulling Strategy with Encounter Locking

**Advantages for your 36-box:**

1. **No KS risk** -- once your puller tags a mob, it's yours. Nobody can steal it
2. **No training risk** -- other players' trained mobs walk right past you (grey names)
3. **Pull freely** -- you can have mobs chasing you, dying at your feet, everything just walks away if it's not yours
4. **Camp security** -- tag a camp's named and it's locked to you

**Best Practices:**

1. **Designate pullers per group:** SK (FD splitting), BRD (speed pulling with Selos), or even WIZ (root parking) -- one puller per group
2. **SK FD pull method:** SK runs in, hits every mob once (tagging all to your group), then FDs. Mobs path back but are all locked to your group. Pick them off one at a time
3. **Race to tag on spawn:** For contested named spawns, have a puller pre-positioned and ready to attack the instant the spawn protection drops
4. **Split camps across groups:** Each group locks its own camp. 6 groups = 6 simultaneously locked camp areas in a dungeon
5. **Raid formation for bosses:** Combine all 6 groups into a raid; one person tags the boss, entire raid can engage

**Power Leveling with Encounter Locking:**

- Put lower-level alts in the group with a high-level tagger
- The tagger hits every mob (locking to the group), then you can kill with lower-level characters getting XP
- Works for catch-up leveling of replacement characters

---

## Randomized Loot Meta

### How Randomized Loot Works (Learned from Mischief/Teek)

- **Rare NPCs** drop loot from other rare NPCs of a **similar level within the same expansion**
- **Raid NPCs** drop loot from other raid NPCs of a **similar level within the same expansion**
- Loot randomization is **expansion-based, not zone-based**
- There is a chance at **extra drops** beyond the normal loot table
- Loot tiers depend on **mob's actual level at time of spawning**, not its max level

### How This Changes Camp Selection

1. **Kill volume > specific camp:** On a normal server, you camp Frenzy for FBSS. On randomized loot, ANY Classic rare of similar level could drop FBSS. Farm the fastest-spawning rares, not specific ones.

2. **Zone for density of rares, not specific loot:** Zones with the MOST rare spawns per hour give the most chances at good randomized loot. Prioritize:
   - **Lower Guk** (many named spawns)
   - **Old Sebilis** (tons of nameds across the zone)
   - **Chardok** (dense named population)
   - **Kael Drakkel** (numerous giant nameds)

3. **Raid loot floods the market:** With free trade + randomized loot, raid gear becomes widely available. Don't overpay for gear early -- prices crash within weeks.

4. **Multi-camp advantage:** With 6 groups, you can hold 6+ named spawn camps simultaneously. On randomized loot, each named kill is a lottery ticket for ANY same-tier item.

5. **Sell early, sell fast:** When an expansion first unlocks, items are at peak value. Farm hard the first 2 weeks and sell everything. Prices drop 50-80% within a month.

### Lessons from Mischief Server

- **ST and ToV weekly clears** yielded ~2 million plat per week for organized groups
- The randomized loot system prevented single-camp monopolies -- bot farmers couldn't corner the market on specific items
- **Extra drops** created massive supply of 3rd-10th best-in-slot gear, available very cheaply
- Free trade meant every drop had value -- even "bad" rolls could be sold
- **Gearing alts was trivial** -- buy raid gear off the bazaar for a fraction of normal server prices
- The economy favored **high-volume farmers** (exactly your 36-box setup)

### Optimal Randomized Loot Farming Rotation

**Week 1-2 (Fresh Velious):**

- Rush 1-2 groups to 60 in Old Sebilis
- Farm Kael Drakkel for Thurgadin armor templates (huge demand early)
- Farm Western Wastes for rare Velious spells (peak value)
- Kill every open-world dragon you can find (randomized raid loot)

**Week 3-4 (Gearing Phase):**

- Run all Classic + Kunark raids weekly (trivial, but randomized loot makes them profitable)
- Begin ToV Halls of Testing and West Wing
- Sell everything that isn't an upgrade

**Week 5+ (Established):**

- Full NToV clears weekly
- King Tormax / Dain Frostreaver kills
- Rotate through all Velious endgame zones for rare spawns
- Buy cheap bazaar gear for alts using the plat you've farmed

---

## Group Composition Notes

### Optimal Zone Assignments by Group Strength

| Group | Best Role | Ideal Zones |
| ----- | --------- | ----------- |
| G1 (SK/CLR/BRD/SHM/MNK/MNK) | Driver group, FD pulls, controlled named farming | Howling Stones, Chardok, Sebilis camps that need precise pulls |
| G2-G4 (WAR/CLR/BRD/SHM/MNK/MNK) | Interchangeable melee grind teams | Old Sebilis, Kael Drakkel, Crystal Caverns, Tower of Frozen Shadow |
| G5 (PAL/CLR/BRD/DRU/BST/RNG) | Outdoor utility, evac insurance, tracking | Western Wastes, Cobalt Scar, Great Divide, open-world named routes |
| G6 (PAL/CLR/BRD/ENC/WIZ/MAG) | Caster utility, CC, summon/vendor support | Velketor's, Siren's Grotto, support duty for contested named or recovery pulls |

### Raid Formation

For raids, combine all 6 groups into a single raid:

- 3 WAR handle the main tank rotation; the SK is the driver/pull tank and the 2 PAL handle off-tank and utility assignments.
- 6 CLR form the Complete Heal chain, while 4 SHM + 1 DRU cover slows, buffs, and spot-heal cleanup.
- 6 BRD remain one-per-group so every raid group keeps movement, resist songs, and ADPS online.
- 8 MNK are the default launch-era DPS stack because they are immediately effective before weapon supply stabilizes.
- MAG and BST pets add damage, but raid survival is built around real tanks and healers rather than pet tanking.

---

## Quick Reference: Top Picks Per Level Range

| Range   | #1 Zone       | #2 Zone                | #3 Zone         | Plat Farm                |
| ------- | ------------- | ---------------------- | --------------- | ------------------------ |
| 1-10    | Field of Bone | Crushbone              | Racial zones    | N/A                      |
| 10-20   | Unrest        | Upper Guk              | Crushbone       | Gargoyle Eyes (OoT)      |
| 20-35   | Lower Guk     | Tower of Frozen Shadow | Crystal Caverns | Hill Giants (Rathe)      |
| 35-50   | City of Mist  | Karnor's Castle        | The Hole        | SolB (Efreeti)           |
| 50-60   | Old Sebilis   | Kael Drakkel           | Chardok         | Western Wastes (spells)  |
| 60 Farm | Kael Drakkel  | Western Wastes         | Siren's Grotto  | ToV (weekly raid clears) |

---

## Detailed P99 Wiki Zone Data

See [P99 Wiki Zone Guide](P99-Zone-Guide.md) for the full P99 wiki-sourced zone reference with exact ZEM values, mob names, camp coordinates, quest turn-ins, and multi-group capacity ratings for every level range.

---

## Sources

- [Project 1999 Per-Level Hunting Guide](https://wiki.project1999.com/Per-Level_Hunting_Guide) -- Primary source for ZEM values, level ranges, camp details
- [Project 1999 Treasure Hunting Guide](https://wiki.project1999.com/Treasure_Hunting_Guide) -- Plat farming and rare drop locations
- [Project 1999 Recommended Levels and ZEM List](https://wiki.project1999.com/Recommended_Levels_and_ZEM_List) -- Zone experience modifier reference
- [Project 1999 Player Guides](https://wiki.project1999.com/Player_Guides) -- Index of class and zone guides
- [Velious Power Leveling Guide 1-60 (EverQuest Guides)](https://www.everquestguides.com/everquest-leveling/velious-power-leveling-guide-1-60/)
- [Almar's Velious Leveling Guide](https://almarsguides.com/eq/leveling/velious/)
- [Almar's Classic Farming Locations](https://almarsguides.com/eq/farming/classic/)
- [Almar's Velious Farming Locations](https://almarsguides.com/eq/farming/velious/)
- [Almar's Kunark Farming Locations](https://almarsguides.com/eq/farming/kunark/)
- [Almar's Kael Drakkel Leveling Guide](https://almarsguides.com/eq/leveling/Velious/Locations/KaelDrakkel.cfm)
- [EQProgression Zone Leveling Guide](https://www.eqprogression.com/zone-leveling-guide/)
- [EQProgression Scars of Velious Raid Overview](https://www.eqprogression.com/scars-of-velious-raid-guide-overview/)
- [EQProgression Classic Raid Boss Guide](https://www.eqprogression.com/classic-raid-guide-overview/)
- [EQProgression TLP Server Rules and Info](https://www.eqprogression.com/tlp-server-rules-and-info/)
- [Temple of Veeshan (Project 1999 Wiki)](https://wiki.project1999.com/Temple_of_Veeshan)
- [Velious Raiding Gear (Project 1999 Wiki)](https://wiki.project1999.com/Players:Velious_Raiding_Gear)
- [Fangbreaker Rulesets FAQ (EverQuest Official)](https://www.everquest.com/guides/eq-2025-tlp-ruleset-faq)
- [Mischief TLP Discussion (Fires of Heaven)](https://www.firesofheaven.org/threads/eq-tlp-mischief-free-trade-random-loot.12847/)
- [Dynamic Loot Tiers (RedGuides)](https://www.redguides.com/community/threads/info-random-loot-server-dynamic-loot-tiers.91960/)
- [Encounter Locking Discussion (RedGuides)](https://www.redguides.com/community/threads/encounter-locking-an-end-to-power-levelling.86342/)
- [ZAM Zone Level Chart](https://everquest.allakhazam.com/db/zlvlchart.html)
- [Cobalt Scar Leveling Guide (Almar's)](https://www.almarsguides.com/EQ/Leveling/Velious/Locations/CobaltScar.cfm)
- [Raid Tiers (Classic EQ Wiki)](http://eqc.wikidot.com/raid-tiers)
