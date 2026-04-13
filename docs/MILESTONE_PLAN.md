# RGMercs Feature Parity - Milestone Plan & Execution Guide

**Version**: 2.0 (With Test Infrastructure & Polish)  
**Date**: 2026-04-13  
**Status**: 📋 Ready for Execution

---

## 📅 Milestone Structure (GitHub Milestones)

### **M0: Foundation & CI/CD** (2 weeks)
**Goal**: Set up infrastructure for all phases

**Issues**:
- [#1236](https://github.com/Maleick/TextQuest/issues/1236) - Test harness & mocks
- [#1237](https://github.com/Maleick/TextQuest/issues/1237) - GitHub Actions CI/CD
- [#1238](https://github.com/Maleick/TextQuest/issues/1238) - Branch protection & auto-cleanup
- [#1239](https://github.com/Maleick/TextQuest/issues/1239) - Code coverage tracking
- [#1241](https://github.com/Maleick/TextQuest/issues/1241) - Integration test framework
- [#1242](https://github.com/Maleick/TextQuest/issues/1242) - Performance benchmarks
- [#1243](https://github.com/Maleick/TextQuest/issues/1243) - Testing documentation

**Team**: 2 engineers (DevOps + QA lead)  
**Deliverables**:
- ✅ CI/CD pipeline working
- ✅ Branch protection enabled
- ✅ Auto-cleanup configured
- ✅ Test framework ready
- ✅ Coverage tracking enabled

**Success Criteria**:
- [ ] All CI checks pass
- [ ] Auto-cleanup working on PR merge
- [ ] Test suite runs successfully
- [ ] Coverage >80%

---

### **M1: Phase 1 - Scripting Foundation** (6 weeks)

#### **1.1: Lua 5.4 VM** (20 tasks)
**Goal**: Enable Lua script execution with sandbox

**Core Tasks**:
- [#1003-#1011](https://github.com/Maleick/TextQuest/issues/1003) - Lua VM implementation + tests

**Test Requirements**:
- Unit tests: VM init, sandbox, API bindings (80%+ coverage)
- Integration tests: Real scripts, lifecycle
- Security tests: Sandbox breakouts
- Stress tests: 1000s of scripts

**Team**: 1 Lua/Rust expert  
**Effort**: ~70 hours

#### **1.2: MacroQuest Plugins** (14 tasks)
**Goal**: Load and execute MQ2 plugins

**Core Tasks**:
- [#1012-#1017](https://github.com/Maleick/TextQuest/issues/1012) - MQ2 plugin loader + tests

**Test Requirements**:
- Unit tests: Type conversions, API calls (80%+ coverage)
- Integration tests: Real plugins, lifecycle
- Compatibility tests: 5+ popular plugins
- Stress tests: Multiple plugins, long runs

**Team**: 1 FFI/memory expert  
**Effort**: ~56 hours

#### **1.3: Hotkey System** (13 tasks)
**Goal**: Register hotkeys and commands

**Core Tasks**:
- [#1020, #1021, #1024, #1026, #1029](https://github.com/Maleick/TextQuest/issues/1020) - Hotkey system + tests

**Test Requirements**:
- Unit tests: Parsing, registry, execution (80%+ coverage)
- Integration tests: Hotkey from Lua, plugin
- Stress tests: Rapid hotkey presses
- Regression tests: Existing TUI unaffected

**Team**: 1 game hooks expert  
**Effort**: ~38 hours

#### **Phase 1 Integration**
- [#1249](https://github.com/Maleick/TextQuest/issues/1249) - Phase 1 integration tests

**Test Requirements**:
- Lua + Plugins together work
- Hotkeys trigger scripts/plugins
- No crashes or hangs
- Performance acceptable

**Team**: QA lead  
**Effort**: ~8 hours

**Phase 1 Total**:
- **Tasks**: 20 core + integration tests
- **Effort**: ~164 hours core + ~20 hours testing = **~184 hours**
- **Team**: 3 engineers + 1 QA
- **Duration**: 6 weeks
- **Delivery**: End of week 6

---

### **M2: Phase 2 - Combat Modules** (5 weeks)

#### **2.1: Clickies** (5 tasks)
**Goal**: Automate consumable item usage

**Core Tasks**: [#1035-#1045](https://github.com/Maleick/TextQuest/issues/1035)

**Test Requirements**:
- Unit tests: Item logic, cooldowns, conditions (80%+ coverage)
- Integration tests: With camp loop
- Scenario tests: Actual combat situations
- Regression tests: Existing combat unaffected

**Team**: 1 engineer  
**Effort**: ~39 hours + ~8 hours testing = **~47 hours**

#### **2.2: Charm/Pets** (5 tasks)
**Goal**: Pet and charm spell automation

**Core Tasks**: [#1049-#1056](https://github.com/Maleick/TextQuest/issues/1049)

**Test Requirements**:
- Unit tests: Charm logic, pet control (80%+ coverage)
- Integration tests: Combat system
- Scenario tests: Real charm/pet scenarios
- Regression tests: Existing combat unaffected

**Team**: 1 engineer  
**Effort**: ~33 hours + ~8 hours testing = **~41 hours**

#### **2.3: Pull System** (4 tasks)
**Goal**: Directional and multi-pull automation

**Core Tasks**: [#1060-#1066](https://github.com/Maleick/TextQuest/issues/1060)

**Test Requirements**:
- Unit tests: Pull patterns, positioning (80%+ coverage)
- Integration tests: With navigation
- Scenario tests: Multi-pull situations
- Regression tests: Existing pull unaffected

**Team**: 1 engineer  
**Effort**: ~31 hours + ~8 hours testing = **~39 hours**

#### **2.4: Named Tracking** (4 tasks)
**Goal**: Named NPC encounter tracking

**Core Tasks**: [#1069-#1074](https://github.com/Maleick/TextQuest/issues/1069)

**Test Requirements**:
- Unit tests: Named detection, tracking (80%+ coverage)
- Integration tests: Combat system
- Scenario tests: Named encounters
- Regression tests: Existing combat unaffected

**Team**: 1 engineer  
**Effort**: ~31 hours + ~8 hours testing = **~39 hours**

#### **Phase 2 Integration**
- Cross-feature integration tests
- Regression test suite

**Team**: QA lead  
**Effort**: ~10 hours

**Phase 2 Total**:
- **Tasks**: 18 core + integration tests
- **Effort**: ~134 hours core + ~40 hours testing = **~174 hours**
- **Team**: 4 engineers + 1 QA (parallel)
- **Duration**: 5 weeks
- **Delivery**: End of week 11 (if parallel with Phase 1 end)

---

### **M3: Phase 3 - Movement & Loot** (3 weeks)

#### **3.1: Travel** (4 tasks)
**Goal**: Portal coordination and group travel

**Core Tasks**: [#1079-#1087](https://github.com/Maleick/TextQuest/issues/1079)

**Test Requirements**:
- Unit tests: Portal lookup, travel logic (80%+ coverage)
- Integration tests: With navigation
- Scenario tests: Group travel
- Regression tests: Existing nav unaffected

**Team**: 1 engineer  
**Effort**: ~27 hours + ~6 hours testing = **~33 hours**

#### **3.2: Smart Loot** (4 tasks)
**Goal**: Loot filtering and "loot and scoot"

**Core Tasks**: [#1089-#1097](https://github.com/Maleick/TextQuest/issues/1089)

**Test Requirements**:
- Unit tests: Filtering logic (80%+ coverage)
- Integration tests: With loot system
- Scenario tests: Loot and scoot
- Regression tests: Existing loot unaffected

**Team**: 1 engineer  
**Effort**: ~31 hours + ~6 hours testing = **~37 hours**

#### **3.3: Drag** (3 tasks)
**Goal**: Corpse dragging automation

**Core Tasks**: [#1098-#1105](https://github.com/Maleick/TextQuest/issues/1098)

**Test Requirements**:
- Unit tests: Drag logic, stuck detection (80%+ coverage)
- Integration tests: With movement
- Scenario tests: Corpse dragging
- Regression tests: Existing nav unaffected

**Team**: 1 engineer  
**Effort**: ~19 hours + ~4 hours testing = **~23 hours**

#### **Phase 3 Integration**
- Cross-feature integration tests
- Regression test suite

**Team**: QA lead  
**Effort**: ~8 hours

**Phase 3 Total**:
- **Tasks**: 15 core + integration tests
- **Effort**: ~77 hours core + ~28 hours testing = **~105 hours**
- **Team**: 3 engineers + 1 QA (parallel)
- **Duration**: 3 weeks
- **Delivery**: End of week 14 (if parallel with Phase 2 end)

---

### **M4: Phase 4 - Quality of Life** (6 weeks)

#### **4.1: GUI Overlay** (6 tasks)
**Goal**: In-game overlay GUI windows

**Core Tasks**: [#1104-#1109](https://github.com/Maleick/TextQuest/issues/1104)

**Test Requirements**:
- Unit tests: Window logic, widgets (80%+ coverage)
- Integration tests: With game, input
- Performance tests: FPS impact <5%
- Stress tests: Many windows, rapid updates

**Team**: 1 graphics expert  
**Effort**: ~76 hours + ~12 hours testing = **~88 hours**

#### **4.2: Monitoring** (4 tasks)
**Goal**: Performance metrics and dashboards

**Core Tasks**: [#1111-#1114](https://github.com/Maleick/TextQuest/issues/1111)

**Test Requirements**:
- Unit tests: Metric calculations (80%+ coverage)
- Integration tests: Data collection
- Accuracy tests: Compare to manual
- Long-run tests: 24h continuous

**Team**: 1 engineer  
**Effort**: ~33 hours + ~8 hours testing = **~41 hours**

#### **4.3: Debug Tools** (4 tasks)
**Goal**: Memory and script debugging

**Core Tasks**: [#1115-#1118](https://github.com/Maleick/TextQuest/issues/1115)

**Test Requirements**:
- Unit tests: Inspection functions (80%+ coverage)
- Integration tests: Debug hooks
- Scenario tests: Debug actual bugs
- Regression tests: No overhead when disabled

**Team**: 1 engineer  
**Effort**: ~36 hours + ~6 hours testing = **~42 hours**

#### **4.4: Help/FAQ** (3 tasks)
**Goal**: In-game help system

**Core Tasks**: [#1119-#1124](https://github.com/Maleick/TextQuest/issues/1119)

**Test Requirements**:
- Content tests: Links, completeness
- UI tests: Search functionality
- Accessibility tests: Readability

**Team**: 1 technical writer + 1 engineer  
**Effort**: ~22 hours + ~4 hours testing = **~26 hours**

#### **Phase 4 Integration**
- End-to-end scenario tests
- 36-box farm simulation

**Team**: QA lead  
**Effort**: ~20 hours

**Phase 4 Total**:
- **Tasks**: 18 core + integration tests
- **Effort**: ~167 hours core + ~50 hours testing = **~217 hours**
- **Team**: 4 engineers + 1 QA (parallel, optional)
- **Duration**: 6 weeks
- **Delivery**: End of week 20 (optional)

---

### **M5: Integration & Polish** (2-3 weeks)

**Goal**: Cross-feature integration and code quality

**Issues**:
- [#1244](https://github.com/Maleick/TextQuest/issues/1244) - Code quality baseline
- [#1246](https://github.com/Maleick/TextQuest/issues/1246) - Error messages & logging
- [#1247](https://github.com/Maleick/TextQuest/issues/1247) - Refactor common patterns
- [#1248](https://github.com/Maleick/TextQuest/issues/1248) - Performance optimization
- [#1249](https://github.com/Maleick/TextQuest/issues/1249) - Cross-feature integration tests
- [#1250](https://github.com/Maleick/TextQuest/issues/1250) - End-to-end scenario tests (36-box)
- [#1251](https://github.com/Maleick/TextQuest/issues/1251) - Documentation polish

**Team**: 2-3 engineers + QA lead  
**Effort**: ~60-80 hours  
**Duration**: 2-3 weeks

---

### **M6: Release** (1 week)

**Goal**: Prepare for public release

**Issues**:
- [#1252](https://github.com/Maleick/TextQuest/issues/1252) - Release workflow & versioning

**Tasks**:
- [ ] Final testing
- [ ] Version tagging
- [ ] Changelog generation
- [ ] Release notes
- [ ] Deployment
- [ ] Monitoring

**Team**: 1-2 engineers + release manager  
**Effort**: ~20 hours  
**Duration**: 1 week

---

## 📊 Total Effort Summary

| Milestone | Duration | Effort | Team | Status |
|-----------|----------|--------|------|--------|
| **M0: Foundation** | 2 weeks | ~34h | 2 | Must do first |
| **M1: Phase 1** | 6 weeks | ~184h | 3+QA | Parallel after M0 |
| **M2: Phase 2** | 5 weeks | ~174h | 4+QA | Parallel with M1 end |
| **M3: Phase 3** | 3 weeks | ~105h | 3+QA | Parallel with M2 start |
| **M4: Phase 4** | 6 weeks | ~217h | 4+QA | Optional, parallel |
| **M5: Integration** | 2-3 weeks | ~70h | 2-3 | After phases 1-3 |
| **M6: Release** | 1 week | ~20h | 1-2 | Final stage |
| **TOTAL** | **16-20 weeks** | **~800h** | **4-6 avg** | **Ready** |

---

## 🎯 Execution Timeline (Recommended)

```
Week 1-2:     M0 (Foundation & CI/CD)
              ↓
Week 2-7:     M1 (Phase 1 - Scripting) + M0 completion
              ↓
Week 6-10:    M2 (Phase 2 - Combat) starts before M1 ends
              ↓
Week 9-12:    M3 (Phase 3 - Movement) starts before M2 ends
              ↓
Week 10-15:   M4 (Phase 4 - QoL) optional, parallel
              ↓
Week 13-15:   M5 (Integration & Polish)
              ↓
Week 16:      M6 (Release)

CRITICAL PATH: M0 → M1 → M5 → M6 (16 weeks)
OPTIMAL PATH: M0 → M1||M2||M3 → M5 → M6 (12-14 weeks with good parallelization)
```

---

## 🔄 Branch Management Strategy

### Branch Naming Convention
- `feature/phase-1-lua` - Feature branches
- `feature/issue-1003-mlua-dependency` - Issue branches
- `bugfix/issue-1234` - Bug fix branches
- `docs/issue-1251-polish` - Documentation branches

### Merge Strategy
- **Default**: Squash & merge (keeps main clean)
- **Exceptions**: Large features use merge commits
- **Auto-cleanup**: Enabled (delete head branch after merge)

### Branch Protection Rules
- ✅ Require PR reviews (1+ approval)
- ✅ Require status checks to pass
- ✅ Require branches up to date
- ✅ Dismiss stale reviews
- ✅ Auto-delete head branch on merge

---

## 📋 Definition of Done (Checklist)

### For Each Issue
- [ ] All work items completed
- [ ] All tests written and passing (80%+ coverage)
- [ ] Code reviewed and approved
- [ ] No clippy warnings
- [ ] Code formatted (rustfmt)
- [ ] Documentation updated
- [ ] Links to parent/related issues added
- [ ] Performance impact assessed
- [ ] Committed to correct branch

### For Each Milestone
- [ ] All issues in milestone complete
- [ ] All integration tests passing
- [ ] No regressions in existing features
- [ ] Performance benchmarks acceptable
- [ ] Documentation complete for milestone
- [ ] Milestone marked as complete

---

## ✅ Quality Gates

### CI/CD Gate (All PRs)
```
✅ cargo fmt --check      (Code formatting)
✅ cargo clippy           (Linting)
✅ cargo test             (Unit & integration tests)
✅ cargo coverage >80%    (Code coverage)
✅ cargo bench            (Performance benchmarks)
```

### Phase Completion Gate
```
✅ All phase issues resolved
✅ All phase tests passing
✅ Code review approved
✅ Integration tests passing
✅ No regressions
✅ Documentation complete
✅ Performance acceptable
```

### Release Gate
```
✅ All milestones complete
✅ M5 integration tests passing
✅ M6 final testing complete
✅ Version tagged
✅ Changelog complete
✅ Release notes written
```

---

## 🚀 Getting Started

### Week 1: Setup Phase
1. **Create milestones** in GitHub (M0-M6)
2. **Assign issues** to milestones
3. **Set up CI/CD** (M0 issues)
4. **Assign team members**

### Week 2: Start Execution
1. **M0**: Infrastructure setup
2. **Begin M1**: Lua VM work
3. **Parallel prep**: Phase 2 team reviews design

### Ongoing
- **Daily**: Stand-ups, PR reviews
- **Weekly**: Milestone progress check
- **Bi-weekly**: Sprint planning/retro
- **Monthly**: Major milestones review

---

## 📞 Team Communication

### Daily
- Stand-up: 15 min (status, blockers)
- PR reviews: Async (2-4 hour SLA)

### Weekly
- Milestone review: 30 min
- Technical discussion: As needed

### Bi-weekly
- Sprint planning: 1 hour
- Retrospective: 45 min

### Monthly
- All-hands: 30 min (progress, big wins)

---

## 🎓 Test Coverage Requirements

All issues must achieve:
- **Unit tests**: 80%+ code coverage
- **Integration tests**: All critical paths
- **E2E tests**: Major scenarios
- **Performance**: No regression

### Coverage by Phase
| Phase | Target | Method |
|-------|--------|--------|
| Phase 1 | 85%+ | Critical (sandbox, FFI) |
| Phase 2 | 80%+ | Scenario-based |
| Phase 3 | 80%+ | Integration-heavy |
| Phase 4 | 75%+ | UI + integration |

---

**Version**: 2.0  
**Status**: 📋 Ready for team execution  
**Last Updated**: 2026-04-13  
**Branch**: `claude/rgmercs-feature-parity-H1dBp`
