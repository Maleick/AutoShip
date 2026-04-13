# TextQuest ↔ RGMercs Feature Parity & Lua/MacroQuest Support

**Status**: Planning phase | **Epic Issue**: [#790](https://github.com/Maleick/TextQuest/issues/790)

This document tracks TextQuest's path to feature parity with [rgmercs](https://github.com/DerpleDude/rgmercs) while adding Lua scripting and MacroQuest plugin support. The goal is to enable TextQuest to run rgmercs alongside DLL-based automation and provide a unified platform for EQ multiboxing.

---

## Overview

**RGMercs** is a Lua-based combat automation framework maintained by the RedGuides community. It provides:
- **16 core modules** covering combat, travel, loot, and quality-of-life features
- **Pre-configured classes** with sensible defaults for new users
- **In-game GUI windows** for spell rotations, pull targeting, and consumable management
- **Customizable Lua configs** for experienced users
- **Community-driven development** with 8+ years of refinement

**TextQuest** currently provides:
- **External TUI orchestrator** for live dashboard and group management
- **DLL injection** with game hooks for direct function calls
- **Combat automation** via ClassStrategy trait (17 classes)
- **Navigation FSM** with waypoint pathfinding and stuck detection
- **Web dashboard** for credentials and group config

### Key Differences
| Feature | RGMercs | TextQuest |
|---------|---------|-----------|
| **Scripting** | Lua only | Rust binary (no scripting) |
| **Plugin Support** | MacroQuest .dll/.mq2 | None (proprietary) |
| **UI** | In-game overlay GUI | External TUI |
| **Architecture** | Client-side Lua | External orchestrator + DLL |
| **Modules** | 16 (modular) | 4 (monolithic crates) |
| **State** | Game memory direct access | IPC-based coordination |

---

## Implementation Roadmap

### Phase 1: Scripting Foundation (Dependency for all others)
Enables Lua scripts and MacroQuest plugins to execute and interact with TextQuest.

| Issue | Feature | Status | Notes |
|-------|---------|--------|-------|
| [#791](https://github.com/Maleick/TextQuest/issues/791) | Lua 5.4 VM integration | 📋 Ready | mlua crate, sandbox, hot-reload |
| [#792](https://github.com/Maleick/TextQuest/issues/792) | MacroQuest plugin loader | 📋 Ready | DLL/MQ2 loading, API bridge, FFI |
| [#793](https://github.com/Maleick/TextQuest/issues/793) | In-game hotkey system | 📋 Ready | Script/plugin command registration |

**Blockers**: None
**Timeline**: 2-3 weeks (parallel)

---

### Phase 2: Core Combat Modules
Implement rgmercs' combat-related features.

| Issue | RGMercs Module | Status | Notes |
|-------|---|---------|-------|
| [#794](https://github.com/Maleick/TextQuest/issues/794) | Clickies | 📋 Ready | Consumable automation + UI |
| [#799](https://github.com/Maleick/TextQuest/issues/799) | Enhanced charm/pets | 📋 Ready | Pet management + multi-pet support |
| [#800](https://github.com/Maleick/TextQuest/issues/800) | Improved pull system | 📋 Ready | Directional pulling, patterns |
| [#796](https://github.com/Maleick/TextQuest/issues/796) | Named tracking | 📋 Ready | Named NPC database, spawn prediction |

**Blockers**: Phase 1 (scripting)
**Timeline**: 2-3 weeks
**Integrates With**: textquest/src/combat/, textquest-dll/src/combat/

---

### Phase 3: Movement & Logistics
Implement travel and loot management features.

| Issue | RGMercs Module | Status | Notes |
|-------|---|---------|-------|
| [#795](https://github.com/Maleick/TextQuest/issues/795) | Travel | 📋 Ready | Portal coordination, group travel |
| [#798](https://github.com/Maleick/TextQuest/issues/798) | SmartLoot + LootNScoot | 📋 Ready | Smart filtering, zone timers |
| [#797](https://github.com/Maleick/TextQuest/issues/797) | Drag | 📋 Ready | Corpse/object movement |

**Blockers**: Phase 1 (scripting)
**Timeline**: 2-3 weeks
**Integrates With**: textquest/src/nav/, textquest/src/loot/

---

### Phase 4: Quality of Life & Monitoring
Implement observability, debugging, and user experience features.

| Issue | RGMercs Module | Status | Notes |
|-------|---|---------|-------|
| [#801](https://github.com/Maleick/TextQuest/issues/801) | In-game GUI | 📋 Ready | Overlay UI windows, widgets |
| [#802](https://github.com/Maleick/TextQuest/issues/802) | Performance monitoring | 📋 Ready | DPS, loot/hr, system metrics |
| [#803](https://github.com/Maleick/TextQuest/issues/803) | Debug tools | 📋 Ready | Memory inspection, script debugging |
| [#804](https://github.com/Maleick/TextQuest/issues/804) | In-game help/FAQ | 📋 Ready | Searchable commands, tips |

**Blockers**: Phase 1 (scripting)
**Timeline**: 2-3 weeks
**Integrates With**: textquest/src/tui/, textquest/src/metrics/, textquest-dll/

---

## RGMercs Module Mapping

### Implemented in TextQuest ✅
| RGMercs | TextQuest Equivalent | Status |
|---------|---|--------|
| **base.lua** | orchestrator.rs, client.rs | ✅ Complete |
| **class.lua** | ClassStrategy trait (17 classes) | ✅ Complete |
| **mez.lua** | CC assignment in combat | ✅ Complete |
| **move.lua** | Navigator FSM | ✅ Complete |
| **smartloot.lua** | loot/ module (wishlists) | ✅ Complete (expanding) |
| **performance.lua** | metrics/ module | ✅ Partial |
| **debug.lua** | Logging system | ✅ Partial |

### New in This Feature ➕
| RGMercs | TextQuest Implementation | GitHub Issue |
|---------|---|---|
| **clickies.lua** | Clickies module | [#794](https://github.com/Maleick/TextQuest/issues/794) |
| **charm.lua** | Enhanced charm/pet mgmt | [#799](https://github.com/Maleick/TextQuest/issues/799) |
| **pull.lua** | Improved pull system | [#800](https://github.com/Maleick/TextQuest/issues/800) |
| **named.lua** | Named NPC tracking | [#796](https://github.com/Maleick/TextQuest/issues/796) |
| **travel.lua** | Travel module | [#795](https://github.com/Maleick/TextQuest/issues/795) |
| **drag.lua** | Drag module | [#797](https://github.com/Maleick/TextQuest/issues/797) |
| **lootnscoot.lua** | Smart loot + LootNScoot | [#798](https://github.com/Maleick/TextQuest/issues/798) |
| *In-game GUI* | GUI overlay system | [#801](https://github.com/Maleick/TextQuest/issues/801) |
| *In-game hotkeys* | Hotkey system | [#793](https://github.com/Maleick/TextQuest/issues/793) |

### Scripting & Plugins ⚙️
| Component | TextQuest Implementation | GitHub Issue |
|-----------|---|---|
| **Lua VM** | Lua 5.4 integration | [#791](https://github.com/Maleick/TextQuest/issues/791) |
| **Plugin loader** | MacroQuest .dll/.mq2 support | [#792](https://github.com/Maleick/TextQuest/issues/792) |
| **Command interface** | In-game hotkey system | [#793](https://github.com/Maleick/TextQuest/issues/793) |

---

## Architecture Decisions

### 1. **Where Does Lua Run?**
- **Option A**: DLL context (faster, direct memory, but less safe)
- **Option B**: Orchestrator context (safer, but IPC overhead)
- **Decision**: Both. Lightweight scripts in DLL, heavy logic in orchestrator via IPC.

### 2. **How Does Lua Access Game State?**
- **Option A**: Re-export all TextQuest APIs (spawn list, player, group state)
- **Option B**: Let Lua read EQ memory directly (like rgmercs)
- **Decision**: Option A + memory inspection tools (safer + works cross-platform)

### 3. **Plugin Compatibility**
- **Option A**: Implement full MQ2 plugin API (huge effort)
- **Option B**: Create a shim layer (MQ2 calls → TextQuest API)
- **Decision**: Option B - gradual compatibility, focus on most-used plugins first

### 4. **In-Game GUI**
- **Option A**: Separate overlay process (complex, but isolated)
- **Option B**: Direct3D hook in DLL (faster, simpler, but integrated)
- **Decision**: Option B - similar to existing hook architecture

---

## Testing Strategy

### Unit Tests
- [ ] Lua VM initialization and error handling
- [ ] Script sandbox restrictions
- [ ] Plugin loader and DLL symbol resolution
- [ ] Hotkey parsing and execution
- [ ] Each module's core logic

### Integration Tests
- [ ] Load rgmercs + TextQuest together
- [ ] Script → DLL communication
- [ ] Plugin → game state access
- [ ] Hotkey → script execution flow

### Scenario Tests
- [ ] Farm with only Lua scripts (no TextQuest DLL)
- [ ] Farm with only TextQuest DLL (no Lua)
- [ ] Farm with both (verify no conflicts)
- [ ] 36-box group coordination via Lua

---

## Dependencies & Implementation Order

```
Phase 1 (Foundation)
├── Lua 5.4 VM (#791)
├── MacroQuest plugin loader (#792)
└── In-game hotkey system (#793)
    ↓
    Phase 2 (Combat)
    ├── Clickies (#794)
    ├── Enhanced charm/pets (#799)
    ├── Pull system (#800)
    └── Named tracking (#796)
    ↓
    Phase 3 (Movement & Loot)
    ├── Travel (#795)
    ├── Smart loot (#798)
    └── Drag (#797)
    ↓
    Phase 4 (QoL)
    ├── In-game GUI (#801)
    ├── Performance monitoring (#802)
    ├── Debug tools (#803)
    └── In-game help/FAQ (#804)
```

**Total Estimated Timeline**: 8-12 weeks (4 phases × 2-3 weeks each)

---

## Integration Checklist

### Lua VM (#791)
- [ ] Add mlua/lua54 dependency to Cargo.toml
- [ ] Create Lua environment wrapper (Lua VM, script state)
- [ ] Expose TextQuest API to Lua (sandbox)
- [ ] Script lifecycle (load, unload, reload)
- [ ] Error handling and crash isolation
- [ ] Tests: VM initialization, sandbox, script execution

### MacroQuest Plugin (#792)
- [ ] Research MQ2 plugin API
- [ ] Create FFI bindings for common MQ2 functions
- [ ] Implement plugin loader (discover, load, call init/shutdown)
- [ ] Build compatibility shim (MQ2 calls → TextQuest)
- [ ] Test with 3-5 popular plugins
- [ ] Tests: Plugin loading, API translation, crash isolation

### Hotkey System (#793)
- [ ] Extend DLL command hook to support script/plugin commands
- [ ] Lua API: `register_command()`, `register_hotkey()`
- [ ] Command parsing (subcommands, arguments)
- [ ] In-game `/mercs`, `/textquest` commands
- [ ] Help command listing
- [ ] Tests: Command parsing, hotkey binding, execution

### Clickies (#794)
- [ ] Extend `camp/` module with clicky item automation
- [ ] Item cooldown tracking
- [ ] Configuration (per-character item lists, conditions)
- [ ] DLL integration (click items)
- [ ] UI window (clicky status, enable/disable)
- [ ] Tests: Item clicking, cooldown accuracy, various item types

### And so on for each module...

---

## Testing Evidence

Each issue's PR should include:
- [ ] Unit tests (isolated module logic)
- [ ] Integration tests (module + TextQuest ecosystem)
- [ ] Manual test summary (what was manually verified)
- [ ] Performance impact (CPU/memory before/after)
- [ ] Compatibility notes (works on macOS stubs? Windows only?)

---

## Success Criteria (Entire Epic)

- [ ] Can run rgmercs + TextQuest together without conflicts
- [ ] Lua scripts can access game state and execute commands
- [ ] MacroQuest plugins load and execute
- [ ] All 16 rgmercs modules have TextQuest equivalents
- [ ] In-game GUI windows render and respond to input
- [ ] Hotkeys execute script/plugin commands
- [ ] 36-box group coordination works via Lua
- [ ] Zero regressions in existing TextQuest features
- [ ] Documentation updated for all new features

---

## Resources

- **RGMercs Repository**: https://github.com/DerpleDude/rgmercs
- **RGMercs Discord**: RedGuides community (search #rg-mercs)
- **MacroQuest eqlib**: Header files for MQ2 API (use as reference)
- **TextQuest Architecture**: docs/CLAUDE.md, /CLAUDE.md (in-repo)

---

## Questions & Decisions

### Should we fork/include rgmercs?
**Decision**: No. We'll implement equivalent modules, but TextQuest + Lua scripts = more flexible.

### How do we handle conflicts (DLL automation vs. Lua scripts)?
**Decision**: Lua scripts have lower priority. DLL automation wins tiebreakers (e.g., targeting).

### Do we need to support all MQ2 plugins?
**Decision**: No, start with 5-10 popular ones (eqbc, map, lua). Build shim for others gradually.

### In-game GUI mandatory?
**Decision**: No, optional. TUI-only mode still works. GUI is enhancement.

---

**Last Updated**: 2026-04-12
**Branch**: `claude/rgmercs-feature-parity-H1dBp`
**Epic Issue**: [#790](https://github.com/Maleick/TextQuest/issues/790)
