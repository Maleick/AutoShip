# Final Comprehensive Issue Index - All 185+ Issues

**Status**: ✅ Complete and Ready for Execution  
**Date**: 2026-04-13  
**Total Issues**: 1 Epic + 14 Features + 71 Sub-Tasks + 15 Infrastructure/Polish = **101 issues**  
**Total Estimated Effort**: ~800 hours across 4-6 engineers over 12-16 weeks

---

## 📋 Quick Navigation

- **[Master Epic](#master-epic)** - Track everything here
- **[Foundation & CI/CD (M0)](#m0-foundation--cicd)** - 7 issues
- **[Phase 1: Scripting (M1)](#m1-phase-1-scripting-foundation)** - 20 core + tests
- **[Phase 2: Combat (M2)](#m2-phase-2-combat-modules)** - 18 core + tests
- **[Phase 3: Movement (M3)](#m3-phase-3-movement--loot)** - 15 core + tests
- **[Phase 4: QoL (M4)](#m4-phase-4-quality-of-life)** - 18 core + tests
- **[Integration & Polish (M5)](#m5-integration--polish)** - 8 issues
- **[Release (M6)](#m6-release)** - 1 issue

---

## Master Epic

| # | Title | Status | Links |
|---|-------|--------|-------|
| **#790** | Epic: rgmercs Feature Parity & Lua/MacroQuest Plugin Support | 📋 | [View](https://github.com/Maleick/TextQuest/issues/790) |

---

## M0: Foundation & CI/CD
**Duration**: 2 weeks | **Team**: 2 engineers | **Effort**: ~34 hours

| # | Task | Type | Hours | Status |
|---|------|------|-------|--------|
| #1236 | Test harness & mocks | test-infrastructure | 8h | 📋 |
| #1237 | GitHub Actions CI/CD | ci-cd | 6h | 📋 |
| #1238 | Branch protection & auto-cleanup | ci-cd | 3h | 📋 |
| #1239 | Code coverage tracking | ci-cd | 4h | 📋 |
| #1241 | Integration test framework | test-infrastructure | 6h | 📋 |
| #1242 | Performance benchmarks | test-infrastructure | 5h | 📋 |
| #1243 | Testing documentation | test-infrastructure | 4h | 📋 |

**Deliverables**: CI/CD pipeline, test framework, branch management  
**Blocks**: All other phases

---

## M1: Phase 1 - Scripting Foundation
**Duration**: 6 weeks | **Team**: 3+QA | **Effort**: ~184 hours total

### Feature #791: Lua 5.4 VM Integration
**Status**: 📋 | **Effort**: ~70h core + testing

| # | Sub-Task | Hours | Depends On | Status |
|---|----------|-------|-----------|--------|
| #1003 | Add mlua dependency | 2h | - | 📋 |
| #1004 | Create LuaContext wrapper | 4h | #1003 | 📋 |
| #1005 | Script loading & execution | 6h | #1004 | 📋 |
| #1006 | Script lifecycle (reload, pause) | 8h | #1005 | 📋 |
| #1007 | Lua sandbox restrictions | 6h | #1004 | 📋 |
| #1008 | Expose TextQuest API to Lua | 16h | #1004,#1007 | 📋 |
| #1009 | Error handling & crash isolation | 8h | #1004,#1008 | 📋 |
| #1010 | Unit tests for Lua VM | 12h | All above | 📋 |
| #1011 | Integration tests & docs | 8h | All above | 📋 |

**Test Requirements**: Unit (80%+), integration, security, stress  
**Success**: Scripts execute safely, sandbox unbreakable

### Feature #792: MacroQuest Plugin Loader
**Status**: 📋 | **Effort**: ~56h core + testing

| # | Sub-Task | Hours | Depends On | Status |
|---|----------|-------|-----------|--------|
| #1012 | Research MQ2 API | 4h | - | 📋 |
| #1013 | Create FFI bindings | 12h | #1012 | 📋 |
| #1014 | Plugin discovery & loading | 8h | #1013 | 📋 |
| #1015 | Create MQ2 API bridge | 16h | #1014,#1013 | 📋 |
| #1016 | Plugin error handling | 8h | #1015 | 📋 |
| #1017 | Plugin tests & docs | 8h | #1016 | 📋 |

**Test Requirements**: Unit (80%+), 5+ real plugins, compatibility  
**Success**: Plugins load, no crashes, API translates correctly

### Feature #793: In-Game Hotkey System
**Status**: 📋 | **Effort**: ~38h core + testing

| # | Sub-Task | Hours | Depends On | Status |
|---|----------|-------|-----------|--------|
| #1020 | Hotkey registration infrastructure | 8h | - | 📋 |
| #1021 | Slash command system | 8h | #1020 | 📋 |
| #1024 | Integrate with Lua & plugins | 8h | #1021,#791,#792 | 📋 |
| #1026 | Built-in TextQuest commands | 8h | #1024 | 📋 |
| #1029 | Hotkey tests & docs | 6h | #1026 | 📋 |

**Test Requirements**: Unit (80%+), integration, stress, regression  
**Success**: Hotkeys work, commands execute, no TUI breakage

### Phase 1 Integration Tests
**Status**: 📋 | **Effort**: ~20 hours

| # | Task | Hours | Status |
|---|------|-------|--------|
| #1249a | Phase 1 integration tests | 12h | 📋 |
| #1249b | Stress tests (heavy load) | 8h | 📋 |

**Success**: All features work together, 0 crashes

---

## M2: Phase 2 - Combat Modules
**Duration**: 5 weeks | **Team**: 4+QA (parallel with M1 end) | **Effort**: ~174 hours total

### Feature #794: Clickies Module
**Status**: 📋 | **Effort**: ~47 hours

| # | Sub-Task | Hours | Status |
|---|----------|-------|--------|
| #1035 | Design clickies | 3h | 📋 |
| #1038 | Implement core logic | 12h | 📋 |
| #1040 | Configuration system | 8h | 📋 |
| #1043 | Integration with camp loop | 8h | 📋 |
| #1045 | Tests & documentation | 8h | 📋 |

**Test Requirements**: Unit (80%+), integration, scenario  
**Success**: Items click on schedule, cooldowns tracked

### Feature #799: Enhanced Charm/Pet Management
**Status**: 📋 | **Effort**: ~41 hours

| # | Sub-Task | Hours | Status |
|---|----------|-------|--------|
| #1049 | Design charm system | 3h | 📋 |
| #1051 | Charm spell automation | 8h | 📋 |
| #1053 | Pet control & commands | 10h | 📋 |
| #1054 | Configuration | 6h | 📋 |
| #1056 | Tests & documentation | 6h | 📋 |

**Test Requirements**: Unit (80%+), integration, scenario  
**Success**: Charms recast, pets respond to commands

### Feature #800: Improved Pull System
**Status**: 📋 | **Effort**: ~39 hours

| # | Sub-Task | Hours | Status |
|---|----------|-------|--------|
| #1060 | Design pulling system | 3h | 📋 |
| #1063 | Directional pulling | 12h | 📋 |
| #1064 | Multi-pull & adaptive | 10h | 📋 |
| #1066 | Tests & documentation | 6h | 📋 |

**Test Requirements**: Unit (80%+), integration, scenario  
**Success**: Mobs pull from positions, spacing correct

### Feature #796: Named NPC Tracking
**Status**: 📋 | **Effort**: ~39 hours

| # | Sub-Task | Hours | Status |
|---|----------|-------|--------|
| #1069 | Design named system | 3h | 📋 |
| #1070 | Named database & tracking | 10h | 📋 |
| #1073 | Combat integration | 12h | 📋 |
| #1074 | Tests & documentation | 6h | 📋 |

**Test Requirements**: Unit (80%+), integration, scenario  
**Success**: Named mobs tracked, special strategies applied

### Phase 2 Integration
**Effort**: ~10 hours | Tests: Cross-feature, regression

---

## M3: Phase 3 - Movement & Loot
**Duration**: 3 weeks | **Team**: 3+QA (parallel with M2) | **Effort**: ~105 hours

### Feature #795: Travel Module
**Status**: 📋 | **Effort**: ~33 hours

| # | Sub-Task | Hours | Status |
|---|----------|-------|--------|
| #1079 | Design travel | 3h | 📋 |
| #1082 | Portal database | 6h | 📋 |
| #1085 | Group travel coordination | 12h | 📋 |
| #1087 | Tests & documentation | 6h | 📋 |

**Test Requirements**: Unit (80%+), integration, scenario  
**Success**: Group travels together, stragglers handled

### Feature #798: Smart Loot Automation
**Status**: 📋 | **Effort**: ~37 hours

| # | Sub-Task | Hours | Status |
|---|----------|-------|--------|
| #1089 | Design smart loot | 3h | 📋 |
| #1090 | Advanced filtering | 10h | 📋 |
| #1094 | Loot and scoot | 12h | 📋 |
| #1097 | Tests & documentation | 6h | 📋 |

**Test Requirements**: Unit (80%+), integration, scenario  
**Success**: Correct items looted, zone timer enforced

### Feature #797: Drag Module
**Status**: 📋 | **Effort**: ~23 hours

| # | Sub-Task | Hours | Status |
|---|----------|-------|--------|
| #1098 | Design drag system | 3h | 📋 |
| #1101 | Corpse dragging | 12h | 📋 |
| #1105 | Tests & documentation | 4h | 📋 |

**Test Requirements**: Unit (80%+), integration, scenario  
**Success**: Corpses drag to location, stuck detection works

### Phase 3 Integration
**Effort**: ~8 hours | Tests: Cross-feature, regression

---

## M4: Phase 4 - Quality of Life
**Duration**: 6 weeks | **Team**: 4+QA (optional, parallel) | **Effort**: ~217 hours

### Feature #801: In-Game GUI Windows
**Status**: 📋 | **Effort**: ~88 hours (LARGE - graphics heavy)

| # | Sub-Task | Hours | Status |
|---|----------|-------|--------|
| #1104 | Design GUI overlay | 4h | 📋 |
| #1105 | DirectX hook (HIGH RISK) | 16h | 📋 |
| #1106 | Window manager & widgets | 20h | 📋 |
| #1107 | Input handling | 12h | 📋 |
| #1108 | Specific GUI windows | 16h | 📋 |
| #1109 | Tests & documentation | 8h | 📋 |

**Test Requirements**: Unit (80%+), performance <5% FPS impact  
**Success**: GUI renders, input works, performance acceptable

### Feature #802: Performance Monitoring
**Status**: 📋 | **Effort**: ~41 hours

| # | Sub-Task | Hours | Status |
|---|----------|-------|--------|
| #1111 | Design monitoring | 3h | 📋 |
| #1112 | Metrics collection | 14h | 📋 |
| #1113 | Historical tracking & alerts | 10h | 📋 |
| #1114 | Tests & documentation | 6h | 📋 |

**Test Requirements**: Unit (80%+), accuracy tests  
**Success**: Metrics accurate, dashboards update real-time

### Feature #803: Debug Tools
**Status**: 📋 | **Effort**: ~42 hours

| # | Sub-Task | Hours | Status |
|---|----------|-------|--------|
| #1115 | Memory inspection | 10h | 📋 |
| #1116 | Lua script debugging | 12h | 📋 |
| #1117 | Command/packet tracing | 8h | 📋 |
| #1118 | Tests & documentation | 6h | 📋 |

**Test Requirements**: Unit (80%+), scenario-based  
**Success**: Can debug scripts, inspect memory

### Feature #804: In-Game Help/FAQ
**Status**: 📋 | **Effort**: ~26 hours

| # | Sub-Task | Hours | Status |
|---|----------|-------|--------|
| #1119 | Design help system | 2h | 📋 |
| #1122 | Help interface | 8h | 📋 |
| #1124 | Help content | 12h | 📋 |

**Test Requirements**: Content accuracy, UI functionality  
**Success**: Help searchable, all topics covered

### Phase 4 Integration
**Effort**: ~20 hours | Tests: End-to-end 36-box scenarios

---

## M5: Integration & Polish
**Duration**: 2-3 weeks | **Team**: 2-3 engineers | **Effort**: ~70 hours

| # | Task | Hours | Status |
|---|------|-------|--------|
| #1244 | Code quality baseline | 8h | 📋 |
| #1246 | Error messages & logging | 6h | 📋 |
| #1247 | Refactor common patterns | 10h | 📋 |
| #1248 | Performance optimization | 12h | 📋 |
| #1249 | Cross-feature integration tests | 12h | 📋 |
| #1250 | End-to-end scenario tests | 16h | 📋 |
| #1251 | Documentation polish | 16h | 📋 |

**Deliverables**: Clean codebase, comprehensive tests, polished docs  
**Success**: Zero clippy warnings, 80%+ coverage, all scenarios pass

---

## M6: Release
**Duration**: 1 week | **Team**: 1-2 engineers | **Effort**: ~20 hours

| # | Task | Hours | Status |
|---|------|-------|--------|
| #1252 | Release workflow & versioning | 6h | 📋 |
| | Final testing & QA | 8h | 📋 |
| | Version tagging & release notes | 6h | 📋 |

**Deliverables**: Release checklist, versioning strategy  
**Success**: Clean release, automatic changelog

---

## 📊 Summary Statistics

### By Phase
| Phase | Features | Core Tasks | Total Effort | Duration |
|-------|----------|-----------|--------------|----------|
| **M0: Foundation** | - | 7 | 34h | 2w |
| **M1: Phase 1** | 3 | 20 | 184h | 6w |
| **M2: Phase 2** | 4 | 18 | 174h | 5w |
| **M3: Phase 3** | 3 | 15 | 105h | 3w |
| **M4: Phase 4** | 4 | 18 | 217h | 6w |
| **M5: Integration** | - | 7 | 70h | 2-3w |
| **M6: Release** | - | 1 | 20h | 1w |
| **TOTAL** | **14** | **86+** | **~804h** | **16-20w** |

### By Type
| Type | Count | Hours | % of Total |
|------|-------|-------|-----------|
| Core implementation | 71 | 542h | 67% |
| Testing | 30+ | 150h | 19% |
| Documentation | 10 | 80h | 10% |
| CI/CD & Release | 7 | 32h | 4% |
| **Total** | **101+** | **804h** | **100%** |

### Team Composition (Recommended)
- **4-6 core engineers** (Rust/game dev)
- **1 QA engineer** (testing automation)
- **1 DevOps engineer** (CI/CD)
- **1 Technical writer** (documentation)

**Total**: 7-9 people for 12-16 week execution

---

## 🏷️ All Labels Used

### Phase Labels
`phase-1` `phase-2` `phase-3` `phase-4`

### Priority Labels
`p0-critical` `p1-high` `p2-medium` `p3-low`

### Component Labels
`lua` `macroquest` `hotkeys` `clickies` `charm-pets` `pull` `named` `travel` `loot` `drag` `gui` `monitoring` `debug` `help` `ci-cd` `test-infrastructure` `polish` `documentation` `integration`

### Type Labels
`task` `testing` `documentation` `design` `research` `refactor` `release`

### Quality Labels
`size-xs` `size-s` `size-m` `size-l` `size-xl` `unsafe` `security` `stability` `performance`

### Status Labels
`📋` (Ready) `⚙️` (In Progress) `✅` (Complete)

---

## ✅ Definition of Done (Final Checklist)

### Code Requirements ✓
- [ ] Implementation complete
- [ ] All functions documented
- [ ] Error handling implemented
- [ ] No panics in production
- [ ] No unwrap() without explanation

### Testing Requirements ✓
- [ ] Unit tests written (80%+ coverage)
- [ ] Integration tests written
- [ ] Edge cases tested
- [ ] Error conditions tested
- [ ] CI passes (fmt, clippy, test)

### Quality Requirements ✓
- [ ] Code review approved
- [ ] No clippy warnings
- [ ] Consistent style (fmt)
- [ ] No dead code
- [ ] Logging added

### Documentation Requirements ✓
- [ ] Public API documented
- [ ] Configuration documented
- [ ] Examples provided
- [ ] Known limitations noted
- [ ] Related issues linked

### Performance Requirements ✓
- [ ] No significant regression
- [ ] No memory leaks
- [ ] Reasonable latency
- [ ] Benchmarks established

### Integration Requirements ✓
- [ ] Integrated with systems
- [ ] No breaking changes
- [ ] Backwards compatible
- [ ] Integration tests passing

---

## 🚀 Ready to Execute!

This comprehensive plan covers:
- ✅ 101+ GitHub issues
- ✅ 4-6 week foundation
- ✅ 12-16 week full execution
- ✅ 4-6 recommended team
- ✅ 80%+ test coverage requirement
- ✅ Automated CI/CD
- ✅ Branch management
- ✅ Release strategy

**Status**: 📋 Ready for team assignment and execution

---

**Document**: Final Comprehensive Issue Index  
**Version**: 2.0  
**Generated**: 2026-04-13  
**Branch**: `claude/rgmercs-feature-parity-H1dBp`  
**Last Updated**: [Timestamp of final commit]
