#!/usr/bin/env python3
"""
Create GitHub issues for MacroQuest / OpenVanilla / RedGuides coverage gaps.

Usage:
    python3 scripts/create_mq2_gap_issues.py

Requires: GITHUB_TOKEN env var with 'issues: write' permission.
Run: export GITHUB_TOKEN=your_pat_with_issues_write
"""

import os
import json
import time
import urllib.request
import urllib.error

REPO = "Maleick/TextQuest"
API_BASE = "https://api.github.com"


def create_issue(token: str, title: str, body: str, labels: list[str] | None = None) -> dict:
    url = f"{API_BASE}/repos/{REPO}/issues"
    payload = {"title": title, "body": body}
    if labels:
        payload["labels"] = labels
    data = json.dumps(payload).encode()
    req = urllib.request.Request(url, data=data, method="POST")
    req.add_header("Authorization", f"token {token}")
    req.add_header("Content-Type", "application/json")
    req.add_header("Accept", "application/vnd.github+json")
    try:
        with urllib.request.urlopen(req) as resp:
            result = json.load(resp)
            print(f"  Created #{result['number']}: {title}")
            return result
    except urllib.error.HTTPError as e:
        body_text = e.read().decode()
        print(f"  ERROR creating '{title}': {e.code} {body_text[:200]}")
        return {}


ISSUES = [
    # ── Group A: Cross-Client Communication ──────────────────────────────────
    {
        "title": "Feature: Network Box Chat Server — MQ2EQBC parity",
        "labels": ["agent:ready", "p1-high", "networking", "multibox"],
        "body": """## Summary

TextQuest uses local named-pipe IPC (single-machine only). MQ2EQBC provides a TCP/IP server (EQBCS) enabling cross-machine client-to-client communication with `/bc`, `/bca`, `/bct` broadcast commands. Without this, multibox setups across different machines cannot coordinate.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Two TextQuest instances on separate machines exchange commands via network
- `/bc <message>` broadcasts to all connected clients
- `/bct <character> <message>` sends to specific client
- Host, port, and auto-connect configurable in web UI
- All settings configurable without restart

## References

- MQ2EQBC: https://github.com/RedGuides/MQ2EQBC
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#a1
""",
    },
    {
        "title": "Feature: Cross-Client Game State Sharing — MQ2NetBots parity",
        "labels": ["agent:ready", "p1-high", "networking", "multibox"],
        "body": """## Summary

MQ2NetBots (built on EQBC) broadcasts each client's HP/mana/endurance/buffs/target/pet to all other clients. This enables group-wide awareness without the game's limited group window. Critical for heal targeting, burn coordination, and raid awareness. TextQuest has no equivalent cross-client state broadcast.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- All connected clients' vitals (HP%, mana%, endurance%, target, buffs) broadcast to all others
- State visible in TUI group panel and web dashboard
- Healer module can query lowest-HP group member from any client
- State updates within 250ms of game change
- Extended data (buff durations, pet buffs) available as opt-in

## References

- MQ2NetBots: https://github.com/RedGuides/MQ2NetBots
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#a2
""",
    },
    {
        "title": "Feature: Cross-Client Heal Coordination — MQ2NetHeal parity",
        "labels": ["agent:ready", "p1-high", "networking", "combat"],
        "body": """## Summary

MQ2NetHeal uses NetBots state data to coordinate heals across clients, preventing over-healing and ensuring coverage. Extends the existing CH chain (which handles planned chain-heal rotation) to cover reactive single-target heals distributed across multiple healers in real time.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Claim-based heal assignment: only one healer responds to each heal need
- Heal claim expires after configurable timeout with fallback to next healer
- Integrates with existing CH chain coordinator
- Configurable thresholds and response priorities per healer class
- Web UI configuration panel: enable/disable, thresholds, claim timeout

## References

- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#a3
""",
    },

    # ── Group B: Auto-Automation ──────────────────────────────────────────────
    {
        "title": "Feature: Auto-Accept Group Invites, Trades, Tasks, DZs, Anchors — MQ2AutoAccept parity",
        "labels": ["agent:ready", "p1-high", "automation", "multibox"],
        "body": """## Summary

MQ2AutoAccept automatically accepts incoming requests: group invites from trusted players, trade confirmations, task adds, expedition (DZ) adds, translocate requests, and primary/secondary anchor teleports. Essential for fully automated multibox setups where boxes cannot be manually managed.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Auto-accept group invites from configured trust list (or all)
- Auto-confirm trades from configured trust list
- Auto-accept task adds and DZ adds
- Auto-accept translocate and anchor requests
- Trust list configurable in web UI
- Per-type enable/disable toggles in web UI
- No acceptance from untrusted sources when trust-list mode is active

## References

- MQ2AutoAccept: https://github.com/RedGuides/MQ2AutoAccept
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#b1
""",
    },
    {
        "title": "Feature: Auto-Accept Resurrection Offers — MQ2Rez parity",
        "labels": ["agent:ready", "p1-high", "automation", "recovery"],
        "body": """## Summary

MQ2Rez automatically accepts resurrection offers based on configurable conditions: minimum rez XP%, caster trust list, zone restrictions. TextQuest has recovery.rs for camp recovery but does not explicitly handle auto-accepting rez offer popups.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Detect incoming resurrection popup
- Auto-accept if conditions met: XP% >= configured minimum AND caster in trust list
- Auto-decline if conditions not met (configurable)
- Configurable delay before accepting (to allow manual override window)
- Web UI configuration: min XP%, trust list, enable/disable, delay

## References

- MQ2Rez: https://github.com/RedGuides/MQ2Rez
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#b2
""",
    },
    {
        "title": "Feature: Auto-Camp to Desktop on Death to Preserve Rez Timer — MQ2AutoCamp parity",
        "labels": ["agent:ready", "p1-high", "automation", "recovery"],
        "body": """## Summary

MQ2AutoCamp automatically `/camp desktop` when a character dies, preventing the rez timer from counting down while unattended. The character re-logs after a configurable waiting period via the existing AutoLogin system. Prevents full-death XP loss during overnight unattended sessions.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Death event detected within one pulse
- Character issues `/camp desktop` after configurable delay (default: 30s to allow rez attempt)
- Re-login triggered via AutoLogin after configurable waiting period
- Alert sent to notification system before camping
- Web UI: enable/disable per character, delay configuration

## References

- MQ2AutoCamp: https://github.com/RedGuides/MQ2AutoCamp
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#b3
""",
    },

    # ── Group C: Safety and Awareness ────────────────────────────────────────
    {
        "title": "Feature: GM Detection and Zone-Wide Alert — MQ2GMCheck parity",
        "labels": ["agent:ready", "p0-critical", "safety", "alerts"],
        "body": """## Summary

MQ2GMCheck detects when a Game Master enters the zone (unless in stealth mode) and alerts all connected clients immediately. The existing alert system (#1567) handles operational alerts (death/stuck/resources) but does not specifically handle GM detection, which is a critical safety feature for any unattended session.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- GM spawn detection within one game pulse (< 1 second)
- All connected clients receive broadcast alert immediately
- Sound alert + TUI notification + web UI notification fires on GM detection
- Automation optionally auto-pauses when GM is in zone
- Automation resumes after GM leaves zone
- Web UI configuration: auto-pause toggle, notification channels

## References

- MQ2GMCheck: https://github.com/RedGuides/MQ2GMCheck
- Existing alert system: #1567
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#c1
""",
    },
    {
        "title": "Feature: Zone Entry/Exit Player Notifications — MQ2Paranoid / MQ2Spawns parity",
        "labels": ["agent:ready", "p1-high", "safety", "awareness"],
        "body": """## Summary

MQ2Paranoid announces when players zone in and out. MQ2Spawns provides a dedicated window for all spawn/despawn events. Essential for awareness during unattended farming in contested zones — know when competitors or potential threats enter your zone.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- PC zone-in and zone-out events announced in TUI within one pulse
- Filter by: all PCs, strangers only, friends only
- Dedicated spawn/despawn event stream panel in TUI
- Optional sound alert on PC zone-in
- Web UI configuration: filter mode, sound alert toggle, friend list

## References

- MQ2Paranoid: https://github.com/RedGuides/MQ2Paranoid
- MQ2Spawns: https://github.com/RedGuides/MQ2Spawns
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#c2
""",
    },
    {
        "title": "Feature: Rare Spawn Alert System — MQ2SpawnMaster parity",
        "labels": ["agent:ready", "p1-high", "automation", "combat"],
        "body": """## Summary

MQ2SpawnMaster alerts when specific named NPCs spawn in the zone. Users configure a watch list of rare spawn names. When any watched NPC appears, alerts fire via sound, chat, and notification system. Related to Named Tracking (#796) but focused specifically on the alert/notification aspect for spawn detection.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Named NPC spawn detected within one pulse
- Alert fires via: sound, TUI notification, web notification, optional broadcast to all clients
- Despawn notification when watched NPC is killed or de-spawns
- Time-of-spawn tracking (time since last pop)
- Web UI watch list management with spawn history

## References

- MQ2SpawnMaster: https://github.com/RedGuides/MQ2SpawnMaster
- Named tracking issue: #796
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#c3
""",
    },

    # ── Group D: Combat / Class Features ─────────────────────────────────────
    {
        "title": "Feature: Comprehensive Melee Skill and Disc Automation — MQ2Melee parity",
        "labels": ["agent:ready", "p1-high", "combat", "melee"],
        "body": """## Summary

MQ2Melee handles melee-specific automation beyond basic rotations: combat disciplines (disc scheduler), endurance management, auto-attack control, backstab positioning for rogues, enrage handling (turn off attack), and melee skill scheduling (kick, bash, slam, etc.). TextQuest's combat engine handles spell rotations well but lacks depth in melee-specific mechanics.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Combat discipline scheduler: activates discs in priority order with cooldown tracking
- Endurance management: skips endurance-consuming abilities when endurance is below threshold
- Auto-attack control: pauses attack during mez, on enrage, etc.
- Class-specific melee skill scheduling: kick, bash, slam, backstab, tiger claw, etc.
- Enrage detection + auto-attack pause to prevent mob fleeing
- Rogue backstab positioning (move behind mob when backstab available)
- All melee settings configurable per class in web UI

## References

- MQ2Melee: https://github.com/RedGuides/MQ2Melee
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#d1
""",
    },
    {
        "title": "Feature: Bard Song Scheduler, Twisting, and Instrument Swap — MQ2Medley / MQ2Twist / MQ2BardSwap parity",
        "labels": ["agent:ready", "p1-high", "combat", "bard"],
        "body": """## Summary

Bard automation requires three coordinated systems: a song scheduler with proper timing (MQ2Medley), song twisting for gem-cast cycling to maintain multiple songs simultaneously (MQ2Twist), and instrument swapping for skill bonuses (MQ2BardSwap). Together these make bard fully autonomous. TextQuest may have basic bard support in the combat engine but not this level of depth.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Song schedule definition: ordered list of songs with timing parameters
- Twist logic: automatic gem selection and re-cast timing to maintain song overlap
- Instrument slot management: equip correct instrument before casting song, restore after
- Instrument type mapping to song categories (string, brass, wind, percussion)
- Bard maintains 3+ songs simultaneously via twisting without dropped songs
- Song list configurable per situation from web UI with drag-and-drop ordering

## References

- MQ2Medley: https://github.com/RedGuides/MQ2Medley
- MQ2Twist: https://github.com/RedGuides/MQ2Twist
- MQ2BardSwap: https://github.com/RedGuides/MQ2BardSwap
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#d2
""",
    },
    {
        "title": "Feature: Ranger Headshot Automation — MQ2Headshot parity",
        "labels": ["agent:ready", "p2-medium", "combat", "ranger"],
        "body": """## Summary

Rangers have Headshot — a disc that instantly kills low-level NPCs when activated correctly. MQ2Headshot auto-activates the disc on valid targets at the right moment. High-value class-specific feature for ranger-based XP farming and named grinding in appropriate level ranges.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Headshot disc availability tracked (cooldown-aware)
- Auto-activate on valid target: level range check and HP threshold
- Integrates with existing ranger combat rotation
- Statistics: headshot attempts, successes, kills-via-headshot tracked in session
- Web UI: enable/disable, target level range, minimum target HP%

## References

- MQ2Headshot: https://github.com/RedGuides/MQ2Headshot
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#d3
""",
    },
    {
        "title": "Feature: Cross-Group and Outside-Group Assist — MQ2XAssist parity",
        "labels": ["agent:ready", "p2-medium", "combat", "multibox"],
        "body": """## Summary

MQ2XAssist enables a character to assist a Main Assist who is outside their group or raid. Essential for scenarios where the MA is not in the same group (cross-group raiding, solo boxes assisting a main, raid scenarios with split groups). TextQuest's orchestrator handles in-group assist but not cross-group targeting.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Configure a named character to assist (can be outside own group)
- Auto-target the configured MA's target
- Works in non-group, cross-group, and raid scenarios
- Takes priority over in-group assist target when enabled
- Web UI configuration per character: MA name, enable/disable

## References

- MQ2XAssist: https://github.com/RedGuides/MQ2XAssist
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#d4
""",
    },
    {
        "title": "Feature: Worst-Hurt Party/Pet Member Targeting for Healing — MQ2WorstHurt parity",
        "labels": ["agent:ready", "p2-medium", "combat", "healing"],
        "body": """## Summary

MQ2WorstHurt finds the most injured member of the party, extended target list, or pet and targets them. Core helper for non-CH-chain healing scenarios where the healer needs to react to the most hurt member in real time. Complements the existing CH chain coordinator.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Scan group members, XTarget list, and pet for lowest HP%
- Target the most injured member for healing
- Configurable scope: group only, group + XTarget, group + pet
- API exposed so combat engine's heal loop can call this
- Web UI configuration: scope selection, enable/disable

## References

- MQ2WorstHurt: https://github.com/RedGuides/MQ2WorstHurt
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#d5
""",
    },

    # ── Group E: Session Tracking ─────────────────────────────────────────────
    {
        "title": "Feature: XP and AA Experience Per-Hour Tracking — MQ2XPTracker parity",
        "labels": ["agent:ready", "p2-medium", "tracking", "session"],
        "body": """## Summary

MQ2XPTracker tracks experience gain over time, calculating XP/hour and AA/hour rates with session history. Essential for optimizing farm routes and measuring efficiency improvements. TextQuest has no XP tracking currently.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Track regular XP gain (% per hour) and AA XP gain (AA per hour)
- Session start/stop with pause support
- Historical data persisted per character per session
- TUI panel showing current rate and session summary
- Web UI panel with session history and rate trends

## References

- MQ2XPTracker: https://github.com/RedGuides/MQ2XPTracker
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#e1
""",
    },
    {
        "title": "Feature: Kill Count Tracking and Auto-Reporting — MQ2KillTracker parity",
        "labels": ["agent:ready", "p2-medium", "tracking", "session"],
        "body": """## Summary

MQ2KillTracker tracks kills per session and can auto-report kill counts every N minutes. Essential for measuring farm efficiency and reviewing overnight session performance.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Track kills per character per session (by mob name, zone, time)
- Calculate kills/hour rate
- Optional auto-report to configured chat channel every N minutes
- Session history stored per character
- TUI panel and web UI with session kill data

## References

- MQ2KillTracker: https://github.com/RedGuides/MQ2KillTracker
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#e2
""",
    },
    {
        "title": "Feature: Platinum Gain/Loss Session Tracking — MQ2PlatTracker parity",
        "labels": ["agent:ready", "p2-medium", "tracking", "economy"],
        "body": """## Summary

MQ2PlatTracker tracks platinum changes during a session (gains from selling loot, losses from repairs/purchases). Essential for measuring economy output and ensuring the economy system generates profit. Integrates with the existing economy/ledger module.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Track platinum, gold, silver, copper changes per transaction
- Session total and running rate (plat/hour)
- Integration with existing economy/ledger module
- TUI panel and web UI with session economy data

## References

- MQ2PlatTracker: https://github.com/RedGuides/MQ2PlatTracker
- Economy ledger module: textquest/src/economy/ledger.rs
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#e3
""",
    },

    # ── Group F: Alerts and Events ────────────────────────────────────────────
    {
        "title": "Feature: Sound-Based Alerts for Game Events — MQ2Sound parity",
        "labels": ["agent:ready", "p2-medium", "alerts", "ux"],
        "body": """## Summary

MQ2Sound plays audio alerts for configurable game events (low HP, target death, named spawn, GM detection, tell received, etc.). Sound alerts are essential for operators monitoring from a distance who need audio cues without watching screens. Currently no sound alerting exists in TextQuest.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Configurable sound events: low HP, death, named spawn, GM enter, tell received, custom patterns
- Play WAV file, MP3, or system beep per event
- Volume control and per-event enable/disable
- Integration with alert system (#1567) as a notification channel
- Web UI: event-to-sound file mapping, volume slider, mute-all toggle

## References

- MQ2Sound: https://docs.macroquest.org/plugins/community-plugins/mq2sound/
- Alert system: #1567
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#f1
""",
    },
    {
        "title": "Feature: Chat Output Logging to File Per Character — MQ2Log parity",
        "labels": ["agent:ready", "p2-medium", "logging", "observability"],
        "body": """## Summary

MQ2Log writes all MQ2 chat window output to a log file per character (server_charname.log). Essential for post-session debugging, audit trails, and reviewing what happened during overnight sessions.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- All MQ2 output written to `logs/server_charname.log`
- Optional: log regular EQ chat channels too
- Configurable log rotation (daily or by file size)
- Log level filtering (info/debug)
- Web UI: enable/disable per character, channel selection

## References

- MQ2Log: https://github.com/RedGuides/MQ2Log
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#f2
""",
    },
    {
        "title": "Feature: Chat Message Timestamps — MQ2Timestamp parity",
        "labels": ["agent:ready", "p3-low", "ux", "logging"],
        "body": """## Summary

MQ2Timestamp adds timestamps to all MQ2 chat messages, making it possible to correlate game events with log entries and understand timing during session reviews.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Configurable timestamp format prepended to all MQ2 chat messages (24h, 12h, with/without date)
- Enable/disable from web UI per character without restart

## References

- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#f3
""",
    },
    {
        "title": "Feature: User-Defined Pattern-Based Chat Event Triggers — MQ2Events / MQ2React parity",
        "labels": ["agent:ready", "p2-medium", "automation", "events"],
        "body": """## Summary

MQ2Events/MQ2React provide a general-purpose if-then system: define text patterns to match against incoming game messages, then define actions (run command, cast spell, send message). TextQuest's internal event_triggers.rs handles hard-coded events; this adds a user-configurable chat-pattern rule engine that requires no code changes to extend.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- User-definable rules: pattern (regex or string match) → action (command string)
- Event sources: any game chat channel, system messages, tells, emotes, say
- Action types: execute EQ command, send IPC command, trigger alert
- Rule priority ordering and enable/disable per rule
- Web UI rule editor with add/edit/delete; at least 50 active rules without performance impact
- Rules persist across sessions

## References

- MQ2Events: https://github.com/RedGuides/MQ2Events
- MQ2React: https://github.com/RedGuides/MQ2React
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#f4
""",
    },
    {
        "title": "Feature: Say Channel Detection and Alerting — MQ2Say parity",
        "labels": ["agent:ready", "p3-low", "automation", "events"],
        "body": """## Summary

MQ2Say detects specific text patterns in the /say channel and triggers alerts or actions. Used for detecting NPC dialogue (quest triggers, warning messages), player communication attempts, and GM messages arriving via /say. Complements the general event trigger system (F4) with say-channel-specific fast detection.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Pattern matches on /say channel detected within one pulse
- Alert fires via configured notification channels
- Configurable actions: alert, run command, broadcast
- Web UI rule editor

## References

- MQ2Say: https://github.com/RedGuides/MQ2Say
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#f5
""",
    },

    # ── Group G: Movement Extensions ─────────────────────────────────────────
    {
        "title": "Feature: Extended Movement Modes — makecamp, stick, circle (MQ2MoveUtils parity)",
        "labels": ["agent:ready", "p1-high", "navigation", "movement"],
        "body": """## Summary

MQ2MoveUtils provides movement commands essential to MQ2 workflows: `/makecamp` (establish camp radius and auto-return after combat), `/stick` (maintain melee range while following a moving target), `/circle` (move in circles around a point), and `/moveto` (navigate to coordinates). TextQuest's navigator handles navmesh pathfinding but lacks these specific movement modes that are core to MQ2 multibox operation.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- `makecamp`: establish camp point, auto-return after each pull/combat ends, configurable radius
- `stick`: maintain configurable distance from a moving target (melee range following)
- `circle`: smooth circular movement around a point at configurable radius and speed
- `moveto`: navigate to specific X/Y/Z coordinates with navmesh obstacle avoidance
- Commands exposed via IPC for script/macro control
- Camp point visible on zone map in web UI

## References

- MQ2MoveUtils: https://github.com/RedGuides/MQ2MoveUtils
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#g1
""",
    },
    {
        "title": "Feature: Relocation Item and AA Management — MQ2Relocate parity",
        "labels": ["agent:ready", "p2-medium", "navigation", "inventory"],
        "body": """## Summary

MQ2Relocate manages relocation items (teleport clickies) and relocation AAs (Throne of Heroes, etc.), providing easy `/relocate [destination]` commands for fast travel to known locations. Integrates with the travel module (#795) as a short-range/personal-travel complement.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Maintain list of relocation options: items and AAs with their known destinations
- Auto-select best travel option (prefer AA over clickie when both available)
- Cooldown status visible in web UI
- Integration with travel module (#795)

## References

- MQ2Relocate: https://github.com/RedGuides/MQ2Relocate
- Travel module: #795
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#g2
""",
    },
    {
        "title": "Feature: Ice-Surface Movement Physics Helper — MQ2Ice parity",
        "labels": ["agent:ready", "p3-low", "navigation", "velious"],
        "body": """## Summary

MQ2Ice helps with movement on ice surfaces (Velious content) where standard movement is compromised by slippery physics. Needed for farming Velious-era zones like Great Divide, Eastern Wastes, and the planes.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Detect ice-surface movement conditions
- Apply corrective movement adjustments for ice physics
- Web UI toggle enable/disable
- Does not interfere with normal movement on non-ice surfaces

## References

- MQ2Ice: https://github.com/RedGuides/MQ2Ice
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#g3
""",
    },

    # ── Group H: Inventory and Items ─────────────────────────────────────────
    {
        "title": "Feature: Extended Bandolier — Swap Any Equipment Slot (MQ2Bandolier parity)",
        "labels": ["agent:ready", "p2-medium", "inventory", "equipment"],
        "body": """## Summary

EQ's built-in bandolier only manages weapon slots. MQ2Bandolier extends this to all equipment slots, enabling complex gear swaps (tank → caster set, fishing gear, tradeskill gear, etc.) with a single command. TextQuest has equipment.rs for gear set management; verify full-slot coverage and add web UI configuration.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Named gear sets covering all equipment slots
- Single command activates a full gear set swap
- Multiple named sets per character
- Sets configurable in web UI
- Swap completes within 3 seconds

## References

- MQ2Bandolier: https://github.com/RedGuides/MQ2Bandolier
- Equipment module: textquest/src/camp/equipment.rs
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#h1
""",
    },
    {
        "title": "Feature: Cursor Item Management with Quantity Rules — MQ2Cursor parity",
        "labels": ["agent:ready", "p2-medium", "inventory", "loot"],
        "body": """## Summary

MQ2Cursor provides rule-based handling for items that appear on the cursor: auto-keep up to N of an item, destroy/drop/consume extras, always-keep or always-destroy specific items. Essential for fully automated looting where the cursor can get stuck on unwanted items and halt the session.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Rule-based cursor handling: item name → action (keep N, destroy, drop, consume)
- Auto-process cursor items without manual intervention
- Integration with loot system (auto-loot creates cursor items)
- Web UI rule editor with keep/destroy/drop actions and quantity thresholds
- Cursor items processed within one pulse of appearing

## References

- MQ2Cursor: https://github.com/RedGuides/MQ2Cursor
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#h2
""",
    },
    {
        "title": "Feature: Item Link Database for Items Not in Inventory — MQ2LinkDB / MQ2FakeLink parity",
        "labels": ["agent:ready", "p3-low", "items", "economy"],
        "body": """## Summary

MQ2LinkDB maintains a database of item IDs to generate item links for items not currently in inventory. Useful for price discussion in trade chat, economy research, and bazaar analysis. MQ2FakeLink provides similar functionality.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Item link database sourced from EQ data or community DB
- Generate item links by item name or ID via command
- Links display correctly in game chat
- Command accessible from IPC for scripting

## References

- MQ2LinkDB: https://github.com/RedGuides/MQ2LinkDB
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#h3
""",
    },
    {
        "title": "Feature: Item Upgrade Scoring and Comparison Engine — MQ2ItemScore parity",
        "labels": ["agent:ready", "p2-medium", "items", "loot"],
        "body": """## Summary

MQ2ItemScore rates whether a looted item is an upgrade for the character/class based on configurable stat weights. Essential for auto-loot keep/sell decisions during farming when evaluating a looted item's value relative to equipped gear.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Per-class stat weight configuration (STR, AGI, STA, etc.)
- Score calculated for any item vs. currently equipped item in same slot
- Score API available to loot module for automated keep/sell decisions
- Web UI: stat weight configuration per class

## References

- MQ2ItemScore: https://github.com/RedGuides/MQ2ItemScore
- Loot module: textquest/src/camp/loot.rs
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#h4
""",
    },
    {
        "title": "Feature: Food and Drink Auto-Consumption — MQ2FeedMe parity",
        "labels": ["agent:ready", "p2-medium", "automation", "survival"],
        "body": """## Summary

MQ2FeedMe monitors hunger/thirst status and automatically uses food/drink items from inventory when needed. Characters that run out of food lose stat bonuses, impacting farming efficiency during long sessions.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Monitor hunger and thirst status each pulse
- Auto-consume food/drink from inventory when hungry/thirsty
- Configurable preferred food/drink items (choose best available)
- Alert when food/drink supply drops below configured threshold
- Web UI: enable/disable, preferred item names, low-supply threshold

## References

- MQ2FeedMe: https://github.com/RedGuides/MQ2FeedMe
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#h5
""",
    },

    # ── Group I: Target and Spawn UI ──────────────────────────────────────────
    {
        "title": "Feature: Overhead Target Direction Compass Overlay — MQ2OTD parity",
        "labels": ["agent:ready", "p3-low", "ui", "navigation"],
        "body": """## Summary

MQ2OTD displays a compass in the EQ HUD showing the direction to the current target. Useful for locating spawns and navigating to targets when the map panel is not sufficient. Complements the existing tactical map module.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Calculate heading from player position to current target
- Display direction indicator in TUI map panel or as HUD overlay
- Updates within one pulse as positions change (real-time)

## References

- MQ2OTD: https://github.com/RedGuides/MQ2OTD
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#i1
""",
    },
    {
        "title": "Feature: Filterable Sortable Spawn Finder — MQ2SpawnSort parity",
        "labels": ["agent:ready", "p2-medium", "ui", "spawns"],
        "body": """## Summary

MQ2SpawnSort provides a searchable, sortable list of all zone spawns by any variable (name, level, distance, type, HP%). More powerful than the basic spawn list in the TUI. Essential for quickly locating specific mobs during farming.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Spawn list sortable by: name, level, class, race, distance, HP%
- Text search filter that updates in real-time
- Click-to-target from spawn list (where applicable in TUI)
- Web UI spawn finder panel accessible from web dashboard

## References

- MQ2SpawnSort: https://github.com/RedGuides/MQ2SpawnSort
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#i2
""",
    },
    {
        "title": "Feature: PC Proximity Friend/Stranger Detector — MQ2Posse parity",
        "labels": ["agent:ready", "p2-medium", "safety", "awareness"],
        "body": """## Summary

MQ2Posse checks for player characters in a defined radius and identifies them as friends or strangers based on a configurable friend list. Used for safety awareness — alert when unknown players approach the farming camp during unattended sessions.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Configurable detection radius
- Friend list management (known players, guild members)
- Alert when stranger enters radius
- Alert when any PC enters radius (strict mode)
- Integration with existing GM check and spawn notification
- Web UI: friend list management, radius configuration

## References

- MQ2Posse: https://github.com/RedGuides/MQ2Posse
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#i3
""",
    },

    # ── Group J: Group and Raid Management ───────────────────────────────────
    {
        "title": "Feature: Auto Group Creation with Role Assignment — MQ2AutoGroup parity",
        "labels": ["agent:ready", "p2-medium", "automation", "group"],
        "body": """## Summary

MQ2AutoGroup automatically creates a group, invites specific characters, assigns group roles (tank, healer, etc.), and then runs a configurable command on completion. Eliminates manual group forming for automated sessions.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Invite specific characters to group in configured order
- Assign group roles after all members join
- Execute configurable command after group is formed
- Handle invite failures with retry
- Web UI: member list, role assignments, completion command

## References

- MQ2AutoGroup: https://github.com/RedGuides/MQ2AutoGroup
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#j1
""",
    },
    {
        "title": "Feature: Raid DZ/Task Add-All Utilities — MQ2RaidUtils parity",
        "labels": ["agent:ready", "p2-medium", "raid", "automation"],
        "body": """## Summary

MQ2RaidUtils makes raid leadership easier by enabling all-member DZ/task add and remove in a single command. Eliminates the need to individually add each raider, which is especially important when managing large multibox raid groups.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Single command adds all raid members to a DZ
- Single command adds all raid members to a task
- Remove equivalents for both
- Failed adds logged and reported
- Web UI: one-click add-all buttons for DZ and task

## References

- MQ2RaidUtils: https://github.com/RedGuides/MQ2RaidUtils
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#j2
""",
    },
    {
        "title": "Feature: Raid Random Roll Helper — MQ2Rand parity",
        "labels": ["agent:ready", "p3-low", "raid", "ux"],
        "body": """## Summary

MQ2Rand is a /random helper for raid leaders to determine loot winners for GDKP and loot distribution systems. Tracks participants in a roll round, announces results, and maintains history. Essential for any raid leadership workflow.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Track participants in a configurable roll round
- Announce roll results to raid/group automatically
- Roll history accessible in web UI
- Multiple simultaneous roll sets supported

## References

- MQ2Rand: https://github.com/RedGuides/MQ2Rand
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#j3
""",
    },
    {
        "title": "Feature: Mission Reward Selection Automation — MQ2Rewards parity",
        "labels": ["agent:ready", "p2-medium", "automation", "economy"],
        "body": """## Summary

MQ2Rewards allows specifying, selecting, and claiming rewards from missions/tasks via configurable rules. Enables automated task farming where reward selection happens automatically without operator input.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Define preferred reward by name or position per task type
- Reward window detected and preference applied automatically on task completion
- Per-task reward configuration in web UI
- Fallback to first reward if preferred is unavailable

## References

- MQ2Rewards: https://github.com/RedGuides/MQ2Rewards
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#j4
""",
    },
    {
        "title": "Feature: Raid Availability HUD Dashboard — raidhud Lua parity",
        "labels": ["agent:ready", "p3-low", "raid", "ui"],
        "body": """## Summary

The RedGuides `raidhud` Lua script provides a lightweight dashboard showing which raid instances are currently available based on reset timers. Useful for daily/weekly raid planning.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Track raid availability by expansion/instance reset timers
- Web dashboard panel showing available raids
- Alert when a watched raid instance resets and becomes available
- Configurable watch list of target raids

## References

- raidhud: https://github.com/RedGuides/raidhud
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#j5
""",
    },

    # ── Group K: Economy and Subscription Utilities ───────────────────────────
    {
        "title": "Feature: Tribute Management Automation — MQ2TributeManager parity",
        "labels": ["agent:ready", "p2-medium", "automation", "economy"],
        "body": """## Summary

MQ2TributeManager automates tribute timer management: monitors when tribute expires and auto-activates it. TextQuest's collectibles.rs mentions tribute but full automation (timer tracking + auto-activate + web UI) is not confirmed complete.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Monitor tribute timer; auto-activate when tribute expires
- Preferred tribute selections configurable per character in web UI
- Tribute point balance visible in web dashboard
- Alert when tribute is about to expire

## References

- MQ2TributeManager: https://github.com/RedGuides/MQ2TributeManager
- Collectibles module: textquest/src/camp/collectibles.rs
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#k1
""",
    },
    {
        "title": "Feature: Tradeskill Trophy Management — MQ2TSTrophy parity",
        "labels": ["agent:ready", "p3-low", "tradeskill", "inventory"],
        "body": """## Summary

MQ2TSTrophy manages tradeskill trophy items that provide crafting bonuses. Items need to be equipped before crafting and removed after. Needed for characters that do tradeskill work during downtime periods in the farming loop.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Auto-equip tradeskill trophy before crafting attempts
- Remove trophy after crafting session completes
- Track trophy charges (some have limited uses)
- Web UI enable/disable toggle with trophy item name configuration

## References

- MQ2TSTrophy: https://github.com/RedGuides/MQ2TSTrophy
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#k2
""",
    },
    {
        "title": "Feature: DBCash All-Access Subscription Auto-Claim — MQ2AutoClaim parity",
        "labels": ["agent:ready", "p3-low", "automation", "subscription"],
        "body": """## Summary

MQ2AutoClaim automatically claims DBCash from all-access membership, handling the claim popup without manual intervention. Small QoL but required for fully automated sessions that run without operator attention.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- DBCash claim popup detected and handled automatically
- Claim amount logged per character per session
- Web UI enable/disable toggle

## References

- MQ2AutoClaim: https://github.com/RedGuides/MQ2AutoClaim
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#k3
""",
    },
    {
        "title": "Feature: Vendor Item Search and Price Alerts in Web UI — MQ2Vendors parity",
        "labels": ["agent:ready", "p2-medium", "economy", "web-ui"],
        "body": """## Summary

MQ2Vendors alerts when browsing a vendor and a watched item appears for sale. TextQuest already has vendor cycle logic for selling, but not for monitoring vendor stock for specific items. Configurable via web UI per the user's requirement that all RedGuides macro features be configurable there.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Watch list of items to look for on vendors
- Alert fires when watched item spotted on vendor during normal vendor interactions
- Price comparison (expected vs. actual vendor price) where known
- Web UI watch list editor with item names and max acceptable price

## References

- MQ2Vendors: https://github.com/RedGuides/MQ2Vendors
- Vendor cycle: textquest/src/camp/vendor.rs
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#k4
""",
    },

    # ── Group L: System and Integration ──────────────────────────────────────
    {
        "title": "Feature: Auto-Resize Characters/NPCs to Minimum Size — MQ2AutoSize parity",
        "labels": ["agent:ready", "p3-low", "ux", "visibility"],
        "body": """## Summary

MQ2AutoSize shrinks all characters and NPCs in range to minimum allowed size. Improves visibility in cramped dungeons and reduces visual clutter during large mob pulls. Simple quality-of-life feature often needed in tight content.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Auto-size PCs, NPCs, self, and pets independently configurable
- Configurable size percentage (0–100%)
- Web UI toggle with size configuration
- Does not interfere with targeting or combat

## References

- MQ2AutoSize: https://github.com/RedGuides/MQ2AutoSize
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#l1
""",
    },
    {
        "title": "Feature: CPU Load Balancer for Multi-EQ-Window Setups — MQ2CPULoad parity",
        "labels": ["agent:ready", "p2-medium", "performance", "system"],
        "body": """## Summary

MQ2CPULoad assigns the focused (foreground) EQ window its own dedicated CPU core and distributes other instances across remaining cores. This significantly improves responsiveness of the active window during manual play in a multibox setup.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Active (foreground) EQ window assigned to dedicated CPU core via process affinity
- Other windows distributed evenly across remaining cores
- Affinity updates when window focus changes
- Web UI: enable/disable, current core assignment visible per client

## References

- MQ2CPULoad: https://github.com/RedGuides/MQ2CPULoad
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#l2
""",
    },
    {
        "title": "Feature: EQ Window Title Customization Per Character — MQ2WinTitle parity",
        "labels": ["agent:ready", "p2-medium", "ux", "system"],
        "body": """## Summary

MQ2WinTitle changes the EQ window title based on config to show character name and server. Essential for identifying windows when managing 6–36 EQ instances from the OS taskbar. Without this, all windows show the same title.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Configurable title format: e.g. `[server] charactername (level class)`
- Title updates on character load and zone change
- Format string configurable per character in web UI

## References

- MQ2WinTitle: https://github.com/RedGuides/MQ2WinTitle
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#l3
""",
    },
    {
        "title": "Feature: Unified Box Controller Interface — MQ2Boxr parity",
        "labels": ["agent:ready", "p1-high", "multibox", "orchestration"],
        "body": """## Summary

MQ2Boxr provides a unified command interface to control boxes running different automation software (CWTN, KissAssist, RGMercs, Entropy, etc.) via standardized commands: Pause, Unpause, Camp, Chase, Manual, BurnNow. For TextQuest, this means providing a standard control API broadcastable to all characters regardless of individual automation configuration, plus a web UI global control panel.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Standard commands: Pause, Unpause, Camp, Chase, Manual, BurnNow, RaidAssistNum
- Commands broadcastable to all connected clients via the box chat server (Group A)
- Per-client automation state visible in web dashboard
- Web UI: global control panel with Pause All / Unpause All / Camp All / Chase All buttons
- State transitions happen within one pulse of command receipt

## References

- MQ2Boxr: https://github.com/RedGuides/MQ2Boxr
- Box chat server: see Group A issues
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#l4
""",
    },
    {
        "title": "Feature: Discord Webhook Integration — MQ2Discord parity",
        "labels": ["agent:ready", "p2-medium", "integration", "alerts"],
        "body": """## Summary

MQ2Discord sends alerts to Discord via webhooks. The alert system (#1567) mentions Discord as an alert channel, but this requires a complete implementation of the Discord webhook client with proper message formatting, rate limiting, embed support, and per-alert-type channel routing.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Discord webhook client (HTTP POST to webhook URL)
- Message types: plain text and rich embed with color/fields/title
- Rate limiting: respect Discord's 30 requests/minute per webhook limit
- Per-event-type configuration: death → @everyone ping, info → no ping
- Multiple webhook URLs configurable (different channels for different alert severities)
- Web UI: webhook URL management, notification level per event type
- Death alerts appear in Discord within 5 seconds

## References

- MQ2Discord: https://github.com/RedGuides/MQ2Discord
- Alert system: #1567
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#l5
""",
    },
    {
        "title": "Feature: In-Game Clipboard Copy — MQ2Clipboard parity",
        "labels": ["agent:ready", "p3-low", "ux", "utility"],
        "body": """## Summary

MQ2Clipboard copies in-game text and MQ command output to the OS clipboard. Useful for pasting item stats, character info, or command output into external analysis tools or documentation.

Parent epic: see MQ2 Coverage Gap Analysis doc.

## Acceptance

- Command to copy last MQ2 output to OS clipboard
- Optional: copy any in-game text string to clipboard
- Clipboard history (last N items) visible in web UI
- Works within DLL injection context on Windows

## References

- MQ2Clipboard: https://github.com/RedGuides/MQ2Clipboard
- MQ2 Coverage Gap Analysis: docs/MQ2_COVERAGE_GAP_ANALYSIS.md#l6
""",
    },
]


def main() -> None:
    token = os.environ.get("GITHUB_TOKEN", "")
    if not token:
        print("ERROR: GITHUB_TOKEN environment variable not set.")
        print("Set it to a token with 'issues: write' permission and re-run.")
        return

    print(f"Creating {len(ISSUES)} issues in {REPO} ...\n")
    created = []
    for issue in ISSUES:
        result = create_issue(token, issue["title"], issue["body"], issue.get("labels"))
        if result:
            created.append(result["number"])
        time.sleep(1)  # Respect API rate limits

    print(f"\nDone. Created {len(created)} issues: {created}")


if __name__ == "__main__":
    main()
