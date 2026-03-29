# Frostreaver Orchestration Architecture

## Overview

The orchestrator manages 36 EQ clients across 6 groups, providing command & control from the TUI, Discord, or automated camp loops. This document defines the group management system, command interface, and revised milestone phases.

## Group Model

### Config (`config/frostreaver.toml`)
```toml
[[groups]]
id = 1
name = "Alpha"
accounts = ["frostreaver01", "frostreaver02", "frostreaver03", "frostreaver04", "frostreaver05", "frostreaver06"]
default_camp = "crushbone_entrance"

[groups.roles]
frostreaver01 = "tank"       # WAR — pulls, holds aggro
frostreaver02 = "healer"     # CLR — heals tank, rezzes
frostreaver03 = "cc"         # ENC — mezzes adds, haste, clarity
frostreaver04 = "puller"     # BRD — pulls with melody, speed
frostreaver05 = "dps"        # RNG — assist MA, DPS burn
frostreaver06 = "dps"        # RNG — assist MA, DPS burn
```

### Group States
| State | Members Online | Behavior |
|-------|---------------|----------|
| Full (6/6) | All present | Normal camp cycle: pull → tank → heal → CC → DPS → loot |
| Strong (4-5/6) | Missing 1-2 | Reduced pull rate, skip CC if enchanter missing |
| Skeleton (2-3/6) | Missing 3-4 | Light camp or travel-only (no combat) |
| Solo (1/6) | Only 1 logged in | Safe tasks: travel, sell, bank, buff, med |
| Offline (0/6) | None logged in | Skip entirely |

### Group Lifecycle
1. **Launch**: `launch_eq.bat` starts 6 clients per group
2. **Inject**: `dmft.exe --inject` injects DLL into all running clients
3. **Discover**: Orchestrator matches PID → character name via window title or memory read
4. **Form**: Leader invites members, all accept
5. **Assign**: Load camp config, assign roles
6. **Run**: Camp loop executes autonomously
7. **Monitor**: TUI dashboard shows real-time status per group

## TUI Command Interface

### Screen Layout
```
┌─ EQ Multibox Controller ─────────────────────────────────────────┐
│ 6x EQ | [1/6] Camrene | Firiona Vie | Freeport                  │
│ 1:Dashboard  2:Spawns  3:Character  4:Map  5:Groups  6:Command   │
├──────────────────────────────────────────────────────────────────┤
│ Group 1 (Alpha)     │ Group 2 (Bravo)     │ Group 3 (Charlie)    │
│ ✓ Camrene   WAR  60 │ ✓ Toon07    CLR  55 │ ✓ Toon13    WAR  48 │
│ ✓ Zisdarenu CLR  58 │ ✓ Toon08    ENC  52 │ ○ Toon14    ---  -- │
│ ✓ Toon03    ENC  55 │ ○ Toon09    ---  -- │ ...                  │
│ ...                  │ ...                  │                      │
│ Status: Camping      │ Status: Traveling    │ Status: 2/6 Online   │
├──────────────────────────────────────────────────────────────────┤
│ Group 4 (Delta)     │ Group 5 (Echo)      │ Group 6 (Foxtrot)    │
│ ...                  │ ...                  │ ...                  │
├──────────────────────────────────────────────────────────────────┤
│ : _                                                    [PRIVATE]  │
└──────────────────────────────────────────────────────────────────┘
```

### Command Bar (`:` mode)
Press `:` to enter command mode. Commands:

**Group commands** (target a group):
```
:G1 camp crushbone_entrance    — Send Group 1 to a saved camp
:G1 travel pok                 — Navigate Group 1 to Plane of Knowledge
:G1 follow Camrene             — All G1 members follow Camrene
:G1 recall                     — All G1 members gate/bind
:G1 stop                       — Stop all automation for G1
:G1 sell                       — Send G1 to nearest vendor to sell
:G1 bank                       — Send G1 to bank
```

**All-groups commands:**
```
:all camp                      — All groups go to their default camps
:all stop                      — Emergency stop everything
:all recall                    — Everyone gates
:all sell                      — Everyone sells
```

**Individual commands:**
```
:Camrene /sit                  — Send slash command to specific character
:Camrene follow Zisdarenu      — One character follows another
```

**Camp management:**
```
:camp save crushbone_entrance  — Save current position as a camp
:camp list                     — Show all saved camps
:camp info crushbone_entrance  — Show camp details
```

**System:**
```
:status                        — Show all group statuses
:inject                        — Inject DLL into any un-injected clients
:eject                         — Remove DLL from all clients
```

### Discord Commands (same syntax)
Messages in the Discord channel are parsed as commands:
```
G1 camp crushbone_entrance
all stop
status
```

## Camp System

### Camp Config (`config/camps/crushbone_entrance.toml`)
```toml
name = "Crushbone Entrance"
zone = "crushbone"
camp_center = [500.0, -200.0, 3.0]
pull_point = [550.0, -180.0, 3.0]
pull_radius = 150.0
camp_radius = 30.0
leash_radius = 200.0       # Max distance from camp before returning
rest_mana_pct = 20          # Sit to med below this %
pull_mana_pct = 60          # Don't pull until healer above this %
level_range = [1, 15]       # Expected mob levels
```

### Camp Loop (per group)
```
loop {
    1. CHECK: Is everyone alive? Rez if needed.
    2. CHECK: Is healer mana > pull_mana_pct? If not, med.
    3. PULL: Puller targets mob within pull_radius, attacks, runs to camp_center.
    4. TANK: Tank taunts/attacks when mob reaches camp.
    5. CC: Enchanter mezzes any adds.
    6. HEAL: Cleric heals tank (reactive, priority-based).
    7. DPS: Rangers /assist tank, attack.
    8. LOOT: When mob dead, nearest character loots.
    9. MED: If mana low, sit. Stand when pull_mana_pct reached.
    10. REPEAT.

    // Periodic:
    - Every 30 min: check bags, sell if full
    - Every 60 min: rebuff group
    - On named spawn: alert Discord, prioritize kill
}
```

## Revised Milestone Phases

Based on tonight's breakthroughs (DLL injection working, slash commands confirmed) and the roadmap review recommendation to "stop building, start integrating":

### Phase 1: Command Foundation (THIS WEEK)
**Goal**: Reliable command pipeline for 2 characters

- [x] DLL injection into live eqgame.exe
- [x] InterpretCmd slash command execution
- [x] IPC named pipe command delivery
- [x] Group invite via /invite
- [x] Follow via /target + /follow
- [ ] Window renaming (DLL renames to character name)
- [ ] PID → character name mapping
- [ ] Zone name reading (for map + status)
- [ ] TUI command bar (`:` mode)
- [ ] cmd_all.bat broadcast working

### Phase 2: Combat Loop (WEEK 2)
**Goal**: 2 characters killing mobs in a newbie zone

- [ ] /assist + /attack automation
- [ ] Basic heal loop (cleric watches tank HP, casts heal)
- [ ] Sit/med when mana low
- [ ] Pull cycle (target mob, attack, return to camp)
- [ ] Loot corpses
- [ ] Death recovery (rez or respawn + rebuff)

### Phase 3: 6-Box Group (WEEK 3)
**Goal**: Full group operating a camp

- [ ] 6-client launch + inject + auto-group
- [ ] Role assignment (tank/healer/CC/DPS)
- [ ] Camp config save/load
- [ ] KissAssist-style priority ability lists (per-class TOML config)
- [ ] Enchanter mez adds
- [ ] Bard twist engine
- [ ] Auto-buff maintenance

### Phase 4: Multi-Group + Economy (WEEK 4-5)
**Goal**: Multiple groups farming simultaneously

- [ ] 36-client management (6 groups)
- [ ] Group dashboard screen in TUI
- [ ] Discord command relay
- [ ] Auto-sell / auto-bank cycle
- [ ] Loot rules (need/greed/destroy)
- [ ] Render strobing for background clients
- [ ] INI optimization deployed
- [ ] Memory limits via Job Objects

### Phase 5: Navigation + Zone Travel (WEEK 5-6)
**Goal**: Characters can travel between zones autonomously

- [ ] Navmesh loading (MQ2Nav format or custom)
- [ ] Zone-to-zone pathing (PoK books, zone lines)
- [ ] Stuck detection + recovery
- [ ] Door clicking
- [ ] Elevator/lift handling

### Phase 6: Anti-Detection + Hardening (WEEK 6-7)
**Goal**: Survive on a live/TLP server

- [ ] Warden anti-cheat research
- [ ] String stripping (remove "DMFT" from DLL)
- [ ] Randomized timing (humanization)
- [ ] GM detection (watch for GM spawns)
- [ ] Innocent mode (stop automation on GM detect)
- [ ] Process name randomization
- [ ] Crash recovery + auto-reconnect

### Phase 7: Polish + TLP Launch (WEEK 7+)
**Goal**: Ready for Frostreaver TLP server launch

- [ ] All 36 accounts configured
- [ ] All class strategies implemented
- [ ] Camp database for launch zones
- [ ] Krono farming loop tested
- [ ] Monitoring dashboard
- [ ] Discord alerts (deaths, named spawns, bags full)
- [ ] Performance tuning (36 clients stable)

## Architecture Notes

- **Config-driven**: Groups, roles, camps, abilities all in TOML. No recompile to change behavior.
- **DLL is dumb**: DLL only executes commands it receives. All intelligence is in the orchestrator.
- **Orchestrator is smart**: Camp loops, role logic, mana checks, pull decisions all run in the Rust orchestrator, sending slash commands via IPC.
- **Graceful degradation**: If a character dies, DCs, or zones, the group adjusts automatically.
- **Observable**: Everything logged, TUI shows real-time state, Discord gets alerts.
