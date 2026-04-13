# RGMercs Feature Parity - Complete Issue Index

**Last Updated**: 2026-04-13
**Total Issues**: 1 Epic + 14 Features + 70+ Sub-Tasks
**Branch**: `claude/rgmercs-feature-parity-H1dBp`

---

## 📋 Master Epic

| Issue | Title | Status | Link |
|-------|-------|--------|------|
| **#790** | **Epic: rgmercs Feature Parity & Lua/MacroQuest Plugin Support** | 📋 Planning | [View](#) |

---

## 🔧 Phase 1: Scripting Foundation (20 Sub-Tasks)

### Feature #791: Lua 5.4 VM Integration

| Issue | Task | Depends On | Complexity | Est. Hours |
|-------|------|-----------|-----------|-----------|
| #1003 | Add mlua dependency and Lua 5.4 setup | - | **Trivial** | ~2h |
| #1004 | Create LuaContext wrapper and initialization | #1003 | **Small** | ~4h |
| #1005 | Implement script loading and execution | #1004 | **Small** | ~6h |
| #1006 | Implement script lifecycle (unload, reload, pause) | #1005 | **Small** | ~8h |
| #1007 | Implement Lua sandbox restrictions | #1004 | **Small** | ~6h |
| #1008 | Expose TextQuest API to Lua | #1004, #1007 | **Medium** | ~16h |
| #1009 | Implement error handling and crash isolation | #1004, #1008 | **Medium** | ~8h |
| #1010 | Add unit tests for Lua VM | All above | **Medium** | ~12h |
| #1011 | Integration tests and Lua documentation | All above | **Small** | ~8h |

**Subtotal**: 9 tasks | ~70 hours

---

### Feature #792: MacroQuest Plugin Loader

| Issue | Task | Depends On | Complexity | Est. Hours |
|-------|------|-----------|-----------|-----------|
| #1012 | Research MQ2 API and plugin architecture | - | **Trivial** | ~4h |
| #1013 | Create FFI bindings for MQ2 types | #1012 | **Medium** | ~12h |
| #1014 | Implement plugin discovery and loading | #1013 | **Small** | ~8h |
| #1015 | Create MQ2 API bridge | #1014, #1013 | **Medium** | ~16h |
| #1016 | Add plugin error handling and crash isolation | #1015 | **Medium** | ~8h |
| #1017 | Plugin tests and documentation | #1016 | **Small** | ~8h |

**Subtotal**: 6 tasks | ~56 hours

---

### Feature #793: In-Game Hotkey System

| Issue | Task | Depends On | Complexity | Est. Hours |
|-------|------|-----------|-----------|-----------|
| #1020 | Create hotkey registration infrastructure | - | **Small** | ~8h |
| #1021 | Implement slash command system | #1020 | **Small** | ~8h |
| #1024 | Integrate hotkeys/commands with Lua and plugins | #1021, #791, #792 | **Small** | ~8h |
| #1026 | Implement built-in TextQuest commands | #1024 | **Small** | ~8h |
| #1029 | Hotkey/command tests and documentation | #1026 | **Small** | ~6h |

**Subtotal**: 5 tasks | ~38 hours

**Phase 1 Total**: 20 tasks | ~164 hours (~5-6 weeks)

---

## ⚔️ Phase 2: Combat Modules (18 Sub-Tasks)

### Feature #794: Clickies Module (5 Sub-Tasks)

| Issue | Task | Depends On | Complexity | Est. Hours |
|-------|------|-----------|-----------|-----------|
| #1035 | Design clickies module architecture | - | **Trivial** | ~3h |
| #1038 | Implement clicky item automation core | #1035 | **Medium** | ~12h |
| #1040 | Implement clicky configuration system | #1038 | **Small** | ~8h |
| #1043 | Integrate clickies into camp loop and TUI | #1040 | **Small** | ~8h |
| #1045 | Clickies testing and documentation | #1043 | **Small** | ~8h |

**Subtotal**: 5 tasks | ~39 hours

---

### Feature #799: Enhanced Charm/Pet Management (5 Sub-Tasks)

| Issue | Task | Depends On | Complexity | Est. Hours |
|-------|------|-----------|-----------|-----------|
| #1049 | Design charm/pet management system | - | **Trivial** | ~3h |
| #1051 | Implement charm spell automation | #1049 | **Small** | ~8h |
| #1053 | Implement pet control and commands | #1051 | **Medium** | ~10h |
| #1054 | Charm/pet configuration | #1053 | **Small** | ~6h |
| #1056 | Charm/pet testing and documentation | #1054 | **Small** | ~6h |

**Subtotal**: 5 tasks | ~33 hours

---

### Feature #800: Improved Pull System (4 Sub-Tasks)

| Issue | Task | Depends On | Complexity | Est. Hours |
|-------|------|-----------|-----------|-----------|
| #1060 | Design directional pulling system | - | **Trivial** | ~3h |
| #1063 | Implement directional pulling | #1060 | **Medium** | ~12h |
| #1064 | Implement multi-pull and adaptive pulling | #1063 | **Medium** | ~10h |
| #1066 | Pull system testing and documentation | #1064 | **Small** | ~6h |

**Subtotal**: 4 tasks | ~31 hours

---

### Feature #796: Named NPC Tracking (4 Sub-Tasks)

| Issue | Task | Depends On | Complexity | Est. Hours |
|-------|------|-----------|-----------|-----------|
| #1069 | Design named NPC tracking system | - | **Trivial** | ~3h |
| #1070 | Implement named NPC database and tracking | #1069 | **Medium** | ~10h |
| #1073 | Integrate named encounters into combat | #1070 | **Medium** | ~12h |
| #1074 | Named NPC testing and documentation | #1073 | **Small** | ~6h |

**Subtotal**: 4 tasks | ~31 hours

**Phase 2 Total**: 18 tasks | ~134 hours (~4-5 weeks)

---

## 🧭 Phase 3: Movement & Loot (15 Sub-Tasks)

### Feature #795: Travel Module (4 Sub-Tasks)

| Issue | Task | Depends On | Complexity | Est. Hours |
|-------|------|-----------|-----------|-----------|
| #1079 | Design travel module architecture | - | **Trivial** | ~3h |
| #1082 | Implement portal database | #1079 | **Small** | ~6h |
| #1085 | Implement group travel coordination | #1082 | **Medium** | ~12h |
| #1087 | Travel system testing and documentation | #1085 | **Small** | ~6h |

**Subtotal**: 4 tasks | ~27 hours

---

### Feature #798: Smart Loot Automation (4 Sub-Tasks)

| Issue | Task | Depends On | Complexity | Est. Hours |
|-------|------|-----------|-----------|-----------|
| #1089 | Design smart loot system | - | **Trivial** | ~3h |
| #1090 | Implement advanced loot filtering | #1089 | **Medium** | ~10h |
| #1094 | Implement loot and scoot automation | #1090 | **Medium** | ~12h |
| #1097 | Smart loot testing and documentation | #1094 | **Small** | ~6h |

**Subtotal**: 4 tasks | ~31 hours

---

### Feature #797: Drag Module (3 Sub-Tasks)

| Issue | Task | Depends On | Complexity | Est. Hours |
|-------|------|-----------|-----------|-----------|
| #1098 | Design drag/object movement system | - | **Trivial** | ~3h |
| #1101 | Implement corpse dragging | #1098 | **Medium** | ~12h |
| #1105 | Drag system testing and documentation | #1101 | **Small** | ~4h |

**Subtotal**: 3 tasks | ~19 hours

**Phase 3 Total**: 15 tasks | ~77 hours (~2-3 weeks)

---

## 🎨 Phase 4: Quality of Life (18 Sub-Tasks)

### Feature #801: In-Game GUI Windows (6 Sub-Tasks)

| Issue | Task | Depends On | Complexity | Est. Hours |
|-------|------|-----------|-----------|-----------|
| #1104 | Design in-game GUI overlay system | - | **Trivial** | ~4h |
| #1105 | Implement DirectX overlay hook | #1104 | **Large** | ~16h |
| #1106 | Implement window manager and widgets | #1105 | **Large** | ~20h |
| #1107 | Implement input handling for overlay | #1106 | **Medium** | ~12h |
| #1108 | Implement specific GUI windows | #1107 | **Medium** | ~16h |
| #1109 | GUI testing and documentation | #1108 | **Small** | ~8h |

**Subtotal**: 6 tasks | ~76 hours

---

### Feature #802: Performance Monitoring (4 Sub-Tasks)

| Issue | Task | Depends On | Complexity | Est. Hours |
|-------|------|-----------|-----------|-----------|
| #1111 | Design performance monitoring system | - | **Trivial** | ~3h |
| #1112 | Implement metrics collection and dashboards | #1111 | **Medium** | ~14h |
| #1113 | Implement historical tracking and alerts | #1112 | **Medium** | ~10h |
| #1114 | Monitoring testing and documentation | #1113 | **Small** | ~6h |

**Subtotal**: 4 tasks | ~33 hours

---

### Feature #803: Debug Tools (4 Sub-Tasks)

| Issue | Task | Depends On | Complexity | Est. Hours |
|-------|------|-----------|-----------|-----------|
| #1115 | Implement memory inspection tools | - | **Medium** | ~10h |
| #1116 | Implement Lua script debugging | #791 | **Medium** | ~12h |
| #1117 | Implement command/packet tracing | - | **Small** | ~8h |
| #1118 | Debug tools testing and documentation | All above | **Small** | ~6h |

**Subtotal**: 4 tasks | ~36 hours

---

### Feature #804: In-Game Help/FAQ (3 Sub-Tasks)

| Issue | Task | Depends On | Complexity | Est. Hours |
|-------|------|-----------|-----------|-----------|
| #1119 | Design in-game help system | - | **Trivial** | ~2h |
| #1122 | Implement help system and interface | #1119 | **Small** | ~8h |
| #1124 | Create help content | #1122 | **Small** | ~12h |

**Subtotal**: 3 tasks | ~22 hours

**Phase 4 Total**: 18 tasks | ~167 hours (~5-6 weeks)

---

## 📊 Summary Statistics

| Metric | Value |
|--------|-------|
| **Total Issues** | 1 Epic + 14 Features + 70 Sub-Tasks = **85 issues** |
| **Phase 1 Total** | 20 tasks • ~164 hours • **5-6 weeks** |
| **Phase 2 Total** | 18 tasks • ~134 hours • **4-5 weeks** |
| **Phase 3 Total** | 15 tasks • ~77 hours • **2-3 weeks** |
| **Phase 4 Total** | 18 tasks • ~167 hours • **5-6 weeks** |
| **Grand Total** | 71 tasks • ~542 hours • **16-20 weeks** |
| **Team Size Estimate** | 4-6 engineers (parallel work) → **8-12 weeks** |

---

## 🏷️ Label Categories Used

### Priority
- `p0-critical` - Must have, blocks other work
- `p1-high` - Important, should do soon
- `p2-medium` - Nice to have, lower priority
- `p3-low` - Enhancement, can defer

### Complexity
- `complexity-trivial` - Research/design only
- `complexity-small` - <6 hours
- `complexity-medium` - 6-16 hours
- `complexity-large` - 16-40 hours

### Size
- `size-xs` - <2 hours
- `size-s` - 2-6 hours
- `size-m` - 6-16 hours
- `size-l` - 16-40 hours
- `size-xl` - 40+ hours

### Type
- `task` - Implementation work
- `testing` - Tests and validation
- `documentation` - Docs and guides
- `design` - Architecture/planning
- `research` - Investigation

### Component
- `lua` - Lua VM and scripts
- `macroquest` - MQ2 plugins
- `hotkeys` - Hotkey system
- `clickies` - Item automation
- `charm-pets` - Pet management
- `pull` - Pull system
- `named` - Named NPC tracking
- `travel` - Travel/portals
- `loot` - Loot filtering
- `drag` - Corpse dragging
- `gui` - In-game overlay
- `monitoring` - Performance metrics
- `debug` - Debug tools
- `help` - Help system

### Special Tags
- `phase-1`, `phase-2`, `phase-3`, `phase-4` - Phase assignment
- `infrastructure` - Foundation work
- `integration` - System integration
- `stability` - Stability/reliability
- `security` - Security concerns
- `unsafe` - Unsafe Rust code
- `ffi` - Foreign function interface
- `error-handling` - Error handling
- `input` - Input system
- `rendering` - Graphics/rendering
- `developer-tools` - Developer features

---

## 🚀 Recommended Execution Order

### Phase 1 (Foundation) - Must complete first
1. Start with **#1003-#1010** (Lua VM) in parallel
2. Start with **#1012-#1017** (MQ2 plugins) in parallel
3. Start with **#1020-#1026** (Hotkeys) in parallel
4. All phase 1 tests and docs (**#1011**, **#1017**, **#1029**)

### Phase 2 (Combat) - Depends on Phase 1
Can work in parallel after Phase 1 foundation is stable:
- **#1035-#1045** (Clickies)
- **#1049-#1056** (Charm/Pets)
- **#1060-#1066** (Pull system)
- **#1069-#1074** (Named tracking)

### Phase 3 (Movement) - Depends on Phase 1
Can work in parallel after Phase 1 foundation is stable:
- **#1079-#1087** (Travel)
- **#1089-#1097** (Smart Loot)
- **#1098-#1105** (Drag)

### Phase 4 (QoL) - Optional, can work in parallel
Can work in parallel but lower priority:
- **#1104-#1109** (GUI overlay)
- **#1111-#1114** (Performance monitoring)
- **#1115-#1118** (Debug tools)
- **#1119-#1124** (Help/FAQ)

---

## 🔍 How to Use This Index

1. **Track Progress**: Each sub-task links to its parent feature issue
2. **Prioritize**: Start with Phase 1, then parallelize Phases 2-3, Phase 4 is optional
3. **Find Work**: Search by component tag (e.g., "lua", "pull") or phase
4. **Understand Dependencies**: Check "Depends On" column for blocker tasks
5. **Estimate Capacity**: Use complexity/hours estimates for sprint planning

---

## 📝 Issue Numbering Scheme

| Range | Phase | Feature |
|-------|-------|---------|
| #790 | Meta | Epic |
| #791-#811 | 1 | Lua (#791) |
| #812-#817 | 1 | MacroQuest (#792) |
| #818-#829 | 1 | Hotkeys (#793) |
| #1031-#1045 | 2 | Clickies (#794) |
| #1049-#1056 | 2 | Charm (#799) |
| #1060-#1066 | 2 | Pull (#800) |
| #1069-#1074 | 2 | Named (#796) |
| #1079-#1087 | 3 | Travel (#795) |
| #1089-#1097 | 3 | Loot (#798) |
| #1098-#1105 | 3 | Drag (#797) |
| #1104-#1109 | 4 | GUI (#801) |
| #1111-#1114 | 4 | Monitoring (#802) |
| #1115-#1118 | 4 | Debug (#803) |
| #1119-#1124 | 4 | Help (#804) |

---

## 🔗 Quick Links

- **Epic Issue**: [#790](https://github.com/Maleick/TextQuest/issues/790)
- **Feature Parity Document**: [docs/RGMERCS_FEATURE_PARITY.md](../docs/RGMERCS_FEATURE_PARITY.md)
- **RGMercs Repository**: https://github.com/DerpleDude/rgmercs
- **TextQuest CLAUDE.md**: [CLAUDE.md](../CLAUDE.md)

---

**Generated**: 2026-04-13 | **Branch**: `claude/rgmercs-feature-parity-H1dBp`
