# MacroQuest & RedGuides Plugin Support - Comprehensive Guide

**Status**: ✅ COMPLETE - Ready for Implementation  
**Date**: 2026-04-13  
**Focus**: Full compatibility with MacroQuest ecosystem and RedGuides community

---

## 🎯 Strategic Vision

TextQuest will become the **primary orchestrator** for the MacroQuest ecosystem, enabling:

1. **Full MQ2 Plugin Compatibility** - Run existing plugins without modification
2. **RedGuides Community Support** - Compatible with all RedGuides tools and scripts
3. **Enhanced Capabilities** - Leverage TextQuest's DLL injection for better performance
4. **Zero Learning Curve** - MQ2 developers use familiar APIs
5. **Maintained Backwards Compatibility** - Old plugins still work

---

## 📋 MacroQuest Plugin Support - Issues Breakdown

### Phase 1: MacroQuest Plugin Loader (Feature #792)

**6 Core Sub-Tasks** for complete MQ2 plugin support:

#### [#1012](https://github.com/Maleick/TextQuest/issues/1012) - Research MQ2 API
- Document all MQ2 plugin API functions
- Identify 10+ popular plugins to support:
  - mq2eqbc (group communication)
  - mq2lua (Lua support)
  - mq2map (zone mapping)
  - mq2nav (navigation)
  - mq2melee (melee combat)
  - mq2cast (spell casting)
  - mq2link (main/alt linking)
  - mq2craft (crafting automation)
  - mq2bard (bard automation)
  - mq2netbots (group coordination)
- Document compatibility requirements
- Identify breaking changes needed

**Deliverable**: Complete MQ2 API documentation + compatibility matrix

#### [#1013](https://github.com/Maleick/TextQuest/issues/1013) - Create FFI Bindings for MQ2 Types
- Define Rust equivalents of key MQ2 datatypes:
  - PlayerClient (main player)
  - SpawnInfo (spawn/NPC data)
  - ItemInst (inventory items)
  - CharInfo (character info)
  - GroupInfo (group data)
  - RaidInfo (raid data)
  - GroupMember (group member)
  - EQData (all game data)
  - UI types (CXWnd, CXStr, etc.)
- Memory-safe wrappers around pointers
- Type conversion traits
- Unit tests verifying memory layouts

**Test Requirements**:
- [ ] Type layouts match MQ2 (offset verification)
- [ ] Memory reads/writes work correctly
- [ ] Type conversions are lossless
- [ ] No buffer overflows
- [ ] 80%+ coverage

**Deliverable**: Safe FFI bindings for 20+ MQ2 types

#### [#1014](https://github.com/Maleick/TextQuest/issues/1014) - Plugin Discovery & Loading
- Auto-discover plugins in designated directory
- Load MQ2 plugin DLLs at runtime
- Resolve plugin entry points:
  - InitializePlugin()
  - ShutdownPlugin()
  - OnPulse()
  - OnZoned()
  - OnCommand()
- Track plugin metadata
- Handle load failures gracefully
- Validate plugin version/compatibility

**Test Requirements**:
- [ ] Discover all plugins in directory
- [ ] Load plugins without crashes
- [ ] Entry points resolve correctly
- [ ] Invalid plugins logged (not loaded)
- [ ] Plugin metadata accessible

**Deliverable**: Plugin loader that discovers and loads MQ2 plugins

#### [#1015](https://github.com/Maleick/TextQuest/issues/1015) - MQ2 API Bridge (CRITICAL)
- Translate MQ2 API calls to TextQuest API
- Implement ~100+ MQ2 functions:

**Core Data Access**:
```
GetSpawn()          → textquest.get_spawn()
GetPlayer()         → textquest.get_player()
GetTarget()         → textquest.get_target()
GetGroup()          → textquest.get_group()
GetRaid()           → textquest.get_raid()
GetZoneID()         → textquest.get_zone()
GetCharInfo()       → textquest.get_char_info()
```

**Combat Functions**:
```
Cmd()               → textquest.execute_command()
CastSpell()         → textquest.cast_spell()
UseItem()           → textquest.use_item()
Attack()            → textquest.attack()
Assist()            → textquest.assist()
```

**UI Functions**:
```
GetWndByName()      → textquest.get_window()
SendWndMessage()    → textquest.send_window_msg()
GetCXStr()          → textquest.get_string()
```

**Navigation**:
```
MoveForward()       → textquest.move_forward()
Strafe()            → textquest.strafe()
TurnLeft()          → textquest.turn_left()
LookUp()            → textquest.look_up()
```

**Test Requirements**:
- [ ] All ~100 functions callable from plugins
- [ ] Return values correct type and value
- [ ] Errors propagate appropriately
- [ ] No crashes on bad calls
- [ ] 80%+ coverage

**Deliverable**: Complete MQ2 API bridge with 100+ functions

#### [#1016](https://github.com/Maleick/TextQuest/issues/1016) - Plugin Error Handling & Crash Isolation
- Catch panics in plugin code
- Implement per-plugin error budgets
- Isolate plugin memory
- Watchdog for hanging plugins
- Graceful degradation
- Error logging with context

**Test Requirements**:
- [ ] Broken plugins don't crash TextQuest
- [ ] Hanging plugins detected and killed
- [ ] Errors logged appropriately
- [ ] Error budgets enforced
- [ ] Stability tests pass

**Deliverable**: Robust plugin isolation and error handling

#### [#1017](https://github.com/Maleick/TextQuest/issues/1017) - Plugin Testing & Documentation
- Create 5+ test plugins:
  - Hello World plugin
  - Plugin using TextQuest API
  - Plugin with errors
  - Multi-plugin interaction test
  - Long-running stress test
- Documentation:
  - Plugin development guide
  - API reference
  - Compatibility matrix
  - Troubleshooting guide
- Wiki article on plugin development

**Test Requirements**:
- [ ] Test plugins compile and load
- [ ] Real MQ2 plugins load successfully
- [ ] Plugin API calls work
- [ ] Plugin unload cleanup works
- [ ] 5+ plugins tested

**Deliverable**: Documentation + test plugins + 5+ real plugins verified

---

## 🔌 RedGuides Ecosystem Integration

### Popular RedGuides Plugins to Support

| Plugin | Purpose | Priority | Status |
|--------|---------|----------|--------|
| **mq2eqbc** | Group communication | HIGH | Core support |
| **mq2lua** | Lua scripting | HIGH | Core support |
| **mq2map** | Zone mapping | MEDIUM | Nice to have |
| **mq2nav** | Navigation | MEDIUM | Enhanced by TextQuest nav |
| **mq2melee** | Melee combat | HIGH | Enhanced by TextQuest combat |
| **mq2cast** | Spell casting | HIGH | Core support |
| **mq2link** | Main/alt linking | HIGH | Group coordination |
| **mq2craft** | Crafting automation | MEDIUM | Additional feature |
| **mq2bard** | Bard automation | MEDIUM | Class-specific |
| **mq2netbots** | Group networking | MEDIUM | Replaces some TextQuest features |

### Compatibility Strategy

**Phase 1 (Foundation)**:
- Support core 5 plugins (mq2eqbc, mq2lua, mq2melee, mq2cast, mq2link)
- Verify against RedGuides current versions
- Publish compatibility matrix

**Phase 2 (Expansion)**:
- Support additional 5 plugins
- Create plugin compatibility guide
- Build plugin showcase

**Phase 3 (Optimization)**:
- Performance optimization for heavy plugin usage
- Multi-plugin coordination tests
- Production readiness

---

## 🛠️ How MacroQuest Plugins Will Work in TextQuest

### 1. Plugin Loading Flow
```
TextQuest Startup
    ↓
Scan plugins/ directory
    ↓
Load each .dll plugin
    ↓
Call InitializePlugin()
    ↓
Register plugin commands/hooks
    ↓
Ready for operation
```

### 2. Plugin Execution Context
```
Game Loop
    ↓
TextQuest DLL Hook
    ↓
Call all loaded plugins' OnPulse()
    ↓
Plugins can call TextQuest API
    ↓
Plugins can call EQ functions via DLL
    ↓
Results returned to plugin
```

### 3. Data Flow
```
Game Memory
    ↓
TextQuest DLL reads spawn/player
    ↓
TextQuest API available to plugins
    ↓
Plugins call TextQuest API functions
    ↓
Results go back to plugin
    ↓
Plugin can act (cast spell, move, etc.)
```

---

## 📊 Plugin Support Roadmap

### Week 1-2 (M0)
- ✅ Test infrastructure setup
- ✅ CI/CD pipeline for plugin testing

### Week 2-4 (M1 Phase 1a - Research & FFI)
- [ ] Research MQ2 API (#1012)
- [ ] Create FFI bindings (#1013)
- [ ] Unit tests for type layouts

### Week 4-6 (M1 Phase 1b - Loader & Bridge)
- [ ] Plugin loader implementation (#1014)
- [ ] MQ2 API bridge (~100 functions) (#1015)
- [ ] Error handling & isolation (#1016)

### Week 6-7 (M1 Phase 1c - Testing & Docs)
- [ ] Plugin testing (#1017)
- [ ] Compatibility matrix
- [ ] Plugin development guide

### Week 8+ (Ongoing)
- Test real plugins from RedGuides
- Gather community feedback
- Iterate on compatibility
- Optimize performance

---

## 🧪 Plugin Testing Strategy

### Unit Tests (Per Plugin Support)
- Type conversion tests (FFI bindings)
- API function tests (all 100+ functions)
- Error handling tests
- Memory safety tests

### Integration Tests
- Load single plugin with TextQuest
- Load multiple plugins together
- Plugin interaction tests
- Plugin + Lua script tests
- Plugin + TextQuest features tests

### Real-World Tests
- Load actual RedGuides plugins
- Test popular plugin combinations
- Long-running stability tests
- Performance tests (CPU, memory)

### Compatibility Tests
```
✅ Core 5 plugins verified
✅ Secondary 5 plugins verified
✅ Popular plugin combinations work
✅ No memory corruption
✅ No performance degradation
```

---

## 🎓 RedGuides Community Integration

### Announcement Strategy
1. **Alpha Phase**: Internal testing with RedGuides core team
2. **Beta Phase**: Limited release to RedGuides community
3. **Launch**: Full release with documentation
4. **Support**: Ongoing compatibility maintenance

### Documentation for RedGuides
- Plugin migration guide (from pure MQ2)
- Compatibility matrix (updated monthly)
- FAQ for RedGuides plugins
- Troubleshooting guide
- Video tutorials on plugin development

### Community Feedback
- GitHub issues for plugin compatibility
- Discord channel for plugin developers
- Monthly compatibility updates
- Plugin showcase on wiki

---

## ✅ Success Criteria for Plugin Support

### Phase 1 Completion
- [ ] 100+ MQ2 API functions implemented
- [ ] Core 5 RedGuides plugins load successfully
- [ ] No crashes when plugins loaded
- [ ] 80%+ test coverage
- [ ] Compatibility matrix published
- [ ] Plugin development guide written

### Phase 2 Completion
- [ ] Secondary 10 plugins load successfully
- [ ] Multi-plugin coordination works
- [ ] Performance meets requirements
- [ ] Community feedback addressed
- [ ] Showcase plugins running

### Full Success
- [ ] ALL RedGuides plugins compatible (or documented as incompatible)
- [ ] Plugin performance equals/exceeds pure MQ2
- [ ] Large community adoption
- [ ] Zero critical plugin bugs
- [ ] Active maintenance cadence

---

## 🔐 Security Considerations

### Plugin Sandboxing
- ✅ Plugins can't access file system directly
- ✅ Plugins can't break out to OS
- ✅ Plugins can't crash main process
- ✅ Memory isolation between plugins

### Validation
- ✅ Verify plugin signatures (if needed)
- ✅ Check for known malicious code patterns
- ✅ Limit plugin memory usage
- ✅ Rate-limit plugin calls

### Monitoring
- ✅ Log all plugin operations
- ✅ Alert on suspicious behavior
- ✅ Track plugin crashes/hangs
- ✅ Monitor CPU/memory per plugin

---

## 📈 Metrics to Track

### Plugin Adoption
- Number of plugins loaded per user
- Most popular plugins
- Plugin crash rates
- Plugin performance impact

### Community
- RedGuides Discord mentions
- GitHub issues for plugins
- Plugin success rate
- User testimonials

### Quality
- Compatibility rate (% plugins that work)
- Average plugin latency
- Plugin crash rate
- User satisfaction

---

## 🚀 Implementation Focus Areas

### For RedGuides Community:
1. **mq2eqbc** - Critical for group play
2. **mq2lua** - Enables community scripts
3. **mq2melee** - Combat automation
4. **mq2cast** - Spell automation
5. **mq2link** - Alt coordination

### For TextQuest Integration:
1. Plugin API bridge (100+ functions)
2. Error isolation and recovery
3. Performance optimization
4. Multi-plugin coordination
5. Plugin configuration management

### For Community:
1. Clear documentation
2. Easy plugin installation
3. Active support
4. Regular updates
5. Compatibility testing

---

## 📝 Plugin Development Workflow (For RedGuides Devs)

### 1. Development
```cpp
// Old MQ2 plugin (still works!)
void InitializePlugin() { ... }
void OnPulse() { 
  GetSpawn();  // ← Calls TextQuest API now
  CastSpell(); // ← Via TextQuest bridge
}
```

### 2. Testing
```
Load plugin → TextQuest loads via plugin loader
Run plugin → Plugin calls TextQuest API
Debug → Use TextQuest debug tools
Verify → Run compatibility tests
```

### 3. Release
```
Publish to RedGuides GitHub
Update compatibility matrix
Announce in Discord
Gather feedback
Iterate
```

---

## 🎉 Vision: TextQuest as the MQ2 Orchestrator

By completing this MacroQuest plugin support initiative, TextQuest becomes:

✅ **The Primary Orchestrator** for EQ multiboxing
✅ **RedGuides Compatible** - All community scripts work
✅ **Plugin Compatible** - All MQ2 plugins load
✅ **Enhanced Features** - Better than pure MQ2
✅ **Community-Driven** - Supported by RedGuides
✅ **Production-Ready** - Stability and performance guaranteed

---

## 📊 MacroQuest Plugin Support Issue Map

| Issue | Feature | Hours | Team |
|-------|---------|-------|------|
| #1012 | Research MQ2 API | 4h | Senior engineer |
| #1013 | FFI bindings | 12h | Memory expert |
| #1014 | Plugin loader | 8h | Systems engineer |
| #1015 | MQ2 API bridge | 16h | API expert |
| #1016 | Error handling | 8h | QA engineer |
| #1017 | Testing & docs | 8h | Technical writer |
| **Total** | **Plugin Support** | **56h** | **1 lead + 5 support** |

---

**Status**: ✅ Ready for RedGuides Integration  
**Team**: 1 lead + 5 supporting engineers  
**Duration**: 5 weeks (within Phase 1)  
**Test Coverage**: 80%+ (unit + integration)  
**Compatibility**: Core 5 plugins, expanding to 20+

---

This is how TextQuest becomes **the de facto standard for EQ multiboxing**.

Let's make it happen! 🚀
