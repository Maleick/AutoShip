# MacroQuest / OpenVanilla / RedGuides — Coverage Gap Analysis

**Date**: 2026-04-14  
**Auditor**: Copilot Agent  
**Sources**:
- https://github.com/macroquest/macroquest (base platform + core plugins)
- https://github.com/RedGuides/openvanilla (fork with additional community features)
- https://github.com/orgs/RedGuides/repositories?q=props.Type%3Amacro (RedGuides macro/plugin extensions)

**Goal**: At minimum 1:1 parity with every feature MacroQuest, OpenVanilla, and RedGuides ship. TextQuest will exceed them in automation depth, but must cover every feature for the ecosystem to be a drop-in upgrade.

---

## Summary

| Category | MQ2/OV/RG Features | TextQuest Coverage | Gaps |
|---|---|---|---|
| Automation foundation | Lua VM, plugin loader, hotkeys | Planned (#791-#793) | 0 new |
| Combat | Rotation engine, CC, CH chain, pull | ✅ Complete | 5 new |
| Loot / Economy | Auto-loot, vendor, banking, bazaar | ✅ / Planned | 2 new |
| Cross-client comms | EQBC, NetBots, NetHeal | ❌ Missing | 3 new |
| Auto-accept / safety | AutoAccept, AutoRez, AutoCamp, GMCheck | ❌ Missing | 4 new |
| Session tracking | XP, kills, platinum | ❌ Missing | 3 new |
| Alerts / events | Sound, log, timestamp, chat events | ❌ Missing | 5 new |
| Movement extensions | MoveUtils, Relocate, Ice | Partial | 3 new |
| Inventory / items | Bandolier, Cursor, LinkDB, ItemScore, FeedMe | Partial | 5 new |
| Spawn / target UI | OTD, SpawnSort, Posse, GMCheck, SpawnMaster | Partial | 5 new |
| Group / raid mgmt | AutoGroup, RaidUtils, Rand, Rewards, RaidHUD | ❌ Missing | 5 new |
| Economy utilities | TributeManager, TSTrophy, AutoClaim, Vendors | Partial | 4 new |
| System / integration | AutoSize, CPULoad, WinTitle, Boxr, Discord, Clipboard | Partial | 6 new |
| **TOTAL NEW GAPS** | | | **~50 issues** |

---

## Already Covered (no new issue needed)

These MQ2/RedGuides features are either already implemented in TextQuest or tracked in an existing open issue:

| Feature | Status | Issue/Module |
|---|---|---|
| Lua VM | Planned | #791 |
| MQ2 plugin loader | Planned | #792 |
| In-game hotkey system | Planned | #793 |
| Clickies / consumable automation | Planned | #794 |
| Travel module / portal coordination | Planned | #795 |
| Named NPC tracking | Planned | #796 |
| Drag module | Planned | #797 |
| Smart loot expansion | Planned | #798 |
| Enhanced charm/pet management | Planned | #799 |
| Improved pull system | Planned | #800 |
| In-game GUI overlay | Planned | #801 |
| MQ2 API research + FFI | Planned | #1012-#1017 |
| Alert system (death/stuck/resource) | Planned | #1567 |
| Bazaar data capture via DLL | Planned | #1584 |
| Tell relay (MQ2RelayTells) | ✅ Implemented | `camp/tell_relay.rs` |
| Auto-forage (MQ2AutoForage) | ✅ Implemented | `camp/forage.rs` |
| AA spend (MQ2AAPurchase) | ✅ Implemented | `camp/aa_spend.rs` |
| Equipment management (MQ2Exchange partial) | ✅ Implemented | `camp/equipment.rs` |
| Quest/task tracking | ✅ Implemented | `camp/quest_tracker.rs` |
| Collectibles + tribute (MQ2Collectible) | ✅ Implemented | `camp/collectibles.rs` |
| Buff management (MQ2BuffTool) | ✅ Implemented | `camp/buffs.rs` |
| CC handling | ✅ Implemented | `camp/cc.rs` |
| Skill tracker | ✅ Implemented | `camp/skill_tracker.rs` |
| Event triggers | ✅ Implemented | `camp/event_triggers.rs` |
| DPS tracking (basic) | ✅ Implemented | `tui/dps.rs` |
| Spawn list (basic) | ✅ Implemented | `tui/ui/spawns.rs` |
| Map | ✅ Implemented | `tui/ui/map.rs` |
| IPC / orchestration | ✅ Implemented | `ipc/`, `orchestrator/` |
| AutoLogin | ✅ Implemented | `login/` |
| Navigation / navmesh / stuck detection | ✅ Implemented | `nav/` |
| Discord integration (via Soul/alerts) | Planned | #1567, #1234 |
| Vendor cycle | Planned | #1227, #1277 |
| Banking cycle | Planned | #1228 |

---

## New Gaps — Issues to Open

The following issues do not exist in any form and represent true coverage gaps.

---

### Group A — Cross-Client Communication

#### A1: Network Box Chat Server (MQ2EQBC equivalent)

**Summary**: TextQuest uses local named-pipe IPC, which only works on the same machine. MQ2EQBC provides a TCP/IP server (EQBCS) enabling cross-machine client-to-client communication. Without this, multibox setups across different machines cannot coordinate.

**Scope**:
- EQBCS-compatible server (or native TextQuest equivalent) for same-network client coordination
- `/bc`, `/bca`, `/bct` commands for broadcast/targeted cross-client messaging
- Subscribe/publish channel model
- Web UI configuration panel (host, port, auto-connect)
- MQ2EQBC source: https://github.com/RedGuides/MQ2EQBC

**Acceptance**:
- Two TextQuest instances on different machines can exchange commands
- Commands broadcast to all clients execute correctly
- Configurable from web UI

---

#### A2: Cross-Client Game State Sharing (MQ2NetBots equivalent)

**Summary**: MQ2NetBots (built on EQBC) broadcasts each client's HP/mana/endurance/buffs/target/pet to all other clients, enabling group-wide awareness without the game's limited group window. Critical for heal targeting, burn coordination, and raid awareness.

**Scope**:
- Broadcast player vitals (HP%, mana%, endurance%, level, class, zone) to all connected clients
- Broadcast active buffs and durations (including pet buffs as opt-in)
- Broadcast current target and target's HP%
- NetBots-style in-game window showing all client states
- API surface for other modules (healer can query group members' HP from any client)
- Web UI panel showing live state of all connected clients
- MQ2NetBots source: https://github.com/RedGuides/MQ2NetBots

**Acceptance**:
- All connected clients' vitals visible in TUI and web dashboard
- Healer module can target lowest-HP group member using shared data
- State updates within 250ms of game change

---

#### A3: Cross-Client Heal Coordination (MQ2NetHeal equivalent)

**Summary**: MQ2NetHeal uses NetBots state data to coordinate heals across clients, preventing over-healing and ensuring coverage. Related to the CH chain system already in TextQuest, but extends it to reactive single-target heals distributed across healers.

**Scope**:
- Claim-based heal assignment (only one healer responds to a heal need)
- Heal claim expiry with fallback
- Integration with existing CH chain coordinator
- Configurable thresholds and response priorities per healer class
- Web UI configuration panel

**Acceptance**:
- Multiple healers respond to different injured members without doubling up
- Integrates cleanly with existing CH chain system
- Configurable per-class heal thresholds in web UI

---

### Group B — Auto-Automation (Accept / Rez / Camp)

#### B1: Auto-Accept Group Invites, Trades, Tasks, DZs, Translocations, Anchors (MQ2AutoAccept)

**Summary**: MQ2AutoAccept automatically accepts incoming requests based on configurable rules: group invites from specific players/friends, trade confirmation, task adds, expedition (DZ) adds, translocate requests, and primary/secondary anchor teleports. This is essential for fully automated multibox setups where boxes cannot be manually managed.

**Scope**:
- Auto-accept group invites (configurable: from friends only, from MA only, from all)
- Auto-confirm trades (configurable: from trust list only)
- Auto-accept task adds and DZ adds
- Auto-accept translocate and anchor requests
- Trust list management in web UI
- Per-type enable/disable toggles in web UI
- MQ2AutoAccept source: https://github.com/RedGuides/MQ2AutoAccept

**Acceptance**:
- Box characters automatically join groups, DZs, and tasks without manual interaction
- Trust list prevents accepting from unknown players
- All toggles configurable from web dashboard

---

#### B2: Auto-Accept Resurrection Offers (MQ2Rez)

**Summary**: MQ2Rez automatically accepts resurrection offers based on configurable conditions: minimum rez percentage, caster trust list, zone restrictions. TextQuest has recovery.rs for camp recovery but does not have explicit auto-rez offer acceptance.

**Scope**:
- Detect incoming resurrection offer popup
- Auto-accept if conditions met (minimum XP%, caster in trust list)
- Auto-decline if conditions not met (configurable)
- Delay before accepting (to allow manual override)
- Web UI configuration panel
- MQ2Rez source: https://github.com/RedGuides/MQ2Rez

**Acceptance**:
- Boxes automatically accept rez when conditions are met
- Configurable minimum rez percentage (e.g. reject <80% rez)
- Trust list prevents accepting from unknown players
- Web UI panel with per-character configuration

---

#### B3: Auto-Camp to Desktop to Avoid Rez Timer (MQ2AutoCamp)

**Summary**: MQ2AutoCamp automatically `/camp desktop` when a character dies, preventing the rez timer from running while unattended. The character re-logs after a configurable delay. This prevents losing XP to full deaths during overnight sessions.

**Scope**:
- Detect character death event
- After configurable delay, issue `/camp desktop`
- Re-login via AutoLogin system after configurable waiting period
- Option to send alert notification before camping
- Web UI configuration panel with enable/disable per character
- MQ2AutoCamp source: https://github.com/RedGuides/MQ2AutoCamp

**Acceptance**:
- Character auto-camps within configurable seconds of death
- Re-logs automatically using existing AutoLogin flow
- Alert sent via notification system before camping
- Web UI enable/disable per character

---

### Group C — Safety and Awareness

#### C1: GM Detection and Zone-Wide Alert (MQ2GMCheck)

**Summary**: MQ2GMCheck detects when a Game Master enters the zone (unless in stealth mode) and alerts all connected clients immediately. This is a critical safety feature for any unattended session. The existing alert system (#1567) covers operational alerts (death/stuck/resources) but does not specifically handle GM detection.

**Scope**:
- Monitor zone spawns for GM flag (IsGM spawn type)
- Immediately broadcast alert to all connected clients when GM detected
- Sound alert + TUI notification + web UI notification
- Optionally pause all automation when GM detected
- Resume automation after GM leaves zone
- Web UI configuration panel (auto-pause on GM, notification channels)
- MQ2GMCheck source: https://github.com/RedGuides/MQ2GMCheck

**Acceptance**:
- GM entry detected within one game pulse (< 1 second)
- All connected clients receive alert immediately
- Automation optionally pauses and resumes
- Configurable from web UI

---

#### C2: Zone Entry / Exit Player Notifications (MQ2Paranoid / MQ2Spawns)

**Summary**: MQ2Paranoid announces when players zone in and out, and MQ2Spawns provides a dedicated window for all spawn/despawn events. Essential for awareness during unattended farming in contested zones.

**Scope**:
- Detect player (PC) zone-in and zone-out events
- Configurable announcements: all PCs, strangers only, friends only
- Dedicated spawn/despawn event stream in TUI panel
- Optional sound alert on PC zone-in
- Web UI configuration panel
- Sources: https://github.com/RedGuides/MQ2Paranoid, https://github.com/RedGuides/MQ2Spawns

**Acceptance**:
- PC zone-in announcements visible in TUI within one pulse
- Filter by friend/stranger list
- Sound alert fires on PC zone-in when enabled
- Web UI configuration panel

---

#### C3: Rare Spawn Alert System (MQ2SpawnMaster)

**Summary**: MQ2SpawnMaster alerts when specific named NPCs spawn in the zone. Users configure a watch list of rare spawn names. When any watched spawn appears, alerts fire via sound, chat, and notification system. Related to Named Tracking (#796) but focused on the alert/notification aspect rather than combat priority.

**Scope**:
- INI/config-based watch list of NPC names to monitor
- Alert on spawn: sound, TUI notification, web notification, optional EQBC broadcast
- Despawn notification (named NPC killed or wandered out)
- Time-of-spawn tracking (how long since last pop)
- Web UI panel to manage watch list
- MQ2SpawnMaster source: https://github.com/RedGuides/MQ2SpawnMaster

**Acceptance**:
- Named NPC spawn detected within one pulse
- Alert fires via all configured channels
- Watch list manageable from web UI
- History of spawn times visible in web UI

---

### Group D — Combat and Class Features

#### D1: Comprehensive Melee Skill and Disc Automation (MQ2Melee)

**Summary**: MQ2Melee handles all melee-specific automation beyond basic combat rotations: combat disciplines (discs), endurance management, auto-attack control, backstab positioning for rogues, riposte awareness, enrage handling (turn off attack on enrage), headshot tracking for rangers, and special melee skill usage (kick, bash, slam, etc.). TextQuest's combat engine handles spell rotations well but lacks depth in melee-specific mechanics.

**Scope**:
- Combat discipline scheduler (activate discs in priority order, respect cooldowns)
- Endurance management (don't use endurance-consuming abilities when low)
- Auto-attack control (turn off during mez, on enrage, etc.)
- Class-specific melee skill scheduling: kick, bash, slam, backstab, slam, tiger claw, etc.
- Enrage detection + auto-attack pause
- Rogue backstab positioning (move behind mob)
- Ranger disc management (headshot CD tracking)
- Web UI configuration panel per class
- MQ2Melee source: https://github.com/RedGuides/MQ2Melee

**Acceptance**:
- Melee classes use all their combat skills on proper cooldowns
- Enrage detection pauses attack to prevent mob fleeing
- Rogues attempt to maintain backstab position
- All melee settings configurable per class in web UI

---

#### D2: Bard Song Scheduler, Twisting, and Instrument Swap (MQ2Medley / MQ2Twist / MQ2BardSwap)

**Summary**: Bard automation requires three coordinated systems: (1) a song scheduler that cycles through songs with proper timing gaps (MQ2Medley), (2) song twisting that manages the gem-casting cycle to keep multiple songs active simultaneously (MQ2Twist), and (3) instrument swapping to equip the right instrument for each song type for the skill bonus (MQ2BardSwap). These three plugins together make bard fully autonomous. TextQuest likely has basic bard support in the combat engine but not this level of depth.

**Scope**:
- Song schedule definition: ordered list of songs with timing parameters
- Twist logic: automatic gem selection and re-cast timing to maintain song overlap
- Instrument slot management: swap weapon/off-hand to instrument before casting, restore after
- Instrument type mapping to song categories (string, brass, wind, percussion)
- Configurable song list per bard per zone/situation
- Web UI configuration panel with drag-and-drop song ordering
- Sources: https://github.com/RedGuides/MQ2Medley, https://github.com/RedGuides/MQ2Twist, https://github.com/RedGuides/MQ2BardSwap

**Acceptance**:
- Bard maintains 3+ songs simultaneously via twisting
- Correct instrument equipped for each song type
- Song list configurable per situation from web UI
- No dropped songs under normal conditions

---

#### D3: Ranger Headshot Automation (MQ2Headshot)

**Summary**: Rangers have a unique ability — Headshot — that instantly kills low-level NPCs when activated at the right moment. Optimizing headshot requires detecting when the ability is available and using it on appropriate targets. This is a high-value class-specific feature for ranger-based farming.

**Scope**:
- Detect headshot disc availability (cooldown tracking)
- Auto-activate headshot on valid target (level check, health threshold)
- Integrate with existing ranger combat rotation
- Statistics tracking: headshot attempts, successes, kills
- Web UI configuration panel: enable/disable, target level range, minimum HP%
- MQ2Headshot source: https://github.com/RedGuides/MQ2Headshot

**Acceptance**:
- Headshot activated automatically when disc is ready and target is valid
- Level-appropriate targeting respected
- Statistics visible in session tracker
- Web UI enable/disable with configuration

---

#### D4: Cross-Group and Outside-Group Assist (MQ2XAssist)

**Summary**: MQ2XAssist enables a character to assist a Main Assist who is outside their group or raid. Essential for scenarios where the MA is not in the same group (cross-group raiding, solo boxes assisting a main). TextQuest's orchestrator handles group assist within the group, but not cross-group targeting.

**Scope**:
- Configure a named character to assist (outside own group)
- Auto-target the configured MA's target
- Works in raid and non-group scenarios
- Priority over group MA when configured
- Web UI configuration per character
- MQ2XAssist source: https://github.com/RedGuides/MQ2XAssist

**Acceptance**:
- Box character targets and attacks the same mob as the configured cross-group MA
- Configurable from web UI per character
- Takes priority over in-group assist target when enabled

---

#### D5: Worst-Hurt Party/Pet Member Targeting for Healing (MQ2WorstHurt)

**Summary**: MQ2WorstHurt finds the most injured member of the party, extended target list, or pet and targets them. This is a core helper for non-CH-chain healing scenarios where the healer needs to react to the most hurt member in real time.

**Scope**:
- Scan group members, XTarget list, and pet for lowest HP%
- Target the most injured member for healing
- Configurable: group only, group + XTarget, group + pet
- Expose API so combat engine's heal loop can call this
- Web UI configuration panel
- MQ2WorstHurt source: https://github.com/RedGuides/MQ2WorstHurt

**Acceptance**:
- Healer targets most injured member within one pulse
- Group + XTarget + pet scanning works correctly
- Integration with heal rotation confirmed in tests
- Web UI configuration panel

---

### Group E — Session Tracking

#### E1: XP and AA Experience Per-Hour Tracking (MQ2XPTracker)

**Summary**: MQ2XPTracker tracks experience gain over time, calculating XP/hour and AA/hour rates with session history. Essential for optimizing farm routes and measuring efficiency improvements. Currently TextQuest has no XP tracking.

**Scope**:
- Track regular XP gain (% per hour) and AA XP gain (AA per hour)
- Session start/stop tracking with pause support
- Historical data stored per character per session
- TUI panel showing current rate and session summary
- Web UI panel with session history and rate charts
- MQ2XPTracker source: https://github.com/RedGuides/MQ2XPTracker

**Acceptance**:
- XP/hour and AA/hour rates accurate within 5%
- Session history persisted across restarts
- Visible in both TUI and web dashboard
- Web UI shows rate trends over multiple sessions

---

#### E2: Kill Count Tracking and Auto-Reporting (MQ2KillTracker)

**Summary**: MQ2KillTracker tracks kills over time and can auto-report kill counts every N minutes. Essential for measuring farm efficiency and setting up reports during overnight sessions.

**Scope**:
- Track kills per character per session (by mob name, zone, time)
- Calculate kills/hour rate
- Auto-report kill count to configured channel every N minutes (optional)
- Session history stored per character
- TUI panel and web UI with session kill data
- MQ2KillTracker source: https://github.com/RedGuides/MQ2KillTracker

**Acceptance**:
- Accurate kill count maintained throughout session
- Kill/hour rate displayed in TUI and web UI
- Optional auto-reporting to chat channel works
- Web UI shows kill history per session and zone

---

#### E3: Platinum Gain/Loss Session Tracking (MQ2PlatTracker)

**Summary**: MQ2PlatTracker tracks platinum changes during a session (gains from selling loot, losses from repairs/spell purchases). Essential for measuring economy output and ensuring the economy system is generating profit.

**Scope**:
- Track platinum, gold, silver, copper changes
- Per-transaction logging (loot sale, vendor purchase, etc.)
- Session total and running rate (plat/hour)
- Integration with existing economy/ledger system
- TUI panel and web UI with session economy data
- MQ2PlatTracker source: https://github.com/RedGuides/MQ2PlatTracker

**Acceptance**:
- Platinum changes accurately tracked throughout session
- Per-transaction log available in web UI
- Plat/hour rate displayed
- Integrates with existing economy ledger module

---

### Group F — Alerts and Events

#### F1: Sound-Based Alerts for Game Events (MQ2Sound)

**Summary**: MQ2Sound plays audio alerts for configurable game events (low HP, target death, named spawn, GM detection, etc.). Sound alerts are essential for operators monitoring from a distance who need audio cues without watching screens.

**Scope**:
- Define sound events: low HP, death, named spawn, GM enter, tell received, etc.
- Play WAV/MP3 file or system beep on event
- Volume and enable/disable per event type
- Integration with alert system (#1567) — sound as a notification channel
- Web UI configuration panel with event-to-sound mapping
- MQ2Sound source: https://docs.macroquest.org/plugins/community-plugins/mq2sound/

**Acceptance**:
- Sound plays within one game pulse of triggering event
- All event types configurable individually in web UI
- Volume control works
- Mute-all override available for quiet environments

---

#### F2: Chat Output Logging to File Per Character (MQ2Log)

**Summary**: MQ2Log writes all MQ2 chat window output to a log file per character (server_charname.log). Essential for post-session debugging, audit trails, and reviewing what happened during overnight sessions.

**Scope**:
- Log all MQ2 output to `logs/server_charname.log`
- Optional: also log regular EQ chat channels
- Configurable log rotation (daily, by size)
- Log level filtering (info, debug, etc.)
- Web UI configuration panel: enable/disable, channels to log
- MQ2Log source: https://github.com/RedGuides/MQ2Log

**Acceptance**:
- All MQ2 output written to log file per character
- Log files accessible and readable
- Log rotation prevents unbounded disk growth
- Web UI enable/disable per character

---

#### F3: Chat Message Timestamps (MQ2Timestamp)

**Summary**: MQ2Timestamp adds timestamps to all chat messages, making it possible to correlate game events with log entries and understand timing of events during reviews.

**Scope**:
- Prepend configurable timestamp format to all MQ2 chat messages
- Configurable format: 24h, 12h, include date, etc.
- Enable/disable from web UI per character
- MQ2Timestamp source: https://docs.macroquest.org/plugins/community-plugins/mq2timestamp/

**Acceptance**:
- Timestamps appear on all MQ2 chat output
- Format configurable from web UI
- Toggle-able without restart

---

#### F4: Pattern-Based Chat Event Triggers (MQ2Events / MQ2React / MQ2ChatEvents)

**Summary**: These plugins provide a general-purpose if-then system: define text patterns to match against incoming game messages, then define actions to execute when matched (run a command, cast a spell, send a message, etc.). This is distinct from TextQuest's internal event_triggers.rs which handles hard-coded events. The MQ2 approach allows user-defined patterns configurable without code changes.

**Scope**:
- User-definable event rules: pattern (regex or string match) → action (command string)
- Event sources: any game chat channel, system messages, tells, emotes
- Action types: execute EQ command, send IPC command, trigger alert
- Rule priority and enable/disable per rule
- Web UI rule editor with add/edit/delete
- Sources: https://github.com/RedGuides/MQ2Events, https://github.com/RedGuides/MQ2React

**Acceptance**:
- User can define custom patterns and actions without code changes
- Rules fire within one game pulse of trigger
- Web UI rule editor works end-to-end
- At least 50 active rules without performance impact

---

#### F5: Say Detection and Alerting (MQ2Say)

**Summary**: MQ2Say detects specific text patterns in `/say` chat and triggers alerts or actions. Used for detecting NPC dialogue (quest text, warning messages), player communication attempts, or GM messages arriving via say.

**Scope**:
- Monitor `/say` channel for pattern matches
- Configurable actions: alert, run command, broadcast to other clients
- Integration with pattern-based event system (F4) or standalone
- Web UI configuration
- MQ2Say source: https://github.com/RedGuides/MQ2Say

**Acceptance**:
- Pattern matches detected within one pulse
- Alert fires via configured notification channels
- Web UI rule editor

---

### Group G — Movement Extensions

#### G1: Extended Movement Modes — makecamp, stick, circle (MQ2MoveUtils)

**Summary**: MQ2MoveUtils provides movement commands beyond basic navigation: `/makecamp` (establish a camp radius and auto-return), `/stick` (maintain melee range while following a moving target), and `/circle` (move in a circular pattern). TextQuest's navigator handles pathfinding but lacks these specific movement modes that are core to MQ2 muscle memory.

**Scope**:
- `/makecamp` equivalent: establish camp point, auto-return after combat, configurable radius
- `/stick` equivalent: maintain configurable distance from a moving target
- `/circle` equivalent: circular movement around a point
- `/moveto` equivalent: move to specific coordinates with obstacle avoidance
- Commands exposed via IPC for macro/script control
- Web UI configuration panel with camp point visualization on map
- MQ2MoveUtils source: https://github.com/RedGuides/MQ2MoveUtils

**Acceptance**:
- `makecamp` auto-returns character to camp point after each pull
- `stick` maintains specified range from moving target
- `circle` executes smooth circular movement
- All modes integrated with existing navmesh pathfinding
- Web UI shows camp point on zone map

---

#### G2: Relocation Item and AA Management (MQ2Relocate)

**Summary**: MQ2Relocate manages relocation items (various teleport clickies) and relocation AAs (Throne of Heroes, etc.), providing easy `/relocate` commands to quickly travel to known locations. Useful for fast travel between camp zones.

**Scope**:
- Maintain a list of relocation options: items and AAs with their destinations
- `/relocate [destination]` command to use appropriate item/AA
- Auto-select best travel option (prefer AA over clickie if both available)
- Integration with travel module (#795)
- Web UI panel listing available relocations and their current cooldowns
- MQ2Relocate source: https://github.com/RedGuides/MQ2Relocate

**Acceptance**:
- Character uses correct relocation option for requested destination
- Cooldown status visible in web UI
- Integrates with existing travel module

---

#### G3: Ice-Surface Movement Physics Helper (MQ2Ice)

**Summary**: MQ2Ice helps with movement on ice surfaces (present in Velious content) where standard movement control is compromised by slippery physics. Important for farming Velious-era zones.

**Scope**:
- Detect ice-surface movement mode
- Apply corrective movement adjustments for ice physics
- Configurable sensitivity for ice correction
- Web UI toggle enable/disable

**Acceptance**:
- Character maintains intended path on ice surfaces
- Configurable from web UI
- Does not interfere with normal movement

---

### Group H — Inventory and Items

#### H1: Extended Bandolier — Swap Any Equipment Slot (MQ2Bandolier)

**Summary**: EQ's built-in bandolier only manages weapon slots. MQ2Bandolier extends this to all equipment slots, enabling complex gear swaps (tank → healer set, fishing gear, tradeskill gear, etc.) with a single command. TextQuest has equipment.rs for gear set management, but needs to verify full-slot coverage and web UI configuration.

**Scope**:
- Named gear sets covering all equipment slots
- `/bandolier activate [setname]` command
- Swap sets from web UI
- Per-character gear sets stored in config
- Integration with equipment.rs if already partially implemented
- MQ2Bandolier source: https://github.com/RedGuides/MQ2Bandolier

**Acceptance**:
- Complete gear set (all slots) swappable in one command
- Multiple named sets supported per character
- Sets configurable in web UI
- Swap completes within 3 seconds

---

#### H2: Cursor Item Management with Quantity Rules (MQ2Cursor)

**Summary**: MQ2Cursor provides INI-based rules for handling items that appear on the cursor: auto-keep up to N of an item, destroy/drop/consume extras, handle specific items always keep/always destroy. Essential for fully automated looting where the cursor can get stuck on unwanted items.

**Scope**:
- Rule-based cursor handling: item name → action (keep N, destroy, drop, consume)
- Auto-process cursor items without manual intervention
- Integration with loot system (auto-loot creates cursor items)
- Web UI rule editor with keep/destroy/drop actions and quantities
- MQ2Cursor source: https://github.com/RedGuides/MQ2Cursor

**Acceptance**:
- Cursor items processed automatically based on rules
- No cursor-blocked state during automated sessions
- Web UI rule editor works end-to-end
- Rules applied within one pulse of item appearing on cursor

---

#### H3: Item Link Database for Items Not in Inventory (MQ2LinkDB / MQ2FakeLink)

**Summary**: MQ2LinkDB maintains a database of item IDs to generate item links for items not currently in inventory. Useful for price discussion, trade negotiation, and economy research. MQ2FakeLink provides similar functionality.

**Scope**:
- Item link database (from EQData or community sources)
- Generate item links by item name or ID
- `/itemlink [itemname]` command
- Integration with bazaar/economy module
- MQ2LinkDB source: https://github.com/RedGuides/MQ2LinkDB

**Acceptance**:
- Item links generated for any known EQ item
- Links display correctly in game chat
- Command accessible from IPC

---

#### H4: Item Upgrade Scoring and Comparison Engine (MQ2ItemScore)

**Summary**: MQ2ItemScore rates whether a looted item is an upgrade for your character/class based on configurable stat weights. Essential for auto-loot decisions during farming when deciding what to keep vs. vendor.

**Scope**:
- Per-class stat weight configuration (strength, agility, etc.)
- Score calculated for any item vs. currently equipped item in same slot
- Expose score via API for loot module to use in keep/sell decisions
- Web UI configuration panel for stat weights per class
- MQ2ItemScore source: https://github.com/RedGuides/MQ2ItemScore

**Acceptance**:
- Item score calculated for any looted item
- Loot module can query score before deciding keep/sell
- Stat weights configurable per class in web UI
- Score logic tested with known item comparisons

---

#### H5: Food and Drink Auto-Consumption (MQ2FeedMe)

**Summary**: MQ2FeedMe monitors hunger/thirst status and automatically uses food/drink items from inventory when needed. Characters that run out of food lose stat bonuses. During long farming sessions this requires periodic attention.

**Scope**:
- Monitor hunger and thirst status
- Auto-consume food/drink from inventory when hungry/thirsty
- Configurable preferred food/drink items
- Alert when food/drink supply is running low
- Web UI configuration panel
- MQ2FeedMe source: https://github.com/RedGuides/MQ2FeedMe

**Acceptance**:
- Food/drink consumed automatically when hungry/thirsty
- Alert fires when supply drops below configured threshold
- Preferred item selection configurable in web UI

---

### Group I — Target and Spawn UI

#### I1: Overhead Target Direction Compass Overlay (MQ2OTD)

**Summary**: MQ2OTD displays a compass in the EQ HUD showing the direction to the current target. Useful for finding spawns and navigating to targets. Complements the existing map module.

**Scope**:
- Calculate heading from player position to target
- Display direction indicator in EQ HUD or TUI
- Update in real-time as target or player moves
- Web UI: show heading to target in session panel
- MQ2OTD source: https://github.com/RedGuides/MQ2OTD

**Acceptance**:
- Heading to target accurate within 5 degrees
- Updates within one pulse as positions change
- Visible in TUI map panel

---

#### I2: Filterable Sortable Spawn Finder (MQ2SpawnSort)

**Summary**: MQ2SpawnSort provides a searchable, sortable list of all zone spawns by any variable (name, level, distance, type, etc.). More powerful than the basic spawns list in the TUI.

**Scope**:
- Spawn list sortable by: name, level, class, race, distance, HP%
- Text search filter
- Quick-target from spawn list
- Export to file for analysis
- Web UI spawn finder panel
- MQ2SpawnSort source: https://github.com/RedGuides/MQ2SpawnSort

**Acceptance**:
- Spawn list sortable by all major fields
- Text search filters results in real-time
- Clicking spawn targets it (where applicable)
- Web UI panel accessible

---

#### I3: PC Proximity Friend/Stranger Detector (MQ2Posse)

**Summary**: MQ2Posse checks for player characters in a defined radius and identifies them as friends or strangers. Used for safety awareness in open-world farming — alert when unknown players approach camp.

**Scope**:
- Configurable detection radius
- Friend list (known players, guild members)
- Alert when stranger enters radius
- Alert when any PC enters radius (stricter mode)
- Integration with GM check and spawn notification systems
- Web UI friend list management and radius configuration
- MQ2Posse source: https://github.com/RedGuides/MQ2Posse

**Acceptance**:
- PC proximity detected within one pulse
- Friend/stranger classification accurate
- Alert fires via configured notification channels
- Web UI friend list editor

---

### Group J — Group and Raid Management

#### J1: Auto Group Creation with Role Assignment (MQ2AutoGroup)

**Summary**: MQ2AutoGroup automatically creates a group, assigns group roles (tank, healer, etc.), and then runs a configurable command on completion. Eliminates manual group forming for automated sessions.

**Scope**:
- Invite specific characters to group in order
- Assign group roles after all members join
- Execute configurable command after group is formed
- Handle invite failures with retry
- Web UI configuration: member list, roles, completion command
- MQ2AutoGroup source: https://github.com/RedGuides/MQ2AutoGroup

**Acceptance**:
- Group formed automatically with correct members
- Roles assigned correctly
- Completion command executes after group is ready
- Web UI configuration panel

---

#### J2: Raid DZ/Task Add-All Utilities (MQ2RaidUtils)

**Summary**: MQ2RaidUtils makes leading raids easier by adding "All" option to DZ/task add and remove commands, eliminating the need to individually add each raider. Essential for large-scale multibox raid operations.

**Scope**:
- `/dzadd all` — add all raid members to a DZ in one command
- `/taskaddalladd` — add all raid members to a task
- Remove equivalents
- Error handling for failed adds (report failures)
- Web UI: one-click add-all DZ/task button
- MQ2RaidUtils source: https://github.com/RedGuides/MQ2RaidUtils

**Acceptance**:
- All raid members added to DZ/task in single command
- Failed adds logged and reported
- Web UI button triggers add-all

---

#### J3: Raid Random Roll Helper (MQ2Rand)

**Summary**: MQ2Rand is a `/random` helper for raid leaders to determine winners for GDKP and loot distribution. Not directly automation but essential for raid leadership workflow.

**Scope**:
- `/rand` command with automatic roll tracking
- Announce results to raid/group
- Track participants in a given roll
- History of recent rolls
- Web UI: roll management panel
- MQ2Rand source: https://github.com/RedGuides/MQ2Rand

**Acceptance**:
- Rolls tracked and announced automatically
- Roll history accessible in web UI
- Multiple simultaneous roll sets supported

---

#### J4: Mission Reward Selection Automation (MQ2Rewards)

**Summary**: MQ2Rewards allows control over specifying, selecting, and claiming rewards from missions/tasks via macros. Enables automated task farming with automatic reward selection.

**Scope**:
- Define preferred reward by name or position per task type
- Auto-select and claim reward when task completes
- Handle reward window detection
- Web UI per-task reward configuration
- MQ2Rewards source: https://github.com/RedGuides/MQ2Rewards

**Acceptance**:
- Reward selected automatically when task completes
- Per-task preference configurable
- Web UI configuration panel

---

#### J5: Raid Availability HUD Dashboard (raidhud Lua equivalent)

**Summary**: The RedGuides `raidhud` Lua script provides a lightweight in-game dashboard showing which raids are currently available at a glance. Useful for planning daily/weekly raid targets.

**Scope**:
- Track raid availability by expansion/instance reset timers
- Display available raids in TUI and web dashboard
- Alert when a raid instance resets and becomes available
- Configurable watch list of target raids
- Web UI panel with raid availability calendar
- raidhud source: https://github.com/RedGuides/raidhud

**Acceptance**:
- Current raid availability visible in web dashboard
- Reset timers tracked accurately
- Alert fires when watched raid becomes available

---

### Group K — Economy and Subscription Utilities

#### K1: Tribute Management Automation (MQ2TributeManager)

**Summary**: MQ2TributeManager adds a `/tribute` command for tribute point management and automatic tribute activation. TextQuest's collectibles.rs mentions tribute but may not have full automation.

**Scope**:
- Monitor tribute timer and auto-activate when expired
- Configure preferred tribute selections per character
- Track tribute point balance
- Web UI panel: current tribute status, activation toggle, point balance
- MQ2TributeManager source: https://github.com/RedGuides/MQ2TributeManager

**Acceptance**:
- Tribute auto-activated when it expires
- Preferred selections configurable in web UI
- Tribute balance visible in web dashboard

---

#### K2: Tradeskill Trophy Management (MQ2TSTrophy)

**Summary**: MQ2TSTrophy manages tradeskill trophy items that provide tradeskill bonuses. These items need to be equipped before crafting and removed after. Useful for characters that do tradeskill work during downtime.

**Scope**:
- Auto-equip tradeskill trophy before crafting attempts
- Remove trophy after crafting session
- Track trophy charges (some have limited uses)
- Integration with tradeskill automation if any
- Web UI configuration panel

**Acceptance**:
- Trophy equipped/removed correctly around crafting
- Charge tracking accurate
- Web UI toggle

---

#### K3: DBCash All-Access Subscription Auto-Claim (MQ2AutoClaim)

**Summary**: MQ2AutoClaim automatically claims DBCash from being an all-access member, handling the claim popup without manual intervention. Small QoL but needed for fully automated sessions.

**Scope**:
- Detect and dismiss DBCash claim popup automatically
- Claim amount logging
- Web UI enable/disable toggle

**Acceptance**:
- DBCash popup handled without manual intervention
- Web UI toggle

---

#### K4: Vendor Item Search and Price Alerts in Web UI (MQ2Vendors)

**Summary**: MQ2Vendors alerts when browsing a vendor and a specific watched item appears for sale. TextQuest already has vendor cycle logic for selling, but not for monitoring vendor stock for specific items.

**Scope**:
- Watch list of items to look for on vendors
- Alert when watched item spotted on vendor during normal vendor interactions
- Price comparison (expected vs. actual vendor price)
- Web UI watch list management
- MQ2Vendors source: https://github.com/RedGuides/MQ2Vendors

**Acceptance**:
- Watched items detected when browsing vendors
- Alert fires via notification system
- Web UI watch list editor

---

### Group L — System and Integration

#### L1: Auto-Resize Characters/NPCs to Minimum Size (MQ2AutoSize)

**Summary**: MQ2AutoSize shrinks all characters and NPCs in range to minimum size. Useful for improving visibility in cramped dungeon areas and reducing visual clutter during large mob pulls.

**Scope**:
- `/autosize` command to toggle auto-sizing
- Configurable: shrink PCs, shrink NPCs, shrink self, shrink pets
- Configurable size percentage
- Web UI toggle with size configuration
- MQ2AutoSize source: https://github.com/RedGuides/MQ2AutoSize

**Acceptance**:
- Characters/NPCs resized to configured size
- Toggle works from web UI
- Does not interfere with targeting or combat

---

#### L2: CPU Load Balancer for Multi-EQ-Window Setups (MQ2CPULoad)

**Summary**: MQ2CPULoad assigns the focused (foreground) EQ window its own CPU core and distributes other instances across remaining cores. This significantly improves responsiveness of the active window during manual play.

**Scope**:
- Detect active/foreground EQ window
- Assign active window to dedicated CPU core via process affinity
- Distribute other windows across remaining cores
- Update affinity when focus changes
- Web UI enable/disable toggle and core assignment visualization
- MQ2CPULoad source: https://github.com/RedGuides/MQ2CPULoad

**Acceptance**:
- Active window assigned to dedicated core
- Core affinity updates on focus change
- Web UI shows current core assignment per client

---

#### L3: EQ Window Title Customization Per Character (MQ2WinTitle)

**Summary**: MQ2WinTitle changes the EQ window title based on INI config, typically to show character name and server. Essential for identifying windows when managing 6-36 EQ instances from the OS taskbar.

**Scope**:
- Configurable title format: `[server] charactername (level class)`
- Update title on character load and zone change
- Web UI format string configuration per character
- MQ2WinTitle source: https://github.com/RedGuides/MQ2WinTitle

**Acceptance**:
- Window titles show character info correctly
- Title updates on zone change
- Format configurable from web UI

---

#### L4: Unified Box Controller Interface (MQ2Boxr equivalent)

**Summary**: MQ2Boxr provides a unified interface to control boxes running different automation software (CWTN, KissAssist, MuleAssist, RGMercs, Entropy, XGen) via a single `/boxr` command. For TextQuest, this means providing a standardized control API that can be broadcast to all characters regardless of their individual automation configuration. This ensures interoperability with the broader RedGuides ecosystem.

**Scope**:
- Standardized control commands: Pause, Unpause, Camp, Chase, Manual, BurnNow, RaidAssistNum
- Broadcast via EQBC equivalent (Group A issues)
- Per-client automation state reporting
- Web UI: global pause/unpause/mode buttons that broadcast to all clients
- MQ2Boxr source: https://github.com/RedGuides/MQ2Boxr

**Acceptance**:
- Single command pauses/unpauses all connected clients
- Camp/Chase/Manual modes broadcast correctly
- Web UI global control panel works
- State visible per client in web dashboard

---

#### L5: Discord Webhook Integration (MQ2Discord)

**Summary**: MQ2Discord sends alerts to Discord via webhooks. The alert system (#1567) mentions Discord as an alert channel but this needs a dedicated implementation of the Discord webhook client with message formatting, rate limiting, and embed support for rich notifications.

**Scope**:
- Discord webhook client (POST to webhook URL)
- Message types: plain text, embed with color/fields
- Rate limiting (Discord limits 30 requests/minute per webhook)
- Per-event type configuration (death → @everyone, info → no ping)
- Web UI configuration: webhook URLs, notification level per event
- MQ2Discord source: https://github.com/RedGuides/MQ2Discord

**Acceptance**:
- Death alerts appear in Discord within 5 seconds
- Rich embed format with character name, zone, cause
- Rate limiting prevents Discord webhook bans
- Multiple webhook URLs configurable (different channels for different alert types)
- Web UI webhook configuration panel

---

#### L6: In-Game Clipboard Copy (MQ2Clipboard)

**Summary**: MQ2Clipboard copies in-game text and command output to the OS clipboard. Useful for pasting item stats, character info, or command output into external tools.

**Scope**:
- `/clipboard` command to copy last MQ2 output to clipboard
- Optional: copy any in-game text string to clipboard
- Windows clipboard API integration in DLL
- Web UI: clipboard history panel (last N items)
- MQ2Clipboard source: https://github.com/RedGuides/MQ2Clipboard

**Acceptance**:
- Game text copyable to OS clipboard via command
- Clipboard history visible in web UI
- Works on Windows (DLL injection context)

---

## Web UI Configuration Requirement

Per the user's requirement, all features above that have user-configurable settings MUST include a configuration panel in the TextQuest web UI (`textquest-web`). This means each issue should include:

1. A specification of what settings are exposed in the web UI
2. The component location in the existing React SPA
3. API endpoint requirements in `textquest-web/src/api/`

The left sidebar navigation in the web UI currently has: Active Engagements, Fleet Formations, Realm Map, Security Wards, Loot Configuration, Soul Engine. New feature panels should be added as sub-sections of relevant existing views or as new top-level views where warranted.

---

## Coverage Matrix

| MQ2/OpenVanilla/RedGuides Feature | Status |
|---|---|
| MQ2EQBC | ❌ Gap: See A1 |
| MQ2NetBots | ❌ Gap: See A2 |
| MQ2NetHeal | ❌ Gap: See A3 |
| MQ2AutoAccept | ❌ Gap: See B1 |
| MQ2Rez | ❌ Gap: See B2 |
| MQ2AutoCamp | ❌ Gap: See B3 |
| MQ2GMCheck | ❌ Gap: See C1 |
| MQ2Paranoid / MQ2Spawns | ❌ Gap: See C2 |
| MQ2SpawnMaster | ❌ Gap: See C3 |
| MQ2Melee (disc/endurance) | ❌ Gap: See D1 |
| MQ2Medley / MQ2Twist / MQ2BardSwap | ❌ Gap: See D2 |
| MQ2Headshot | ❌ Gap: See D3 |
| MQ2XAssist | ❌ Gap: See D4 |
| MQ2WorstHurt | ❌ Gap: See D5 |
| MQ2XPTracker | ❌ Gap: See E1 |
| MQ2KillTracker | ❌ Gap: See E2 |
| MQ2PlatTracker | ❌ Gap: See E3 |
| MQ2Sound | ❌ Gap: See F1 |
| MQ2Log | ❌ Gap: See F2 |
| MQ2Timestamp | ❌ Gap: See F3 |
| MQ2Events / MQ2React | ❌ Gap: See F4 |
| MQ2Say | ❌ Gap: See F5 |
| MQ2MoveUtils (makecamp/stick/circle) | ❌ Gap: See G1 |
| MQ2Relocate | ❌ Gap: See G2 |
| MQ2Ice | ❌ Gap: See G3 |
| MQ2Bandolier | ❌ Gap: See H1 |
| MQ2Cursor | ❌ Gap: See H2 |
| MQ2LinkDB / MQ2FakeLink | ❌ Gap: See H3 |
| MQ2ItemScore | ❌ Gap: See H4 |
| MQ2FeedMe | ❌ Gap: See H5 |
| MQ2OTD | ❌ Gap: See I1 |
| MQ2SpawnSort | ❌ Gap: See I2 |
| MQ2Posse | ❌ Gap: See I3 |
| MQ2AutoGroup | ❌ Gap: See J1 |
| MQ2RaidUtils | ❌ Gap: See J2 |
| MQ2Rand | ❌ Gap: See J3 |
| MQ2Rewards | ❌ Gap: See J4 |
| raidhud | ❌ Gap: See J5 |
| MQ2TributeManager | ❌ Gap: See K1 |
| MQ2TSTrophy | ❌ Gap: See K2 |
| MQ2AutoClaim | ❌ Gap: See K3 |
| MQ2Vendors (web config) | ❌ Gap: See K4 |
| MQ2AutoSize | ❌ Gap: See L1 |
| MQ2CPULoad | ❌ Gap: See L2 |
| MQ2WinTitle | ❌ Gap: See L3 |
| MQ2Boxr | ❌ Gap: See L4 |
| MQ2Discord | ❌ Gap: See L5 |
| MQ2Clipboard | ❌ Gap: See L6 |
| MQ2EQIM | 🚫 Discontinued — skip |
| MQ2IRC | 🚫 Discontinued — skip |
| MQ2FPS | 🚫 Discontinued — skip |
| MQ2Telnet | 🚫 Discontinued — skip |
| AutoLogin | ✅ Implemented |
| Map (enhanced) | ✅ Implemented |
| MQ2RelayTells | ✅ Implemented — tell_relay.rs |
| MQ2AutoForage | ✅ Implemented — forage.rs |
| MQ2AAPurchase | ✅ Implemented — aa_spend.rs |
| MQ2Exchange (partial) | ✅ Implemented — equipment.rs |
| MQ2Collectible | ✅ Implemented — collectibles.rs |
| MQ2BuffTool | ✅ Implemented — buffs.rs |
| Chat events (internal) | ✅ Implemented — event_triggers.rs |
| Skill tracking | ✅ Implemented — skill_tracker.rs |
| CC handling | ✅ Implemented — camp/cc.rs |
| DPS tracking (basic) | ✅ Implemented — tui/dps.rs |
| HUD/Overlay | Planned — #801 |
| CustomBinds / Hotkeys | Planned — #793 |
| ItemDisplay enhancements | Planned — #801 (in-game GUI scope) |
| MQ2Cast deep features | Planned — #1015 (MQ2 API bridge) |
| Lua VM | Planned — #791 |
| MQ2 plugin loader | Planned — #792 |
| Bazaar search/capture | Planned — #1584 |
| Alert system | Planned — #1567 |
| Discord (via alerts) | Planned — #1567 / #1234 |
| Vendor cycle | Planned — #1227 |
| Banking cycle | Planned — #1228 |
| Named NPC tracking | Planned — #796 |
| Smart loot | Planned — #798 |
| Charm/pet mgmt | Planned — #799 |
| Pull system | Planned — #800 |
| Travel module | Planned — #795 |
| Drag module | Planned — #797 |
| Clickies | Planned — #794 |
| MQ2AutoSize (partially overlap w/ #801) | Partial — See L1 |
| MQ2PluginManager | Planned — #1014 (loader UI) |
| MQ2NetHeal | Overlap with CH chain (#804?) | See A3 |
