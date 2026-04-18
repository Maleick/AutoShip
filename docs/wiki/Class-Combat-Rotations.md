# EverQuest Class Combat Rotations: Classic / Kunark / Velious

Reference for Frostreaver automation. Covers the 11 classes relevant to the 36-box TLP setup.
Spell levels, rotation priorities, mana management, and group roles.

---

## 1. Warrior (Tank)

### Key Abilities & Disciplines

| Ability / Discipline     | Level | Notes                                                |
| ------------------------ | ----- | ---------------------------------------------------- |
| Kick                     | 1     | Basic damage ability, use on cooldown                |
| Taunt                    | 1     | Forces aggro check; use every cooldown (6s)          |
| Bash (with shield)       | 6     | Stun + aggro; requires shield equipped               |
| Disarm                   | 35    | Situational                                          |
| Evasive Discipline       | 52    | +Avoidance, 3 min duration, 15 min reuse             |
| Charge Discipline        | 53    | +Hit chance, 14s duration, 30 min reuse              |
| Mighty Strike Discipline | 54    | All attacks crit, 10s duration, 60 min reuse         |
| Defensive Discipline     | 55    | +45% melee mitigation, 3 min duration, 15 min reuse  |
| Furious Discipline       | 56    | Riposte every incoming blow, short duration          |
| Fortitude Discipline     | 59    | Near-immunity to melee (must face mob), 15 min reuse |

### Combat Priority (Group Tanking)

1. **Taunt** on cooldown (every 6 seconds)
2. **Bash** on cooldown (stun + aggro generation)
3. **Kick** on cooldown (minor damage + aggro)
4. Auto-attack with best weapon; dual wield or sword-and-board depending on content

### Discipline Usage Priority

1. **Defensive** -- default "oh crap" disc for sustained incoming damage
2. **Fortitude** -- boss fights, must be facing the mob
3. **Evasive** -- lighter tanking situations, shorter cooldown than Defensive
4. **Mighty Strike** -- burn phase, use when mob is slowed and aggro is secure

### Aggro Generation

Warriors generate aggro primarily through **melee swings** (auto-attack). Haste directly increases aggro generation rate. Taunt forces an aggro check but does not generate large hate. Bash and Kick add small hate increments. There are no warrior aggro spells in Classic-Velious.

### Mana Management

N/A -- Warriors have no mana. Endurance is not a mechanic until later expansions.

### Group Role

Primary tank. Establish and hold aggro. Call for heals. Position mobs facing away from group. Use disciplines to survive burst damage. Relies entirely on the group for healing, slows, and haste buffs.

### Automation Notes

- Taunt every 6s tick
- Bash and Kick on their respective cooldown timers
- Disc usage is situational -- trigger Defensive/Fortitude when HP drops below threshold
- No spell gem management needed

---

## 2. Cleric (Healer)

### Key Spells

| Spell             | Level | Mana | Heal  | Cast Time | Notes                               |
| ----------------- | ----- | ---- | ----- | --------- | ----------------------------------- |
| Minor Healing     | 1     | 10   | 12    | 1.0s      | Training heal                       |
| Light Healing     | 5     | 25   | 47    | 2.0s      | Low level workhorse                 |
| Healing           | 9     | 50   | 135   | 3.0s      |                                     |
| Greater Healing   | 19    | 100  | 280   | 4.0s      | Primary heal through mid-levels     |
| Superior Healing  | 29    | 150  | 500   | 4.0s      | Main fast heal through 50s          |
| Complete Heal     | 39    | 400  | 7500  | 10.0s     | Full heal; backbone of raid healing |
| Celestial Healing | 44    | 200  | 400x4 | 2.5s      | HoT (4 ticks over 24s); 1600 total  |
| Celestial Elixir  | 59    | 450  | 600x4 | 2.5s      | HoT upgrade (4 ticks); 2400 total   |

**Buff Spells:**

| Spell                  | Level | Notes                          |
| ---------------------- | ----- | ------------------------------ |
| Courage / Center       | 1-55  | Stat buffs (HP/AC)             |
| Armor of Faith         | 24    | AC buff                        |
| Shield of Words        | 39    | AC/HP buff                     |
| Heroic Bond / Aegolism | 55-60 | Top-tier HP/AC buff (Velious)  |
| Symbol of Naltron      | 55    | HP buff                        |
| Resurrection           | 47    | 90% exp return (96% with epic) |

### Healing Rotation Priority

**Group Healing (XP groups):**

1. Keep **Superior Healing** as primary reactive heal (fast, decent efficiency)
2. Use **Complete Heal** on tank when damage is predictable and steady
3. **Celestial Healing/Elixir** as a pre-heal or supplement during heavy pulls
4. Save **Complete Heal** for emergencies when tank drops fast

**Raid Healing (Complete Heal Chain / CH Chain):**

1. Assign 3-5 clerics to a CH rotation on the main tank
2. Each cleric fires Complete Heal in sequence, timed so one lands every 2-3 seconds
3. Use /timer or external tool to coordinate (e.g., Cleric 1 fires at 0s, Cleric 2 at 3s, Cleric 3 at 6s)
4. CH chain requires discipline -- early casting wastes the heal, late casting lets the tank die
5. Outside the chain, spot-heal with Superior Healing

### Mana Management

- Sit and meditate between heals (6s server tick restores mana based on Meditation skill)
- Clarity/Clarity II from enchanter is critical (6/10 mana/tick)
- Mana preservation: use the most efficient heal for the damage taken
- Complete Heal is only efficient if tank is missing >932 HP (otherwise Superior Healing wins on mana)
- Avoid overhealing -- wasted mana is the #1 problem

### Group Role

Primary healer. Keep tank alive. Buff group with HP/AC buffs. Resurrect dead players (high priority in XP groups to minimize exp loss). Stun undead when possible. Do NOT nuke unless group is trivially easy content.

### Automation Notes

- Monitor tank HP; fire heal when HP drops below threshold
- CH chain requires precise timing coordination across multiple cleric characters
- Buff cycle: Symbol > Aegolism > AC buff on all group members at camp
- Res priority: always res before rebuffing

---

## 3. Enchanter (Crowd Control / Buffs)

### Key Spells

**Mesmerize (Mez) Line:**

| Spell             | Level | Duration | Notes                               |
| ----------------- | ----- | -------- | ----------------------------------- |
| Mesmerize         | 4     | 18s      | Single target, first mez            |
| Enthrall          | 12    | 36s      | Upgrade                             |
| Entrance          | 24    | 72s      | Workhorse mez for mid-levels        |
| Dazzle            | 36    | 96s      | Long duration, great for groups     |
| Glamour of Kintaz | 53    | 96s      | Kunark upgrade, higher resist check |
| Mesmerization     | 60    | 96s      | Velious best single-target mez      |

**AoE Mez:**

| Spell       | Level | Notes                               |
| ----------- | ----- | ----------------------------------- |
| Color Flux  | 2     | PBAE stun (not technically mez); 6s |
| Color Shift | 16    | PBAE stun upgrade                   |
| Color Skew  | 30    | PBAE stun upgrade                   |
| Color Slant | 44    | PBAE stun upgrade                   |

**Haste Line:**

| Spell                | Level | Haste% | Notes                         |
| -------------------- | ----- | ------ | ----------------------------- |
| Quickness            | 16    | 30%    | First haste                   |
| Alacrity             | 24    | 40%    | Main haste through 40s        |
| Swift like the Wind  | 44    | 64%    | Top Classic haste             |
| Wonderous Rapidity   | 57    | 70%    | Kunark haste                  |
| Speed of the Shissar | 60    | 72%    | Velious top haste (raid drop) |

**Clarity (Mana Regen) Line:**

| Spell      | Level | Regen        | Notes                           |
| ---------- | ----- | ------------ | ------------------------------- |
| Breeze     | 14    | 3 mana/tick  | Basic mana regen                |
| Clarity    | 29    | 6 mana/tick  | The spell everyone wants        |
| Clarity II | 54    | 10 mana/tick | Kunark; makes you indispensable |

**Debuffs:**

| Spell               | Level | Notes                                    |
| ------------------- | ----- | ---------------------------------------- |
| Tashan              | 4     | -MR debuff; always cast before mez/charm |
| Tashani             | 20    | Upgrade                                  |
| Tashania            | 44    | Major upgrade                            |
| Tashanian           | 60    | Velious best tash                        |
| Slow (Languid Pace) | 12    | Enchanter slow, weaker than shaman       |

**Charm:**

| Spell              | Level | Notes                |
| ------------------ | ----- | -------------------- |
| Charm              | 12    | Level 15 cap charm   |
| Beguile            | 30    | Level 35 cap charm   |
| Cajoling Whispers  | 46    | Level 52 cap charm   |
| Allure             | 51    | Charm up to level 53 |
| Boltran's Agacerie | 58    | Velious best charm   |

### Combat Rotation Priority (Group CC)

1. **Tash** the incoming mob (reduces magic resist for mez/slow)
2. **Mez** all adds immediately (highest level mob first -- it resists most)
3. **Slow** the mob the group is killing (if no shaman in group)
4. **Haste** the tank (keep buff active at all times)
5. **Clarity** on all casters (pre-buff, refresh as needed)
6. **Re-mez** before mez wears off (watch timers -- mez breaks cause wipes)
7. **Charm** for DPS if situation allows (advanced; risky in automation)

### Mez Priority Order

1. Mez the add closest to the cleric/casters first
2. Then mez remaining adds from highest level to lowest
3. Re-mez before expiration (build in 5-10s safety margin)
4. If mez breaks, stun with Color Flux then re-mez

### Mana Management

- Enchanter is often mana-neutral due to self-Clarity
- Mez is cheap; the expensive spells are Haste and Charm
- In emergencies, drop Haste to conserve mana for CC
- Sit and med between pulls

### Group Role

Crowd control is the #1 priority. One missed mez can wipe the group. Secondary: keep Haste on melee, Clarity on casters. Tertiary: Slow if no shaman. Charm-pet for DPS in safe situations.

### Automation Notes

- Mez requires target switching and priority logic (most complex automation target)
- Tash > Mez sequence should be atomic
- Haste buff tracking per group member
- Clarity buff tracking per caster
- The DLL now tracks successful **Enchanter** charm casts, detects a break when the former pet drops out of `MyPet` and shows back up as hostile, and immediately re-casts the resolved charm spell.
- Retryable re-charm failures (for example cooldown/pending-style outcomes) still use the normal cast retry policy; terminal failures such as resists/immunity are counted separately and stop after 3 attempts so the group can fall back to killing the mob.
- Current scope is the resolved `Charm` line in the DLL combat FSM; Druid/Necromancer animal/undead charm extensions still need explicit spell-line support before they get the same automation path.
- Color Flux (PBAE stun) is the emergency "everything broke" button

---

## 4. Shaman (Debuffer / Buffer / Off-Healer)

### Key Spells

**Slow Line (Attack Speed Debuff):**

| Spell            | Level | Slow% | Notes                                 |
| ---------------- | ----- | ----- | ------------------------------------- |
| Drowsy           | 9     | 20%   | First slow                            |
| Tepid Deeds      | 19    | 30%   | Early slow                            |
| Togor's Insects  | 24    | 40%   | Mid-level workhorse                   |
| Walking Sleep    | 34    | 50%   | Good slow                             |
| Turgur's Insects | 49    | 60%   | Best slow pre-Velious                 |
| Togor's Decay    | 55    | 68%   | Kunark top slow                       |
| Turgur's Swarm   | 60    | 75%   | Velious best slow (resist check hard) |

**Cannibalize Line (HP to Mana):**

| Spell           | Level | Notes                              |
| --------------- | ----- | ---------------------------------- |
| Cannibalize     | 23    | ~60 HP for ~20 mana (bad ratio)    |
| Cannibalize II  | 33    | Better ratio                       |
| Cannibalize III | 48    | Good ratio, primary canni spell    |
| Cannibalize IV  | 55    | Kunark; best HP-to-mana conversion |

**Healing:**

| Spell            | Level | Notes                                                            |
| ---------------- | ----- | ---------------------------------------------------------------- |
| Healing          | 9     | 135 HP                                                           |
| Greater Healing  | 19    | 280 HP                                                           |
| Superior Healing | 34    | 500 HP, primary group heal                                       |
| Torpor           | 52    | 300 HP/tick for 4-5 ticks + 30% slow on TARGET; best Kunark heal |
| Chloroblast      | 55    | ~1000 HP direct heal (Velious)                                   |

**Buffs:**

| Spell            | Level | Notes                         |
| ---------------- | ----- | ----------------------------- |
| Regen (line)     | 14+   | HP regeneration; keep on tank |
| Focus (line)     | 34+   | Stat buff (STA/STR/DEX/AGI)   |
| Haste (Alacrity) | 49    | Shaman haste; ~40%            |
| Primal Avatar    | 55    | +ATK/STR/DEX; melee DPS buff  |

### Combat Rotation Priority (Group)

1. **Slow** the current target immediately. The Live-safe automation treats slow as mandatory for every new target.
2. **Malo / Malos** only after a resisted slow, then retry slow. Avoid spending the debuff slot when slow already landed cleanly.
3. **Emergency heal** below 40% with **Torpor**, then use the direct-heal line for normal recovery below 70%.
4. **Cannibalize** only when mana is below 55% and the group is stable (everyone at or above roughly 80% HP).
5. **DOT** only after slow is secure, the target still has meaningful HP left, and mana is above 60%.
6. Do not re-cast slow, Malo, heals, Canni, or the active DOT early just because a gem is available; let the cooldown and target-state gates drive the next action.

### Live Automation Breakpoints

| Level | Slow / Debuff        | Heal Pair                    | Mana Tool        | Primary DOT                 | Key Buff Upgrade         |
| ----- | -------------------- | ---------------------------- | ---------------- | --------------------------- | ------------------------ |
| 60    | Turgur's Insects + Malo | Torpor + Chloroblast         | Cannibalize IV   | Ancient: Scourge of Nife    | Focus of the Sixth       |
| 61    | Turgur's Insects + Malo | Torpor + Chloroblast         | Cannibalize IV   | Cloud of Grummus            | Focus of the Sixth       |
| 62-64 | Turgur's Insects + Malo | Torpor + Tnarg's Mending     | Cannibalize IV   | Cloud of Grummus            | Focus of Soul            |
| 65+   | Turgur's Insects + Malos | Torpor + Quiescence          | Cannibalize IV   | Cloud of Grummus            | Focus of the Seventh     |

### Buff Priority (Pre-Combat)

1. Haste on melee DPS and tank
2. Focus (stat buff) on entire group
3. Regen on tank (and group if mana allows)
4. Primal Avatar on tank or top melee DPS

### Mana Management -- Canni Dancing

Canni dancing is the shaman's signature mana technique:

1. Cast Cannibalize (trades HP for mana)
2. Immediately sit down
3. Receive the meditation mana tick at the 6-second server tick
4. Stand up, cast Canni again
5. Repeat -- the key insight is you only need to be sitting at the exact moment of the server tick, not the full 6 seconds

Torpor synergy: Cast Torpor on self, then Canni. Torpor heals back the HP while Canni restores mana. Net result: sustained mana with no downtime.

### Group Role

Slow is the single most impactful debuff in the game. A slowed mob does 60-75% less melee damage, making the cleric's job trivially easier. Secondary: off-heal with Superior Healing / Chloroblast. Tertiary: buff group with Haste/Focus/Regen.

### Automation Notes

- Slow must land on every mob; if resisted, Malo then re-slow
- Canni loop is highly automatable (cast, sit, wait for tick, stand, repeat), but only when the group is already safe
- Torpor on tank is a "set and forget" HoT
- Buff tracking: Focus and Regen are the durable defaults; add Avatar-style melee buffs when a dedicated gem or clicky is available

---

## 5. Druid (Healer / Nuker / Utility)

### Key Spells

**Healing:**

| Spell            | Level | Heal  | Notes                                    |
| ---------------- | ----- | ----- | ---------------------------------------- |
| Minor Healing    | 1     | 12    |                                          |
| Healing          | 9     | 135   |                                          |
| Greater Healing  | 19    | 280   | Primary heal through 30s                 |
| Superior Healing | 29    | 500   | Workhorse heal for groups                |
| Nature's Touch   | 44    | ~1000 | Longer cast, more efficient              |
| Chloroblast      | 55    | ~1000 | Velious; faster cast than Nature's Touch |

**Nukes (Fire):**

| Spell          | Level | Damage | Notes             |
| -------------- | ----- | ------ | ----------------- |
| Burst of Fire  | 1     | 11     | Training nuke     |
| Burst of Flame | 14    | 90     |                   |
| Combust        | 19    | 165    |                   |
| Starfire       | 29    | 340    | Good fire nuke    |
| Firestrike     | 44    | 640    | Primary fire nuke |
| Scoriae        | 54    | ~900   | Kunark fire nuke  |

**Nukes (Cold):**

| Spell           | Level | Damage | Notes                     |
| --------------- | ----- | ------ | ------------------------- |
| Frost           | 5     | 30     |                           |
| Moonfire        | 39    | 480    |                           |
| Ice             | 49    | ~700   | Primary cold nuke for 50s |
| Pillar of Frost | 54    | ~600   | AoE; used for quad-kiting |

**DOTs:**

| Spell          | Level | Notes                      |
| -------------- | ----- | -------------------------- |
| Stinging Swarm | 14    | Insect DOT line            |
| Drifting Death | 34    | Upgraded DOT               |
| Winged Death   | 49    | Top DOT for Classic/Kunark |
| Drones of Doom | 54    | Kunark DOT upgrade         |

**Utility:**

| Spell             | Level | Notes                                       |
| ----------------- | ----- | ------------------------------------------- |
| Snare / Ensnare   | 9/19  | Movement speed debuff (critical for kiting) |
| Skin like Nature  | 39    | HP/AC buff + mana regen component           |
| Spirit of Wolf    | 14    | Run speed buff (group staple)               |
| Harmony of Nature | 39    | Lull spell for safe pulling                 |
| Evacuation        | 29    | Group evac to zone safe point               |
| Port spells       | 29-49 | Teleport group to various zones             |

**Port Spell Progression:**

- Level 19-20: Self-only ports (ring spells)
- Level 29: Group ports begin (Commonlands, Karana, etc.)
- Level 39-44: Most useful ports available
- Level 49: Full set of generic group ports
- Kunark: Wind of the South/North (Kunark zone evacs)

### Combat Rotation Priority (Group -- Off-Healer Role)

1. **Snare** the mob (if puller brings runners, snare prevents trains)
2. **Heal** the tank when cleric needs help (Superior Healing / Chloroblast)
3. **DOT** (Winged Death / Drones of Doom) for sustained damage
4. **Nuke** (Starfire / Firestrike) when mana is healthy
5. Avoid over-nuking -- druid aggro management is poor

### Combat Rotation Priority (Group -- DPS Role, No Cleric)

1. **Heal** is #1 priority if you are the only healer
2. **Snare** on pull
3. **DOT** for mana-efficient damage
4. **Nuke** only when tank is healthy and mana is above 50%

### Mana Management

- Druids have no Canni equivalent; rely on meditation and Clarity from enchanter
- DOTs are more mana-efficient than nukes; prefer DOTs for sustained damage
- Nukes burn mana fast; only nuke when mana pool is healthy or mob is nearly dead
- Skin line provides small mana regen component

### Group Role

Versatile support. Primary: heal if no cleric. Secondary: snare, DOTs, nukes. Utility: SoW, ports, evac (group safety net). Druids are the "Swiss army knife" -- good at many things, best at none.

### Automation Notes

- Snare on pull target is the first action
- Heal logic: if group has cleric, only heal when tank < 40% HP; if solo healer, heal at < 70%
- DOT once per mob (don't re-DOT unless fight is long)
- Evac macro for emergencies (one-button group safety)
- Port macro for travel between zones

---

## 6. Wizard (Burst DPS / Ports)

### Key Spells

**Nukes (Cold):**

| Spell           | Level | Damage | Mana | DPM  | Notes                |
| --------------- | ----- | ------ | ---- | ---- | -------------------- |
| Frost Bolt      | 12    | 72     | 40   | 1.8  |                      |
| Column of Frost | 20    | 175    | 100  | 1.75 |                      |
| Frost Strike    | 29    | 302    | 150  | 2.01 |                      |
| Ice Comet       | 44    | 710    | 250  | 2.84 | Best pre-Kunark nuke |
| Frost Rift      | 56    | ~900   | 300  | ~3.0 | Kunark               |

**Nukes (Fire):**

| Spell             | Level | Damage | Mana   | DPM    | Notes                        |
| ----------------- | ----- | ------ | ------ | ------ | ---------------------------- |
| Shock of Fire     | 4     | 20     | 15     | 1.33   |                              |
| Pillar of Fire    | 16    | 128    | 75     | 1.71   | AoE                          |
| Fire Bolt         | 16    | 142    | 75     | 1.89   |                              |
| Conflagration     | 29    | 350    | 160    | 2.19   |                              |
| Sunstrike         | 49    | 750    | 209    | 3.59   | Best DPM nuke in Classic     |
| Lure of Ice/Flame | 54-60 | varies | varies | varies | Lure spells bypass MR partly |

**Utility:**

| Spell       | Level | Notes                                |
| ----------- | ----- | ------------------------------------ |
| Gate        | 4     | Return to bind point                 |
| Port spells | 29-49 | Same ports as druid (group teleport) |
| Evacuate    | 34    | Group evac to zone safe point        |

**Velious Bane Spells:**

- Anti-dragon and anti-giant nukes with bonus damage against those mob types
- Available at 55-60; critical for Velious raid DPS

### Nuke Rotation Priority (Group DPS)

1. **Sunstrike** (best DPM; use as primary nuke for sustained damage)
2. **Ice Comet** (high burst when you need fast kills)
3. **Lure spells** (use against magic-resistant mobs)
4. **Bane nukes** in Velious against dragons/giants
5. Sit and meditate between casts

### Mana Management

Wizards are the most mana-dependent class. Key strategies:

- **DPM over DPS**: In groups, use high-DPM spells (Sunstrike) rather than highest-damage spells
- **Meditate aggressively**: Sit between every cast when possible
- **Mana-free clicky items**: Items with nuke click effects are a massive DPS increase (no mana cost)
- **Harvest/Concussion**: Later AAs; not available in Classic-Velious
- **Clarity II** from enchanter is essential
- Know when NOT to nuke: if the mob will die before your cast finishes, don't waste mana

### Group Role

Pure DPS. Wizards exist to kill things fast. Secondary: ports and evac for group safety. Wizards should NOT pull aggro -- if you pull aggro, you die. Watch the tank's aggro and throttle accordingly. Use Concussive Intuition (if available) or just stop nuking if threat is high.

### Automation Notes

- Simple rotation: Nuke > Sit > Wait for mana threshold > Nuke > Sit
- Aggro check: do not nuke if mob has < 20% HP and tank's aggro is shaky
- Mana threshold: do not nuke below 20% mana (reserve for evac/gate)
- Click mana-free nuke items on cooldown (these are free DPS)

### Live 60-65 Rotation Notes

- Level 60 sustained order: **Sunstrike** -> **Ice Comet** -> **Lure of Thunder** -> **Jyll's Wave of Heat** when 3+ enemies are stacked
- Level 61 adds **Harvest of Druzzil** as the mana recovery action; automation should trigger it around **35% mana**
- Level 62 upgrades the primary burst pair to **White Fire** and **Ancient: Destruction of Ice**
- Level 65 upgrades to **Fire of Tallon** and **Draught of E`ci** and swaps the emergency evac line to **Greater Decession**
- Live-safe reserve thresholds: keep **20% mana** in reserve for self-preservation and ports, and require roughly **45% mana** before spending casts on AoE
- Emergency logic should treat evac as a self-preservation tool, not part of the normal DPS loop

---

## 7. Monk (Puller / Melee DPS)

### Key Abilities

| Ability      | Level | Notes                                               |
| ------------ | ----- | --------------------------------------------------- |
| Round Kick   | 5     | Kick DPS skill; shares timer with Kick              |
| Feign Death  | 17    | Core pulling tool; success rate improves with level |
| Flying Kick  | 25    | Best kick DPS skill; shares timer with Round Kick   |
| Eagle Strike | 10    | Punch DPS skill                                     |
| Dragon Punch | 25    | Human monks; punch DPS (Tail Rake for Iksar)        |
| Tiger Claw   | 35    | Punch DPS upgrade                                   |
| Mend         | 1     | Self-heal; long cooldown                            |
| Safe Fall    | 3     | Passive; reduces fall damage                        |

### Disciplines

| Discipline  | Level | Duration | Cooldown | Notes                        |
| ----------- | ----- | -------- | -------- | ---------------------------- |
| Thunderkick | 52    | 1 min    | 10 min   | Increases Flying Kick damage |
| Voiddance   | 54    | 1 min    | 30 min   | Dodge all incoming attacks   |
| Innerflame  | 56    | 1 min    | 72 min   | Increases all melee damage   |
| Whirlwind   | 58    | 15s      | 30 min   | AoE melee attacks            |
| Stonestance | 51    | 1 min    | 15 min   | Increased defense            |

### Pulling Techniques

**Basic FD Pull:**

1. Run to camp of mobs
2. Tag one mob (melee or throwing weapon)
3. Run back toward group
4. Feign Death when near group
5. Mob either attacks the group (pulled) or resets (success)

**FD Splitting (Single Pull from Linked Mobs):**

1. Tag a mob in a group (will bring linked adds)
2. Run away from the mob camp toward group
3. Feign Death when adds are spread out
4. Wait for adds to path home (they walk back to spawn point)
5. Stand up when only one mob remains nearby
6. Bring the single mob to camp

**Tag Splitting (with Partner):**

1. Monk pulls group of mobs
2. Monk FDs
3. Second player (SK, ranger, etc.) tags one add
4. Monk stands, brings remaining mob
5. Used when mobs share bind point and won't separate naturally

**Velious FD Changes:**

- In Velious, mobs remember the monk much faster after FD
- Splitting is harder; mobs reaggro almost instantly
- Requires faster timing and sometimes multiple FD attempts
- Pulling in Velious often requires snare or root from another class

### Combat Rotation (DPS at Camp)

1. **Flying Kick** on cooldown (best kick skill)
2. **Tiger Claw** on cooldown (best punch skill)
3. Auto-attack with fists or weapons (H2H or 2H staff at high levels)
4. **Mend** self when below 50% HP

### Live Automation Priorities

1. **Emergency**: fire **Mend** below 50% HP, then fall through to **Voiddance / Earthwalk** below 35% HP, and reserve **Feign Death** for hard-break escapes when HP is critically low.
2. **Utility**: treat **Planeswalk** as a low-priority movement utility once it unlocks at 61, never ahead of survival tools or core burn discs.
3. **Burn**: use **Whirlwind** only on multi-enemy packs, then **Innerflame / Hundred Fists / Speed Focus** for primary burst, followed by **Thunderkick / Ashenhand** for special-attack amplification.
4. **Baseline melee**: keep the kick family (**Kick / Round Kick / Flying Kick**) and punch family (**Eagle Strike / Tiger Claw**) on shared timers so only the best unlocked skill fires.

### Level Breakpoints

- **60**: upgrade the kick-focus line from **Thunderkick Discipline** to **Ashenhand Discipline**.
- **61**: add **Planeswalk Discipline** as a utility option without displacing the main burn or emergency lines.
- **62**: keep the 61 rotation intact; this level is a tuning checkpoint, not a new line swap.
- **63-64**: replace **Hundred Fists Discipline** with **Speed Focus Discipline** for the primary attack-speed burn window.
- **65**: replace **Voiddance Discipline** with **Earthwalk Discipline** for the defensive line while retaining **Speed Focus** and **Planeswalk**.

### Mana Management

N/A -- Monks have no mana. All abilities are skill-based with timers.

### Endurance Management

- Hold burn disciplines when endurance is low so short-fuse utility or survival tools are not starved.
- Keep basic melee skills active down to a low endurance floor, but stop firing low-value activated abilities once endurance drops below the configured burn thresholds.

### Group Role

Primary puller. Use FD to split camps and deliver single mobs. Secondary: melee DPS (competitive with rogues). Monks are essential for dungeon crawling where controlled pulling prevents wipes.

### Automation Notes

- Pull routine: target mob > attack > run toward camp > FD > wait > stand
- FD timing is critical; too early = mob resets, too late = mob reaches group
- Flying Kick / Round Kick / Kick share one timer; Tiger Claw / Eagle Strike share another
- Mend at HP threshold
- Weight management: monks lose AC when carrying too much (keep weight low)

---

## 8. Bard (Buffs / Puller / CC / Jack-of-All-Trades)

### Key Songs

**Haste:**

| Song                          | Level | Haste%    | Notes                           |
| ----------------------------- | ----- | --------- | ------------------------------- |
| Anthem de Arms                | 10    | 10%       | First haste song                |
| McVaxius' Berserker Crescendo | 40    | 33%       | Primary melee haste             |
| Vilia's Verses of Celerity    | 49    | 40%       | Classic best haste              |
| Battlecry of the Vah Shir     | 55    | Overhaste | Breaks 100% haste cap (Velious) |

**Mana Regen:**

| Song                          | Level | Regen  | Notes                  |
| ----------------------------- | ----- | ------ | ---------------------- |
| Cassindra's Chorus of Clarity | 33    | 3/tick | Mana regen for casters |
| Cassindra's Chant of Clarity  | 47    | 5/tick | Upgrade                |

**Damage (Chants/DOTs):**

| Song             | Level | Notes                         |
| ---------------- | ----- | ----------------------------- |
| Chant of Battle  | 6     | DD proc on melee (group buff) |
| Chant of Flame   | 16    | Fire DOT on target            |
| Chant of Frost   | 26    | Cold DOT on target            |
| Chant of Disease | 36    | Disease DOT on target         |
| Chant of Poison  | 46    | Poison DOT on target          |

**Movement:**

| Song               | Level | Notes                                       |
| ------------------ | ----- | ------------------------------------------- |
| Selo's Accelerando | 5     | Run speed buff (fastest in game with drums) |

**Crowd Control:**

| Song                       | Level | Notes             |
| -------------------------- | ----- | ----------------- |
| Kelin's Lucid Lullaby      | 15    | Single target mez |
| Solon's Song of the Sirens | 30    | AoE mez           |

**Slow:**

| Song                    | Level | Slow% | Notes                 |
| ----------------------- | ----- | ----- | --------------------- |
| Largo's Melodic Binding | 20    | 25%   | Weak slow, but stacks |

**Resist Debuffs:**

| Song                   | Level | Notes                            |
| ---------------------- | ----- | -------------------------------- |
| Selo's Consonant Chain | 20    | -MR debuff (like Tash for bards) |

### Song Twisting Mechanics

- Bard songs have a 12-second duration and 3-second casting time
- You can maintain **4 songs** simultaneously by constantly cycling through them
- The `/melody` command automates this: `/melody 1 2 3 4` plays gems 1-2-3-4 in rotation
- DOT songs last 18 seconds (3 ticks), so you can twist **5 DOTs** if only running DOTs
- Songs are instant-on when recast before expiration (no gap in effect)

### Standard Twist Rotations

**Melee Group (default):**

1. Haste song (McVaxius/Vilia's/Battlecry)
2. Mana regen (Cassindra's Chant of Clarity)
3. HP regen or resist song
4. Chant of Battle (melee DD proc) or Selo's for movement

**Caster Group:**

1. Mana regen (Cassindra's)
2. Resist debuff on mob (Selo's Consonant Chain)
3. Damage DOT chant
4. HP regen or haste (some casters benefit from haste for procs)

**Pulling Twist:**

1. Selo's Accelerando (run speed to outrun mobs)
2. Snare song (if available; prevents runners)
3. Mez (Kelin's Lucid Lullaby for splitting)

### Mana Management

Bards use mana for songs but regenerate it while singing (unlike casters who must sit). Bard mana management is largely trivial -- songs cost little mana and regen is constant. The real "resource" is song slots and twist timing.

### Group Role

Force multiplier. Bard makes every group member better. Haste for melee, mana regen for casters, resist debuffs for nuke-heavy groups. Can off-tank, off-pull, mez adds in emergencies. Best puller in open zones (SoW speed + mez).

### Automation Notes

- `/melody` command is the automation foundation -- set it and forget it
- Swap songs based on group composition (melee vs. caster heavy)
- Pulling: automate Selo's + tag + run + mez add sequence
- Bard is one of the easiest classes to automate due to `/melody`

---

## 9. Necromancer (DOTs / Pet / Utility)

### Key Spells

**DOT Line (Disease):**

| Spell         | Level | Damage     | Mana | DPM  | Notes                            |
| ------------- | ----- | ---------- | ---- | ---- | -------------------------------- |
| Disease Cloud | 1     | 36 total   | 20   | 1.8  |                                  |
| Heart Flutter | 12    | 228 total  | 60   | 3.8  |                                  |
| Scourge       | 24    | 540 total  | 120  | 4.5  |                                  |
| Plague        | 44    | ~850 total | 200  | ~4.3 |                                  |
| Splurt        | 53    | 1616 total | 237  | 6.82 | Escalating DOT; best DPM in game |

**DOT Line (Poison):**

| Spell              | Level | Damage     | Mana | DPM  | Notes |
| ------------------ | ----- | ---------- | ---- | ---- | ----- |
| Poison Bolt        | 1     | 32 total   | 15   | 2.1  |       |
| Venom of the Snake | 34    | 456 total  | 160  | 2.85 |       |
| Envenomed Bolt     | 49    | ~700 total | 200  | ~3.5 |       |

**DOT Line (Fire):**

| Spell        | Level | Damage      | Mana | DPM  | Notes                        |
| ------------ | ----- | ----------- | ---- | ---- | ---------------------------- |
| Ignite Blood | 49    | 1008 total  | 250  | 4.03 | 18 ticks; very long duration |
| Pyrocruor    | 56    | ~1400 total | 300  | ~4.7 | Kunark upgrade               |

**Lifetaps:**

| Spell         | Level | Notes                               |
| ------------- | ----- | ----------------------------------- |
| Lifetap       | 8     | Drains HP from target, heals caster |
| Siphon Life   | 24    | Upgrade                             |
| Bond of Death | 49    | 720 damage, heals necro; 360 mana   |
| Drain Soul    | 55    | Kunark top lifetap                  |

**Lich Line (Mana Regen):**

| Spell         | Level | HP Cost/tick | Mana Gain/tick | Notes                   |
| ------------- | ----- | ------------ | -------------- | ----------------------- |
| Dark Pact     | 29    | ~20          | ~10            | First lich              |
| Call of Bones | 39    | ~30          | ~16            | Turns you into skeleton |
| Lich          | 49    | ~45          | ~22            | Primary lich for 50s    |
| Demi-Lich     | 56    | ~60          | ~30            | Kunark; massive regen   |

**Utility:**

| Spell             | Level | Notes                                        |
| ----------------- | ----- | -------------------------------------------- |
| Feign Death       | 16    | FD for aggro dumps                           |
| Summon Corpse     | 39    | Pull corpses to camp (critical raid utility) |
| Dead Man Floating | 34    | Underwater breathing + levitate              |
| Screaming Terror  | 55    | Fear (can be used for kiting)                |

### DOT Rotation Priority (Group)

The current Live-safe automation path is intentionally conservative and only
loads long-duration DoTs while the mob is still healthy enough to pay back the
mana cost.

1. **Scent of Terris** at the top of the pull (resist debuff; only while target HP is near full)
2. **Splurt** / **Dark Plague** (disease line; `Dark Plague` takes over at 61)
3. **Funeral Pyre of Kelador** / **Night Fire** (fire line; `Night Fire` takes over at 65)
4. **Legacy of Zek** / **Blood of Thule** when available (poison line; enabled from 62, upgraded at 65)
5. **Send Pet** to attack
6. **Touch** lifetap line only after the target is partly burned down or when HP drops into the emergency band
7. **Death Peace** when HP is critical and a lifetap would be too risky or too expensive

### DOT Stacking Rules

- You can stack multiple DOT lines simultaneously (disease + poison + fire)
- Only one DOT per line active at a time (e.g., don't cast Plague AND Splurt -- both disease)
- Splurt replaces lower disease DOTs
- Watch aggro: full DOT stack generates enormous hate over time
- FD to dump aggro after loading DOTs

### Mana Management -- Lich Mechanic

- Keep Lich/Demi-Lich running at all times (converts HP to mana)
- Lifetap to recover HP lost from Lich
- Net effect: necromancers have near-infinite mana if they can lifetap
- In groups with a healer, ask healer to toss occasional heals to offset Lich HP drain
- Canni-dance equivalent: Lich + Lifetap loop

### Group Role

Sustained DPS through DOTs. Pet provides additional damage. Utility: summon corpse (essential for raids), FD for aggro management. Necromancers excel in long fights where DOTs can tick to full duration. In short fights, much of their damage is wasted on corpses.

### Automation Notes

- Runtime rotation order is `ResistDebuff -> DiseaseDot -> FireDot -> PoisonDot`, with each DoT line gated by target HP and a minimum mana reserve.
- Emergency behavior is split from the sustained rotation: lifetap at roughly the 45% self-HP band, then `Death Peace` as the critical-hp bailout.
- Level tuning currently has explicit shipped profiles for **60**, **61**, **62-64**, and **65** so the operator-facing class config matches the DLL ability-line resolution.
- Pet management is limited to `/pet attack` in the combat loop; self-buff and pet-buff lines remain documented in the class config for operator slot planning.

---

## 10. Magician (Pet / Nukes)

### Key Spells

**Pet Summoning (Water -- Best DPS Pet):**

| Spell                      | Level | Notes                           |
| -------------------------- | ----- | ------------------------------- |
| Elemental: Water           | 1     | First water pet                 |
| Lesser Conjuration: Water  | 8     |                                 |
| Conjuration: Water         | 16    |                                 |
| Greater Conjuration: Water | 24    |                                 |
| Vocaration: Water          | 34    |                                 |
| Greater Vocaration: Water  | 44    | Primary pet through Classic     |
| Manifest Element: Water    | 49    | Top Classic pet                 |
| Summon Servant of Water    | 56    | Kunark pet                      |
| Greater Servant of Water   | 60    | Velious pet (or Epic pet at 46) |

**Pet Types:**

| Element | Proc/Special          | Best Use                                    |
| ------- | --------------------- | ------------------------------------------- |
| Water   | DD proc, backstab 51+ | **Best DPS pet** in Classic; default choice |
| Earth   | Root proc (18s)       | Off-tanking; root holds mob in place        |
| Air     | Stun proc             | Stun-locking; useful for interrupts         |
| Fire    | Fire DD proc + DS     | Damage shield; less popular                 |

**Pet Gear (Summoned Items):**

| Spell               | Level  | Notes                                 |
| ------------------- | ------ | ------------------------------------- |
| Summon Heatstone    | varies | Pet weapons (give to pet for +damage) |
| Burnout (line)      | 16+    | Pet haste/damage buff                 |
| Shield of Fire/Lava | varies | Summoned shield for pet AC            |

**Nukes:**

| Spell                   | Level  | Damage | Notes                                    |
| ----------------------- | ------ | ------ | ---------------------------------------- |
| Burn                    | 1      | 8      | First nuke                               |
| Shock of Blades         | 20     | 140    | Magic-based nuke                         |
| Spear of Warding        | 39     | 340    | Good nuke                                |
| Shock of Swords         | 44     | 456    | Primary nuke                             |
| Seeking Flame of Seukor | 56     | ~700   | Kunark nuke                              |
| Mala (Rain spells)      | varies | AoE    | Rain nukes; 4 hits but pet absorbs a hit |

**Utility:**

| Spell             | Level  | Notes                                              |
| ----------------- | ------ | -------------------------------------------------- |
| Summon Food/Water | varies | Conjure supplies for group                         |
| Call of the Hero  | 52     | Summon a player to you (Kunark; HUGE raid utility) |
| Mod Rods (Wand)   | 34     | Summoned item that converts HP to mana for others  |
| Monster Summoning | 29+    | Summon a mob to your location                      |

### Combat Rotation Priority (Group)

1. **Summon pet** before group starts (water pet default)
2. **Gear pet**: give summoned weapons and shield, cast Burnout (pet haste/damage)
3. **Send pet** to attack (pet is primary DPS source)
4. **Nuke** with Shock of Swords / Seeking Flame (secondary DPS)
5. **Pet heal** if pet is taking damage (pet heals are weak pre-Luclin; earth pet can tank light hits)
6. **Re-summon pet** if pet dies (keep reagents stocked)
7. **Rain nukes** only on stationary mobs (AoE; risk of breaking mez)

### Pet Management

- **Always haste your pet** (Burnout line) -- this is the single biggest DPS increase
- **Give pet summoned weapons** -- pets gain significant damage from equipped items
- Water pet backstabs from level 51+; position it behind the mob if possible
- **Pet health monitoring**: pet HP is displayed in the pet window; if pet is about to die, send it away and re-summon
- **Epic pet** (Magician 1.0 Epic): far stronger than any other pet pre-PoP; get ASAP in Kunark
- AoE note: if your pet is inside the AoE of your Rain spell, it absorbs one of the 4 hits (Classic-Velious bug)

### Mana Management

- Magicians are mana-hungry; nuking + pet management is expensive
- **Mod Rods**: summon these for other casters (converts their HP to mana); good group utility
- **Clarity** from enchanter is critical
- Pet does DPS for free (no mana cost once summoned); lean on pet damage in long fights
- Nuke less, pet more = better mana efficiency

### Group Role

Pet class DPS. Pet provides consistent melee damage. Supplemental nuking. Utility: Call of the Hero (summon players), Mod Rods for caster mana, summoned food/water. Magician damage is split between pet and nukes; automating both is key.

### Automation Notes

- Pet summon + equip + buff is a startup macro (do once per session or on pet death)
- Pet attack on current target when tank establishes aggro
- Nuke on cooldown (with mana threshold check)
- Pet recall if pet is about to die
- Mod Rod distribution to casters between pulls

---

## 11. Beastlord (Melee DPS / Warder Support)

### Key Spells and Disciplines

| Ability / Spell         | Level | Notes |
| ----------------------- | ----- | ----- |
| Kick                    | 1     | Baseline melee filler when spell DPS is gated |
| Bestial Fury Discipline | 60    | Primary burn discipline for healthy targets |
| Sha's Advantage         | 60    | Opening slow for the 60-64 profile |
| Scorpion Venom          | 61    | Poison DoT; expensive enough to reserve for mana-positive fights |
| Healing of Sorsha       | 61    | Pet heal spell; tracked as an available line, not auto-fired in combat yet |
| Infusion of Spirit      | 61    | Strong single-target melee buff for group members |
| Spiritual Vigor         | 62    | Group HP/attack buff upgrade |
| Talisman of Kragg       | 62    | HP/stat buff upgrade for group members |
| Arag's Celerity         | 63    | Warder haste/attack buff; held out of active combat rotation for now |
| Ferocity                | 65    | Best single-target melee stat buff in this bracket |
| Sha's Revenge           | 65    | Final slow upgrade in the 65+ profile |
| Trushar's Frost         | 65    | Direct-damage nuke for mana-positive fights |
| Trushar's Mending       | 65    | Emergency self-heal when HP falls under the configured floor |

### Combat Priority

1. **Emergency self-heal** if HP falls below 40% and mana is still healthy
2. **Sha's slow** at the opener while the target is still near full HP
3. **Bestial Fury Discipline** on durable targets
4. **Spell DPS** with Scorpion Venom at 61-64 or Trushar's Frost at 65+ when mana is comfortably above the reserve threshold
5. **Kick** as the low-cost fallback
6. Keep the **warder attacking** the current assist target

### Mana and Endurance Management

- Reserve mana for the opener slow first
- Skip spell DPS once mana drops under roughly 45%
- Fall back to kick and warder damage instead of spending the last mana on low-value casts
- Bestial Fury is modeled as a burn button and should not crowd out the opener slow

### Buff and Utility Notes

- Group-safe buff maintenance lives in `config/classes/beastlord.toml`
- The 60 profile uses **Savagery** and **Spiritual Strength**
- The 61 profile adds **Infusion of Spirit**
- The 62 profile upgrades to **Spiritual Vigor** and **Talisman of Kragg**
- The 65 profile upgrades the melee buff line to **Ferocity**
- Warder-only buffs and warder heals are tracked as known Beastlord lines, but active combat automation does not try to maintain them because the current combat context does not expose pet HP or pet buff state

### Group Role

Secondary slow support plus steady melee/pet DPS. Beastlords should not be treated like primary healers or CC casters in this era; their value is opener control, melee buffs, and clean damage once the pull is stable.

### Automation Notes

- Slow is modeled as an opener-only action because the DLL does not yet track target debuff state
- Beastlord spell DPS upgrades from **Scorpion Venom** at 61-64 to **Trushar's Frost** at 65, with both lines held behind the mana reserve threshold
- Optional resist-debuff clickies remain operator-specific and belong in per-toon overrides rather than as hard-coded runtime item assumptions
## 16. Paladin (Hybrid Tank / Off-Healer)

### Live-Safe Rotation Priorities

1. **Emergency heal** when a group member drops below `35%`
2. **Group heal** when `2+` members are at or below `55%`
3. **Cure** only when the group is otherwise stable above the `55%` floor
4. **Support heal** for sustained damage up to `60%`
5. **Stun** for pickup control and interrupts when mana is above the support reserve
6. **Holy nuke** only when no heal/cure/stun action is needed and mana is above the DPS floor
7. **Rebuff self** out of combat with Yaulp / HP buffs

### Level Tuning Breakpoints

| Level | Main change |
| ----- | ----------- |
| 60 | `Superior Healing`, `Healing Wave of Prexus`, `Brell's Mountainous Barrier`, `Yaulp IV` |
| 61 | `Touch of Nife` replaces the older single-target heal line |
| 62 | `Crusader's Touch` becomes the preferred cure and `Force of Akilae` replaces `Force` |
| 63 | `Light of Nife` and `Pious Might` become the main heal / DPS lines |
| 64 | `Quellious' Word of Serenity` and `Supernal Cleansing` upgrade the stun / cure package |
| 65 | `Wave of Marr` and `Brell's Stalwart Shield` finish the Live-safe group profile |

### Resource Floors

- Hold **at least `15%` endurance** before spending Paladin melee skills; this preserves pickup tools during long fights.
- Reserve **`22%` mana** before using stun as a low-value control action.
- Reserve **`40%` mana** before casting the DPS nuke line.
- Only spend cure mana when the group is already stable; healing beats cure when the HP floor is collapsing.
- Cancel long heals if the group has recovered above **`82%`** before the cast lands.

### Group Role

Secondary tank and stabilization support. The Paladin should not be treated like a primary nuker; unattended value comes from keeping bad pulls recoverable through pickup aggro, stuns, off-heals, and selective cure coverage.

### Automation Notes

- Keep the heal package conservative: Paladins bridge damage spikes so clerics and shamans can stay on their primary jobs.
- Use the stun line as an **interrupt / pickup** tool, not as a spammed DPS button.
- Prefer single-target heals for true emergencies; only pivot to the group heal line when multiple members are meaningfully low.
- Rebuff Yaulp and HP buffs only out of combat.

---

## Cross-Class Group Composition Notes

### Automation-First 6-Person Group (Velious)

| Slot | Class             | Role |
| ---- | ----------------- | ---- |
| 1    | Tank              | Warrior, shadowknight, or paladin holds aggro and anchors positioning |
| 2    | Cleric            | Primary healer, rez anchor, and worst-case recovery backbone |
| 3    | Bard              | Passive resist and mana floor, movement control, peel/mez backup |
| 4    | Shaman or Paladin | Preferred second healer (`SHM`) or pickup-and-heal fallback (`PAL`) |
| 5    | Monk              | Puller and primary melee DPS |
| 6    | DPS or CC flex    | Second monk for farm speed, or enchanter / caster utility if the camp requires it |

TextQuest's unattended default is intentionally more conservative than a manual-boxer "ideal" group. The automation target is a group that can survive surprises, not just win clean pulls faster.

### Survivability Core and DPS Tradeoff

The main composition trade is whether slot 4 becomes a second healer/support-healer or a third real damage/CC slot.

For design budgeting, use an effective-DPS-slot model instead of pretending we have live parse certainty:

- `tank ~= 0.3 DPS slot` while holding aggro
- `bard ~= 0.3 DPS slot` while staying in support melody
- `shaman/paladin second healer ~= 0.4-0.6 DPS slot` while healing, slowing, stunning, or picking up

| Layout | Effective DPS slot budget | Clean-fight pace | Recovery margin |
| ------ | ------------------------- | ---------------- | --------------- |
| `Tank / CLR / BRD / SHM|PAL / DPS / DPS` | about `3.0-3.2` | roughly `15-25%` slower than a DPS-first layout | high |
| `Tank / CLR / BRD / DPS|CC / DPS / DPS` | about `3.6-3.8` | faster on clean pulls | medium to low |

That loss is acceptable for TextQuest because automation cannot make human-speed judgment calls. The second healer/support slot is there to prevent a single resist, lag spike, or bad path from cascading into a wipe.

### Pull-to-Kill Sequence (Automated)

1. **Bard** starts the defensive melody package before the pull lands so mana, resist, and movement coverage are already online.
2. **Monk** or the configured puller brings a single mob via split tools.
3. **Tank** establishes aggro and locks the mob in camp geometry.
4. **Shaman or Paladin** applies the survivability layer first: slow if present, then off-heal or pickup support as needed.
5. **Cleric** owns the tank HP floor and keeps rez/recovery priority above any offensive cast.
6. **Flex DPS/CC** joins only after the mob is stable. If an add shows up, that slot pivots to control rather than greedily finishing the kill target.
7. **Bard or CC flex** isolates the first unexpected add. If control fails, the pickup class takes the add and the group drops into stabilization instead of burn.
8. Kill the target only after the add state is clean or the group has switched to an orderly disengage.

### Multi-Group Coordination (36-Box)

With 36 characters across 6 groups:

- Each group operates the above sequence independently, but unattended groups are expected to preserve the `BRD + CLR + second healer/support-healer` floor whenever the content is unstable.
- **CH Chain** across groups for raid content (3-5 clerics rotating)
- **Bard** in each group for passive buffs (set `/melody` and forget)
- **Call of the Hero** (magician) for rapid group assembly
- **Ports** (druid/wizard) for zone-to-zone movement

---

## 8. Ranger (Hybrid DPS / Utility Puller)

### Live-Safe Rotation Bands

- **Level 60**: maintain `Call of the Predator`, use `Calefaction` as the primary nuke, `Immolate` only on healthier targets, and reserve `Weapon Shield Discipline` for the emergency bucket.
- **Level 61**: add `Circle of Winter` as the cold nuke line without changing the Trueshot / Weapon Shield discipline pairing.
- **Level 62-63**: upgrade the self-buff and proc package to `Strength of Tunare` plus `Call of the Rathe`, swap the dot line to `Drifting Death`, and move emergency healing to `Chloroblast`.
- **Level 64**: keep the 62-era DPS package but promote the debuff line to `Nature's Rebuke`.
- **Level 65+**: promote `Sylvan Burn` to the primary nuke slot and `Natureskin` to the main downtime buff while keeping the 62-era proc and dot lines.

### Rotation Priority

1. **Downtime**: refresh `SelfBuff` before pulls, then `ProcBuff` when mana permits.
2. **Emergency**: cast the emergency heal first below the HP floor; fall through to `Weapon Shield Discipline` only when the heal path is insufficient.
3. **Debuff**: apply the snare / debuff line early on stable, healthy targets.
4. **Burn**: fire `Trueshot Discipline` only on long-lived targets with enough endurance to justify the shared discipline lockout.
5. **Combat**: `Drifting Death` on durable targets, then the primary nuke, then `Circle of Winter`, and finally `Kick` inside melee range when endurance is still healthy.

### Resource Rules

- Spell casts are gated so Rangers stop spending mana on low-value nukes before they starve the emergency heal or downtime buff refresh.
- `Kick` stays behind an endurance threshold so melee utility does not crowd out the long-lockout discipline buttons.
- `Trueshot Discipline` and `Weapon Shield Discipline` should be treated as sharing the Ranger discipline lockout; once one fires, the other is unavailable until the reuse window expires.

---

## Automation Priority Summary

Classes ranked by automation complexity (simplest to hardest):

1. **Bard** -- `/melody` handles 90% of the work
2. **Warrior** -- Taunt + Bash + Kick on timers
3. **Wizard** -- Nuke > Sit > Repeat
4. **Magician** -- Pet attack + Nuke loop
5. **Necromancer** -- DOT stack + FD + Lich loop
6. **Monk** -- Pulling logic is complex but combat is simple
7. **Druid** -- Heal/nuke priority switching
8. **Shaman** -- Slow + Canni + Heal juggling
9. **Cleric** -- Heal timing is critical; CH chain coordination
10. **Enchanter** -- CC requires real-time target prioritization and mez tracking

---

_Sources: Project 1999 Wiki, EQProgression, Allakhazam, EQ Classic Wiki, Magician's Tower, community guides_
_Compiled for Frostreaver automation reference_
