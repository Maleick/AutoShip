# RGMercs Feature Parity - Implementation Breakdown

**Status**: Work planning complete | **Date**: 2026-04-13
**Total Effort**: ~542 hours (~16-20 weeks) | **Team Size**: 4-6 engineers (8-12 weeks parallel)

---

## 🎯 Implementation Phases Overview

### Phase 1: Scripting Foundation (164 hours / 5-6 weeks)
**Goal**: Enable Lua scripts and MacroQuest plugins to execute

```
┌─────────────────────────────────────────┐
│   Lua VM (#791)                         │
│   - mlua integration                    │
│   - Sandbox restrictions                │
│   - Script lifecycle (load/unload)      │
│   - Error isolation                     │
└─────────────────────────────────────────┘
         ↑
         │ (both depend on Lua VM)
         ↓
┌─────────────────────────────────────────┐
│   MacroQuest Plugins (#792)             │   Hotkey System (#793)
│   - FFI bindings                        │   - Hotkey registration
│   - Plugin loader                       │   - Slash commands
│   - API bridge                          │   - Built-in commands
│   - Error handling                      │
└─────────────────────────────────────────┘
```

**Deliverables**:
- ✅ Lua scripts can execute and access game state
- ✅ MQ2 plugins can load and run
- ✅ In-game `/textquest` and `/mercs` commands work
- ✅ Scripts/plugins can register hotkeys and commands

**Critical Path**: Lua VM → API Bridge → Hotkey Integration

---

### Phase 2: Combat Modules (134 hours / 4-5 weeks)
**Goal**: Implement combat-related features from rgmercs

**Can work in parallel** (after Phase 1 foundation):

```
Clickies (#794)          Charm/Pets (#799)
  ├─ Design              ├─ Design
  ├─ Core logic          ├─ Charm automation
  ├─ Config              ├─ Pet control
  ├─ Integration         ├─ Config
  └─ Testing             └─ Testing
  ~39 hours              ~33 hours

Pull System (#800)       Named Tracking (#796)
  ├─ Design              ├─ Design
  ├─ Directional pull    ├─ NPC database
  ├─ Multi-pull          ├─ Combat integration
  ├─ Patterns            └─ Testing
  └─ Testing             ~31 hours
  ~31 hours
```

**Deliverables**:
- ✅ Consumable items auto-use on schedule
- ✅ Pet/charm spell automation with control
- ✅ Multi-pull and directional pulling
- ✅ Named NPC encounters with special strategies

**Parallelization**: All 4 features can proceed in parallel after Phase 1

---

### Phase 3: Movement & Loot (77 hours / 2-3 weeks)
**Goal**: Implement travel and loot management

**Can work in parallel** (after Phase 1 foundation):

```
Travel Module (#795)     Smart Loot (#798)    Drag Module (#797)
  ├─ Portal DB           ├─ Filtering          ├─ Design
  ├─ Group travel        ├─ Loot & scoot       ├─ Corpse drag
  ├─ Coordination        ├─ History            └─ Testing
  └─ Testing             └─ Testing            ~19 hours
  ~27 hours              ~31 hours
```

**Deliverables**:
- ✅ Group fast travel with portal coordination
- ✅ Smart loot filtering and "loot and scoot" automation
- ✅ Corpse dragging to safe zones

**Parallelization**: All 3 features can proceed in parallel

---

### Phase 4: Quality of Life (167 hours / 5-6 weeks)
**Goal**: Add observability, debugging, and user experience features

**Optional** (can work in parallel, lower priority):

```
In-Game GUI (#801)       Performance Monitor (#802)
  ├─ DirectX hook        ├─ Metrics collection
  ├─ Window manager      ├─ Dashboards
  ├─ Widgets             ├─ Historical tracking
  ├─ Input handling      └─ Testing
  ├─ Windows             ~33 hours
  └─ Testing
  ~76 hours

Debug Tools (#803)       Help/FAQ (#804)
  ├─ Memory inspect      ├─ Design
  ├─ Lua debugging       ├─ Interface
  ├─ Command tracing     ├─ Content
  └─ Testing             └─ (Writing)
  ~36 hours              ~22 hours
```

**Deliverables**:
- ✅ In-game overlay GUI (optional but nice)
- ✅ Real-time performance metrics
- ✅ Debug/inspection tools for developers
- ✅ In-game help system

**Parallelization**: All features can proceed in parallel (but lower priority)

---

## 📋 Detailed Task Breakdown by Phase

### Phase 1: Scripting Foundation

#### Lua 5.4 VM Integration (9 tasks, ~70 hours)
| Task | Hours | Risk | Notes |
|------|-------|------|-------|
| Dependency setup | 2 | Low | Add mlua to Cargo.toml |
| LuaContext wrapper | 4 | Low | Basic struct + initialization |
| Script loading | 6 | Low | Read files, execute Lua |
| Script lifecycle | 8 | Medium | Unload/reload edge cases |
| Sandbox | 6 | **High** | Security critical |
| API bindings | 16 | **High** | Many functions to expose |
| Error handling | 8 | Medium | Crash isolation |
| Unit tests | 12 | Low | Test each component |
| Integration tests | 8 | Low | End-to-end tests |

**Critical**: Sandbox and API bindings need careful design

#### MacroQuest Plugin Loader (6 tasks, ~56 hours)
| Task | Hours | Risk | Notes |
|------|-------|------|-------|
| Research | 4 | Low | Document API |
| FFI bindings | 12 | **High** | Memory layout critical |
| Plugin loader | 8 | Medium | DLL loading, error handling |
| API bridge | 16 | **High** | Lots of functions |
| Error handling | 8 | Medium | Crash isolation |
| Tests + docs | 8 | Low | Validation |

**Critical**: FFI bindings and API bridge need verification

#### Hotkey System (5 tasks, ~38 hours)
| Task | Hours | Risk | Notes |
|------|-------|------|-------|
| Hotkey registry | 8 | Low | Basic registration |
| Command parsing | 8 | Low | Parse slash commands |
| Lua/plugin integration | 8 | Low | Use registry from Lua |
| Built-in commands | 8 | Low | Implement /textquest cmds |
| Tests + docs | 6 | Low | Straightforward |

**Critical**: Integration with Lua/plugin systems

---

### Phase 2: Combat Modules

#### Clickies (5 tasks, ~39 hours)
| Task | Hours | Risk | Notes |
|------|-------|------|-------|
| Design | 3 | Low | Architecture |
| Core logic | 12 | Low | Item queuing, cooldowns |
| Config | 8 | Low | TOML format |
| Integration | 8 | Low | Hook into camp loop |
| Tests + docs | 8 | Low | Straightforward |

**Dependencies**: Needs camp loop integration

#### Charm/Pets (5 tasks, ~33 hours)
| Task | Hours | Risk | Notes |
|------|-------|------|-------|
| Design | 3 | Low | Architecture |
| Charm automation | 8 | Medium | Spell casting logic |
| Pet control | 10 | Medium | Command execution |
| Config | 6 | Low | Per-class settings |
| Tests + docs | 6 | Low | Straightforward |

**Dependencies**: Combat system integration

#### Pull System (4 tasks, ~31 hours)
| Task | Hours | Risk | Notes |
|------|-------|------|-------|
| Design | 3 | Low | Architecture |
| Directional pulling | 12 | **High** | Position math |
| Multi-pull | 10 | **High** | Spacing validation |
| Tests + docs | 6 | Low | Examples needed |

**Critical**: Directional pulling requires careful tuning

#### Named Tracking (4 tasks, ~31 hours)
| Task | Hours | Risk | Notes |
|------|-------|------|-------|
| Design | 3 | Low | Architecture |
| Database | 10 | Low | JSON/TOML data |
| Combat integration | 12 | Medium | Override targeting |
| Tests + docs | 6 | Low | Straightforward |

**Dependencies**: Loot and combat systems

---

### Phase 3: Movement & Loot

#### Travel (4 tasks, ~27 hours)
| Task | Hours | Risk | Notes |
|------|-------|------|-------|
| Design | 3 | Low | Architecture |
| Portal database | 6 | Low | Data entry |
| Group coordination | 12 | Medium | Straggler handling |
| Tests + docs | 6 | Low | Straightforward |

**Dependencies**: Navigation system

#### Smart Loot (4 tasks, ~31 hours)
| Task | Hours | Risk | Notes |
|------|-------|------|-------|
| Design | 3 | Low | Architecture |
| Filtering | 10 | Medium | Complex rules |
| Loot & scoot | 12 | Medium | Timer/zone logic |
| Tests + docs | 6 | Low | Examples needed |

**Dependencies**: Existing loot module

#### Drag (3 tasks, ~19 hours)
| Task | Hours | Risk | Notes |
|------|-------|------|-------|
| Design | 3 | Low | Architecture |
| Implementation | 12 | **High** | Physics/movement |
| Tests + docs | 4 | Low | Straightforward |

**Critical**: Movement physics need careful implementation

---

### Phase 4: Quality of Life

#### GUI Overlay (6 tasks, ~76 hours)
| Task | Hours | Risk | Notes |
|------|-------|------|-------|
| Design | 4 | Low | Architecture |
| DirectX hook | 16 | **High** | Graphics programming |
| Window manager | 20 | **High** | Lots of widget code |
| Input handling | 12 | Medium | Mouse/keyboard routing |
| Windows | 16 | Medium | Specific window impls |
| Tests + docs | 8 | Low | Integration tests |

**Critical**: DirectX hooking and window manager are complex

#### Performance Monitoring (4 tasks, ~33 hours)
| Task | Hours | Risk | Notes |
|------|-------|------|-------|
| Design | 3 | Low | Architecture |
| Metrics collection | 14 | Medium | Accurate tracking |
| Historical data | 10 | Low | SQLite backend |
| Tests + docs | 6 | Low | Straightforward |

**Dependencies**: Existing metrics module

#### Debug Tools (4 tasks, ~36 hours)
| Task | Hours | Risk | Notes |
|------|-------|------|-------|
| Memory inspection | 10 | Low | Hex dumps |
| Lua debugging | 12 | Medium | Debugger hooks |
| Command tracing | 8 | Low | Logging |
| Tests + docs | 6 | Low | Straightforward |

**Dependencies**: Lua VM system

#### Help/FAQ (3 tasks, ~22 hours)
| Task | Hours | Risk | Notes |
|------|-------|------|-------|
| Design | 2 | Low | Structure |
| Interface | 8 | Low | Searchable window |
| Content | 12 | Low | Writing |

**Lowest Risk**: Mostly writing content

---

## 🚨 Risk Assessment

### High-Risk Items (Need Extra Review)
1. **Lua Sandbox** (#1007) - Security critical
2. **FFI Type Bindings** (#1013) - Memory layout must match exactly
3. **MQ2 API Bridge** (#1015) - Lots of functions, easy to miss edge cases
4. **DirectX Hooking** (#1105) - Graphics programming, platform-specific
5. **Window Manager** (#1106) - Lots of widget code, UI complexity
6. **Pull System Physics** (#1063, #1064) - Requires testing/tuning
7. **Drag Movement** (#1101) - Physics and stuck detection

### Medium-Risk Items
1. Script lifecycle edge cases (#1006)
2. Plugin error isolation (#1016)
3. Charm/pet spell logic (#1051)
4. Loot filtering rules (#1090)
5. Input handling for overlay (#1107)

### Low-Risk Items
- Dependency setup
- Configuration systems
- Testing and documentation
- Help/FAQ content

---

## 💼 Resource Planning

### Recommended Team Structure

**Phase 1 (Foundation) - 4 people, 5-6 weeks**:
- **Person A**: Lua VM (#1003-#1010) - Lua/Rust expert
- **Person B**: MQ2 plugins (#1012-#1017) - FFI/memory layout expert
- **Person C**: Hotkey system (#1020-#1029) - Game hooks expert
- **Person D**: Integration/review - Senior engineer

**Phase 2 (Combat) - 4 people, 4-5 weeks, parallel**:
- **Person A**: Clickies (#1035-#1045)
- **Person B**: Charm/pets (#1049-#1056)
- **Person C**: Pull system (#1060-#1066)
- **Person D**: Named tracking (#1069-#1074)

**Phase 3 (Movement) - 3 people, 2-3 weeks, parallel**:
- **Person A**: Travel (#1079-#1087)
- **Person B**: Smart loot (#1089-#1097)
- **Person C**: Drag (#1098-#1105)

**Phase 4 (QoL) - 2-3 people, 5-6 weeks, parallel (optional)**:
- **Person A**: GUI overlay (#1104-#1109) - Graphics expert
- **Person B**: Monitoring (#1111-#1114) + Debug (#1115-#1118)
- **Person C**: Help/FAQ (#1119-#1124) - Technical writer

### Time Estimates with Team

| Scenario | Team Size | Total Duration |
|----------|-----------|-----------------|
| Sequential (Phase after Phase) | 1 person | ~20 weeks |
| Phase 1 only | 4 people | 6 weeks |
| Phases 1+2 sequential | 4 people | 10 weeks |
| Phases 1+2+3 sequential | 4 people | 12 weeks |
| All phases parallel (2+2+2+2) | 8 people | 6 weeks |
| Recommended (1→2→3, 4 parallel) | 6 people | **8-12 weeks** |

---

## 📅 Sample Sprint Planning (8-week execution, 2-week sprints)

### Sprint 1-2: Phase 1 Lua VM & Foundation
- **Team**: 4 engineers
- **Work**: #1003-#1010, #1020-#1021
- **Completion**: Basic Lua VM, hotkey registration

### Sprint 3-4: Phase 1 Completion + Phase 2 Start
- **Team**: 6 engineers (2 join for Phase 2)
- **Work**: #1012-#1017 (MQ2), #1024-#1029 (Hotkeys), #1035-#1038 (Clickies start)
- **Completion**: Full Phase 1, Clickies core started

### Sprint 5-6: Phase 2 Combat Modules
- **Team**: 4 engineers (on Phase 2)
- **Work**: All Phase 2 core implementations
- **Completion**: All combat modules functional

### Sprint 7: Phase 2 Completion + Phase 3 Start
- **Team**: 5 engineers (3 Phase 2 completion, 3 Phase 3 start)
- **Work**: Phase 2 testing/docs, Phase 3 cores
- **Completion**: Phase 2 complete, Phase 3 halfway

### Sprint 8: Phase 3 Completion + Phase 4 Optional
- **Team**: 4-6 engineers
- **Work**: Phase 3 completion, Phase 4 start (optional)
- **Completion**: Full feature parity achieved

---

## ✅ Success Criteria

### Phase 1 Complete
- [ ] Lua scripts execute without crashing
- [ ] Can load and run MQ2 plugins
- [ ] Hotkeys register and execute
- [ ] All Phase 1 tests pass

### Phase 2 Complete
- [ ] Clickies work on schedule
- [ ] Charm/pets function correctly
- [ ] Pull system works (including multi-pull)
- [ ] Named encounters tracked
- [ ] All Phase 2 tests pass

### Phase 3 Complete
- [ ] Group travel works
- [ ] Smart loot filters correctly
- [ ] Corpse dragging functions
- [ ] All Phase 3 tests pass

### Phase 4 Complete (Optional)
- [ ] GUI renders without lag
- [ ] Performance metrics accurate
- [ ] Debug tools functional
- [ ] Help system searchable
- [ ] All Phase 4 tests pass

### Overall Parity
- [ ] rgmercs features replicated in TextQuest
- [ ] Feature parity tests pass
- [ ] Can run rgmercs + TextQuest together
- [ ] Zero regressions in existing features
- [ ] Documentation complete

---

## 📚 Related Documents

- **Master Epic**: [#790](https://github.com/Maleick/TextQuest/issues/790)
- **Feature Parity Doc**: [docs/RGMERCS_FEATURE_PARITY.md](./RGMERCS_FEATURE_PARITY.md)
- **Issue Index**: [docs/ISSUE_INDEX.md](./ISSUE_INDEX.md)
- **TextQuest CLAUDE.md**: [../CLAUDE.md](../CLAUDE.md)
- **RGMercs Repo**: https://github.com/DerpleDude/rgmercs

---

**Generated**: 2026-04-13 | **Version**: 1.0 | **Branch**: `claude/rgmercs-feature-parity-H1dBp`
