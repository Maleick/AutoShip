# MQ2 Plugin API Quick Reference

**Issue:** #1012  
**Purpose:** Quick lookup table for plugin commands, TLOs, and INI settings

---

## 1. MQ2Melee

### Entry Point

```
/melee [subcommand] [options]
```

### Key Subcommands

| Command | Purpose | Example |
|---------|---------|---------|
| `on` | Enable melee rotation | `/melee on` |
| `off` | Disable rotation | `/melee off` |
| `reload` | Reload INI config | `/melee reload` |
| `aggro on\|off` | Set tank/DPS mode | `/melee aggro on` |
| `petassist on\|off` | Pet engagement | `/melee petassist on` |

### TLO: `${Melee}`

```
${Melee.Active}           → bool
${Melee.Engaged}          → bool
${Melee.SwingHits}        → uint
${Melee.TakenHits}        → uint
```

### INI File

```
Location: {MQConfig}/{ServerShortName}_{CharName}.ini
Section: [MQ2Melee]

Key Examples:
plugin=1                   (enable/disable)
melee=1                    (enable melee mode)
aggro=1                    (for WAR/PAL/SHD)
petassist=1                (for pet classes)
feigndeath=30              (HP% to feign)
jolt=5                     (every N hits)
holyshit0=jolt             (special attack command)
stickmode=0                (0=built-in, 1=custom, 2=off)
```

### Key Dependencies

- **Requires:** MQ2MoveUtils (for stick positioning)
- **Optional:** MQ2Cast (for spell casting in rotation)

### Critical Values (EQ Internals)

- **Attack Range:** < avatar_height + 12 units
- **Behind Arc:** 45-degree sector
- **Aggro Cap:** Target < 250 units (hard coded)

---

## 2. MQ2Cast

### Entry Point

```
/casting <spell_name> [options]
```

### Options

| Flag | Purpose | Example |
|------|---------|---------|
| `-maxtries\|N` | Retry on failure | `/casting "heal" -maxtries\|3` |
| `-kill` | Keep casting until target dies | `/casting "damage" -kill` |
| `-targetid\|ID` | Cast on specific spawn | `/casting "root" -targetid\|1234` |
| `-invis` | Abort if invisible | `/casting "damage" -invis` |

### Alternate Subcommands

```
/memorize "spell name" gem1           (Memorize to slot 1)
/memorize "spell name" gem2 gem3      (Multiple slots)
/interrupt                             (Stop current cast)
/castdebug on|off                      (Toggle debug output)
/sss "myspellset"                      (Save spell set)
/ssm "myspellset"                      (Load spell set)
```

### TLO: `${Cast}`

```
${Cast.Active}            → bool (currently casting?)
${Cast.Effect}            → spell object
${Cast.Result}            → string (CAST_SUCCESS, CAST_FIZZLE, etc.)
${Cast.Timing}            → int (ms remaining)
${Cast.Status}            → string (C=casting, S=stopping, M=memorizing, I=idle)
${Cast.Ready[spellname]}  → bool
```

### Cast Result Codes (21 states)

```
CAST_SUCCESS(0)           Spell cast successfully
CAST_INTERRUPTED(1)       Interrupted mid-cast
CAST_RESIST(2)            Target resisted
CAST_FIZZLE(5)            Spell fizzled
CAST_STANDING(6)          Must be standing
CAST_STUNNED(7)           Character stunned
CAST_INVISIBLE(8)         Invisible (spell breaks it)
CAST_OUTOFMANA(10)        Not enough mana
CAST_OUTOFRANGE(11)       Out of range
CAST_NOTARGET(12)         No valid target
CAST_IMMUNE(17)           Target immune
```

### INI File

```
Location: {MQConfig}/{ServerShortName}_{CharName}.ini
Section: [MQ2Cast(SpellSet)]

# Spell sets: "setname = spell1|gem spell2|gem"
healing = 1234|1 5678|2

Section: [Settings]
Normal                    (bard non-beta casting)
```

### Dependencies

- **Optional:** MQ2Bandolier (for focus swaps)
- **Optional:** MQ2Twist (for bard detection)
- **Integrates:** MQ2MoveUtils (pause during cast)

### Key Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `DELAY_CAST` | 16000ms | Max time for single attempt |
| `DELAY_STOP` | 4000ms | Max immobilization time |
| `DELAY_PULSE` | 125ms | Pulse interval |

---

## 3. MQ2MoveUtils

### Entry Point

```
/stick [distance] [position]
/moveto loc <y> <x> [z]
/circle on [radius]
/makecamp on|off [radius]
```

### Stick Positions

| Position | Arc | Usage |
|----------|-----|-------|
| (default) | - | Melee range (avatar_height * 1.0) |
| `behind` | 45° | Position behind mob |
| `front` | 240° | In front of mob |
| `!front` | 135° | Anywhere but front |
| `pin` | 112-144° | Side positioning |
| `hold` | - | Persist on target switch |
| `always` | - | Auto-restick new targets |

### TLO: `${Stick}`

```
${Stick.Active}           → bool
${Stick.Distance}         → float (current distance to target)
${Stick.Behind}           → bool
${Stick.InRange}          → bool
${Stick.Broken}           → bool (reason)
${Stick.ID}               → uint (target spawn ID)
```

### INI File

```
Location: MQ2MoveUtils.ini
Section: [Defaults]

AllowMove=32.0            (heading tolerance before forward movement)
AutoPause=true            (pause when casting/stunned)
Heading=true              (fast|loose|true)
TurnRate=14.0             (loose heading turn speed)

Section: [Stick]
ArcBehind=45.0
DelayStrafe=true
BreakOnGate=true
UseBackward=true

Section: [MakeCamp]
CampRadius=40.0
MinDelay=500
MaxDelay=1500
ReturnNoAggro=false
```

### Heading System

```
0-512 scale (NOT degrees)
256 = 180 degrees (opposite direction)
128 = 90 degrees
```

### Key Algorithms

```
Distance calc:  |target_pos - self_pos|
Heading calc:   atan2(target.X - self.X, target.Y - self.Y) * 256/π
Behind arc:     |target_heading - self_heading| < 45
Anti-orbit:     Block fwd if |heading_diff| > AllowMove
```

### Dependencies

- **None** (standalone movement system)
- **Used by:** MQ2Melee (stick from combat), MQ2Cast (pause during cast)

---

## 4. MQ2AutoLoot

### Entry Point

```
/autoloot [subcommand]
/setitem <action> [param]
```

### Subcommands

| Command | Purpose |
|---------|---------|
| `turn on\|off` | Enable/disable |
| `spamloot on\|off` | Chat spam |
| `raidloot on\|off` | Raid automation |
| `sell` | Sell marked items |
| `buy "item" #` | Purchase from merchant |
| `deposit` | Bank marked items |
| `barter` | Barter marked items |
| `reload` | Reload INI |

### SetItem Actions

| Action | Description |
|--------|-------------|
| `Keep` | Loot and keep (can bank later) |
| `Sell` | Loot and sell to merchant |
| `Deposit` | Loot and deposit to bank |
| `Barter\|MINPRICE` | Loot and barter if offer >= min plat |
| `Quest\|COUNT` | Loot up to N per character |
| `Gear\|CLASSES\|WAR\|PAL\|...\|N\|` | Auto-class-check gear |
| `Ignore` | Never loot |
| `Destroy` | Master looter destroys |

### TLO: `${AutoLoot}`

```
${AutoLoot.Active}        → bool
${AutoLoot.SellActive}    → bool (thread running)
${AutoLoot.BuyActive}     → bool
${AutoLoot.DepositActive} → bool
${AutoLoot.BarterActive}  → bool
${AutoLoot.FreeInventory} → int (slots)
```

### INI File

```
Location: Loot.ini
Section: [Settings]

UseAutoLoot=1
LogLoot=0
RaidLoot=0
DistributeLootDelay=5
NewItemDelay=0
CursorDelay=10
SaveBagSlots=0
QuestKeep=10
NoDropDefault=Quest      (Quest|Keep|Ignore)
ExcludeBag1=Extraplanar Trade Satchel

Section: [A]              (first-letter sections)
Adamantite Ore=Sell
Aegis=Keep
Ancient Coin=Barter|500

Section: [Z]
Zombie Heart=Ignore
```

### Master Looter Logic

1. Detect if you're master looter
2. Loot item
3. Wait `DistributeLootDelay` seconds
4. Offer to group/raid members
5. If nobody wants → distribute by need priority
6. If still nobody → leave on corpse

### Dependencies

- **Optional:** MQ2EQBC (trade acceptance from boxed chars)
- **Optional:** MQ2MoveUtils (navigate to merchants)

---

## 5. MQ2EQBC (EverQuest Box Chat)

### Entry Point

```
/bccmd connect <server> <port> <password>
/bc <message>                (broadcast to all)
/bca <command>               (command to all except self)
/bct <toon> <message>        (direct to toon)
/bcg <message>               (group members)
```

### Message Format

```
/bc "My message"
/bct MyAlt "/casting heal"   (execute command on alt)
/bca "/melee reload"         (broadcast command)
/bcg "group status"
/bcz "zone message"          (zone members)
```

### Silent Variants

```
/bcsa                        (silent /bca)
/bcsaa                       (silent /bcaa)
/bcsg                        (silent /bcg)
```

### BCI (Box Client Interface) Response

On `/bccmd names`, each connected client responds with pipe-delimited fields:

```
HP|MaxHP|Mana|MaxMana|End|MaxEnd|Zone|Level|Class|Race|Exp|AAExp|AAPoints|StandState|Pet|ZoneShort|X|Y|Z|TargetName|TargetLevel|TargetHP|TargetMaxHP|InGroup|GroupLeader
```

### TLO: `${EQBC}`

```
${EQBC.Connected}         → bool
${EQBC.Server}            → string
${EQBC.Port}              → int
${EQBC.ToonName}          → string
${EQBC.Names}             → string (space-sep list)
${EQBC.GotNames}          → bool
```

### INI File

```
Location: MQ2EQBC.ini
Section: [Last Connect]

Server=127.0.0.1
Port=2112
Password=mypassword

Section: [Settings]

AllowControl=1             (allow remote command execution)
AutoConnect=1              (auto-connect on login)
AutoReconnect=1
LocalEcho=1                (echo own messages)
ReconnectRetrySeconds=15
SaveByCharacter=1
SilentCmd=0
```

### Wire Protocol

```
TCP/IP messages to EQBCS relay server
Format: [TAB]COMMAND\n or text\n

Examples:
\tLOGIN[password]=charactername;
\tMSGALL\nMessage text\n
\tTELL\nToonName message\n
\tNAMES\n                  (request connected list)
```

### Dependencies

- **Requires:** EQBCS relay server running (external)
- **Used by:** MQ2NetBots, MQ2AutoLoot, MQ2Boxr

---

## 6. MQ2AdvPath

### Entry Point

```
/advpath record [pathname]
/advpath play [pathname]
/advpath pause|resume
/advpath list|delete
```

### TLO: `${AdvPath}`

```
${AdvPath.Playing}        → bool
${AdvPath.Recording}      → bool
${AdvPath.PathName}       → string
${AdvPath.Progress}       → float (0.0-1.0)
```

### INI File

```
Location: MQ2AdvPath.ini
Stored paths in: {MQConfig}/paths/
```

### Use Cases

- Pre-camp positions before pull
- Consistent movement patterns
- Escape routes

### Dependencies

- **Requires:** MQ2MoveUtils (movement primitives)

---

## 7. MQ2Events

### Entry Point

```
/event add <name> <regex> <command>
/event list|remove|delete|clear
/event load|save <filename>
```

### Example Triggers

```
/event add guardian_add "^\d{1,2}:\d{2} (.*) has become non-aggro" "/assist {1}"
/event add heal_needed "^\d{1,2}:\d{2} (.*) casts a spell" "/casting heal"
/event add aggro_lost "^\d{1,2}:\d{2} You have lost" "/stick hold"
/event add zone_arrive "You have entered" "/announce In {ZONE}"
```

### Match Syntax

```
{1} {2} {3}  → Capture group 1, 2, 3 from regex
{ZONE}       → Current zone name
{TARGET}     → Current target name
{TIME}       → Current game time
```

### Dependencies

- **None** (standalone event system)
- **Foundation for:** MQ2React (higher-level reactions)

---

## 8. MQ2React

### Entry Point

```
/react <reaction> on|off|status
```

### INI File

```
Location: Reactions.ini
Section: [ReactionName]

Event=your_custom_event
Condition=${Me.PctHPs}<50
Action1=/casting "heal me" -kill
Action2=/melee aggro off
Delay=500
```

### Example Reaction

```
[Combat Healer]
Event=guardian_casting
Condition=${Me.PctHPs}<30 && ${Me.PctMana}>25
Action1=/casting "Emergency Heal" -maxtries|2
Action2=/announce Healing at ${Me.PctHPs}%
Delay=100
```

### Dependencies

- **Requires:** MQ2Events (trigger source)
- **Requires:** MQ2Cast (spell execution)
- **Requires:** MQ2Melee (status TLO)

---

## Compatibility Quick Table

| Plugin | Core Req | EQ Version | Linux? | External? |
|--------|----------|-----------|--------|-----------|
| MQ2Melee | 2.21+ | Kunark+ | ✗ | - |
| MQ2Cast | 2.21+ | Kunark+ | ✗ | - |
| MQ2EQBC | 2.20+ | Any | ✓ | EQBCS srv |
| MQ2MoveUtils | 2.21+ | Kunark+ | ✗ | - |
| MQ2AutoLoot | 2.21+ | Underfoot+ | ✗ | - |
| MQ2AdvPath | 2.21+ | Kunark+ | ✗ | - |
| MQ2AutoAccept | 2.20+ | Any | ✓ | - |
| MQ2Events | 2.20+ | Any | ✓ | - |
| MQ2React | 2.21+ | Kunark+ | ✗ | - |
| MQ2NetBots | 2.21+ | Any | ✓ | EQBCS srv |

---

## Common Error Messages

| Message | Cause | Fix |
|---------|-------|-----|
| "Spell casting in progress" | Already casting | Wait for cast to complete |
| "Not enough mana" | Insufficient mana | Wait for regen or drink mana pot |
| "Out of range" | Target too far | Move closer or adjust stick distance |
| "Target disappeared" | Mob died/despawned | Reacquire target |
| "EQBC not connected" | No relay server | Run EQBCS and `/bccmd connect` |
| "Advanced loot not supported" | EQ version too old | Update EQ client |
| "INI file locked" | Another client accessing | Wait or restart process |

---

## Debugging TLOs

Print current status:

```
/echo ${Me.PctHPs}%              (player HP%)
/echo ${Target.ID}               (target spawn ID)
/echo ${Melee.Active}            (melee enabled?)
/echo ${Cast.Result}             (last cast result)
/echo ${Stick.Distance}          (distance to target)
/echo ${EQBC.Connected}          (EQBC relay connected?)
```

View INI values:

```
/ini [section] [key]             (display value)
```

---

## Recommended Multi-Box Configuration

```
Master Box (Leader):
├─ MQ2Melee
├─ MQ2Cast
├─ MQ2MoveUtils
├─ MQ2EQBC (master looter mode)
└─ MQ2Events

Alt Boxes:
├─ MQ2Melee
├─ MQ2Cast
├─ MQ2MoveUtils
├─ MQ2EQBC (connected to master)
├─ MQ2AutoAccept (accept invites)
└─ MQ2Events
```

---

## Key Metrics for FFI Bridge

| Metric | Value | Notes |
|--------|-------|-------|
| Sync Frequency | 6.3x/sec (156ms) | EQ 6s server tick = 38 client frames |
| Command Queue Latency | <50ms | Queued command → executed next pulse |
| State Staleness | <200ms | All TLOs updated per pulse |
| Memory Overhead | ~2KB | Shared state + queue buffers |
| Max Commands/Sec | 1,000 | 50ms pulse * 20 queued |

---

