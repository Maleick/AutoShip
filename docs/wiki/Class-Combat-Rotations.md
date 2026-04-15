# EverQuest Class Combat Rotations: Classic / Kunark / Velious

Reference for Frostreaver automation. Covers the 10 classes relevant to the 36-box TLP setup.
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

1. **Slow** the current target (highest priority -- reduces damage by 60-75%)
2. **Malo/Malosini** magic resist debuff if slow is being resisted
3. **Canni** between casts to regenerate mana
4. **Heal** the tank with Superior Healing or Torpor
5. **DOT** (Envenomed Bolt / Bane of Nife) if mana allows
6. **Re-slow** if it wears off mid-fight

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
- Canni loop is highly automatable (cast, sit, wait for tick, stand, repeat)
- Torpor on tank is a "set and forget" HoT
- Buff tracking: Haste, Focus, Regen on all group members

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

### Mana Management

N/A -- Monks have no mana. All abilities are skill-based with timers.

### Group Role

Primary puller. Use FD to split camps and deliver single mobs. Secondary: melee DPS (competitive with rogues). Monks are essential for dungeon crawling where controlled pulling prevents wipes.

### Automation Notes

- Pull routine: target mob > attack > run toward camp > FD > wait > stand
- FD timing is critical; too early = mob resets, too late = mob reaches group
- Flying Kick and Tiger Claw on cooldown timers
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

1. **Splurt** (best DPM in game; cast first, it escalates each tick)
2. **Ignite Blood** / Pyrocruor (fire DOT; long duration, high total damage)
3. **Plague** or disease DOT (separate resist check from fire)
4. **Envenomed Bolt** (poison DOT; stacks with above)
5. **Send Pet** to attack
6. **Lifetap** (Bond of Death) if you need self-healing or mob is near 60% HP
7. **Feign Death** if aggro gets too high

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

- DOT loading sequence: Splurt > Fire DOT > Poison DOT > Pet attack
- FD aggro dump after DOT loading
- Lich always on; lifetap when HP drops below 60%
- Pet management: send pet, heal pet if needed (pet heals are bad pre-Luclin)
- Summon Corpse macro for raids

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

## Cross-Class Group Composition Notes

### Ideal 6-Person Group (Velious)

| Slot | Class      | Role                                     |
| ---- | ---------- | ---------------------------------------- |
| 1    | Warrior    | Main tank; hold aggro                    |
| 2    | Cleric     | Main healer; keep tank alive             |
| 3    | Enchanter  | CC; haste tank; clarity casters          |
| 4    | Shaman     | Slow mob; off-heal; buffs                |
| 5    | Monk       | Pull; melee DPS                          |
| 6    | DPS (flex) | Wizard/Necro/Mage/Bard depending on zone |

### Pull-to-Kill Sequence (Automated)

1. **Monk** pulls single mob via FD split
2. **Warrior** taunts and establishes aggro
3. **Enchanter** Tash + Mez any adds
4. **Shaman** Slow the kill target
5. **Cleric** heals warrior (Superior Healing / CH)
6. **DPS** classes engage (nukes, DOTs, pets)
7. **Enchanter** re-mez adds as needed
8. Kill mob; move to next add or next pull

### Multi-Group Coordination (36-Box)

With 36 characters across 6 groups:

- Each group operates the above sequence independently
- **CH Chain** across groups for raid content (3-5 clerics rotating)
- **Bard** in each group for passive buffs (set `/melody` and forget)
- **Call of the Hero** (magician) for rapid group assembly
- **Ports** (druid/wizard) for zone-to-zone movement

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
