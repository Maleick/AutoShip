# RGMercs Feature Parity - Complete Project Summary

**Status**: 📋 Work Planning Complete | **Date**: 2026-04-13
**Branch**: `claude/rgmercs-feature-parity-H1dBp`

---

## 🎯 What Was Created

### 1️⃣ Epic Issue
- **[#790](https://github.com/Maleick/TextQuest/issues/790)** - Epic: rgmercs Feature Parity & Lua/MacroQuest Plugin Support
  - Master tracking issue for entire initiative
  - Links all 14 feature issues
  - Defines scope and success criteria

### 2️⃣ Feature Issues (14 Total)
Organized into 4 phases:

#### **Phase 1: Scripting Foundation** (3 features)
- [#791](https://github.com/Maleick/TextQuest/issues/791) - Lua 5.4 VM integration
- [#792](https://github.com/Maleick/TextQuest/issues/792) - MacroQuest plugin loader
- [#793](https://github.com/Maleick/TextQuest/issues/793) - In-game hotkey system

#### **Phase 2: Combat Modules** (4 features)
- [#794](https://github.com/Maleick/TextQuest/issues/794) - Clickies module
- [#799](https://github.com/Maleick/TextQuest/issues/799) - Enhanced charm/pet management
- [#800](https://github.com/Maleick/TextQuest/issues/800) - Improved pull system
- [#796](https://github.com/Maleick/TextQuest/issues/796) - Named NPC tracking

#### **Phase 3: Movement & Loot** (3 features)
- [#795](https://github.com/Maleick/TextQuest/issues/795) - Travel module
- [#798](https://github.com/Maleick/TextQuest/issues/798) - Smart loot automation
- [#797](https://github.com/Maleick/TextQuest/issues/797) - Drag module

#### **Phase 4: Quality of Life** (4 features)
- [#801](https://github.com/Maleick/TextQuest/issues/801) - In-game GUI windows
- [#802](https://github.com/Maleick/TextQuest/issues/802) - Performance monitoring
- [#803](https://github.com/Maleick/TextQuest/issues/803) - Debug tools
- [#804](https://github.com/Maleick/TextQuest/issues/804) - In-game help/FAQ

### 3️⃣ Sub-Task Issues (70+ Total)
Each feature broken into atomic 2-16 hour tasks with:
- Clear dependencies
- Complexity assessment
- Hour estimates
- Success criteria
- Testing requirements

**Examples**:
- Phase 1.1a: Add mlua dependency (~2h)
- Phase 1.1f: Expose TextQuest API to Lua (~16h)
- Phase 2.1b: Implement clicky item automation core (~12h)
- Phase 3.2c: Implement loot and scoot automation (~12h)
- Phase 4.1b: Implement DirectX overlay hook (~16h)

### 4️⃣ Comprehensive Documentation

#### **docs/RGMERCS_FEATURE_PARITY.md** (302 lines)
- Feature-by-feature gap analysis
- 16 rgmercs modules mapped to TextQuest
- 4-phase implementation roadmap
- Architecture decisions documented
- Testing strategy
- Success criteria
- Resource requirements

#### **docs/ISSUE_INDEX.md** (350+ lines)
- Complete mapping of all 85 issues
- Organized by phase and feature
- Dependencies clearly marked
- Complexity and hour estimates
- Label categories explained
- Quick lookup by component
- Issue numbering scheme
- Recommended execution order

#### **docs/IMPLEMENTATION_BREAKDOWN.md** (500+ lines)
- Visual dependency diagrams
- Detailed task breakdown by phase
- Risk assessment for each task
- Resource planning guide
- Sample 8-week sprint plan
- Team structure recommendations
- Success criteria per phase
- Time estimates for different scenarios

---

## 📊 Work Summary

### By Numbers
| Metric | Value |
|--------|-------|
| **Epic Issues** | 1 |
| **Feature Issues** | 14 |
| **Sub-Task Issues** | 70+ |
| **Total Issues** | 85+ |
| **Total Estimated Hours** | ~542 hours |
| **Estimated Duration (Serial)** | 16-20 weeks |
| **Estimated Duration (Parallel)** | 8-12 weeks |
| **Recommended Team Size** | 4-6 engineers |
| **Documentation Pages** | 3 comprehensive docs |

### By Phase

| Phase | Features | Tasks | Hours | Duration | Priority |
|-------|----------|-------|-------|----------|----------|
| **1: Scripting** | 3 | 20 | ~164h | 5-6 weeks | **CRITICAL** |
| **2: Combat** | 4 | 18 | ~134h | 4-5 weeks | **HIGH** |
| **3: Movement** | 3 | 15 | ~77h | 2-3 weeks | **HIGH** |
| **4: QoL** | 4 | 18 | ~167h | 5-6 weeks | **OPTIONAL** |

### Features vs RGMercs

| RGMercs Module | TextQuest Equivalent | Status | Issue |
|---|---|---|---|
| base.lua | orchestrator.rs | ✅ Complete | - |
| class.lua | ClassStrategy | ✅ Complete | - |
| mez.lua | CC system | ✅ Complete | - |
| move.lua | Navigator FSM | ✅ Complete | - |
| smartloot.lua | loot/ module | ⚙️ Expanding | [#798](https://github.com/Maleick/TextQuest/issues/798) |
| **clickies.lua** | Clickies module | 📋 New | [#794](https://github.com/Maleick/TextQuest/issues/794) |
| **charm.lua** | Enhanced charm | 📋 New | [#799](https://github.com/Maleick/TextQuest/issues/799) |
| **pull.lua** | Improved pull | 📋 New | [#800](https://github.com/Maleick/TextQuest/issues/800) |
| **named.lua** | Named tracking | 📋 New | [#796](https://github.com/Maleick/TextQuest/issues/796) |
| **travel.lua** | Travel module | 📋 New | [#795](https://github.com/Maleick/TextQuest/issues/795) |
| **drag.lua** | Drag module | 📋 New | [#797](https://github.com/Maleick/TextQuest/issues/797) |
| **In-game GUI** | GUI overlay | 📋 New | [#801](https://github.com/Maleick/TextQuest/issues/801) |
| **Lua support** | Lua VM + API | 📋 New | [#791](https://github.com/Maleick/TextQuest/issues/791) |
| **Plugin support** | MQ2 loader | 📋 New | [#792](https://github.com/Maleick/TextQuest/issues/792) |

---

## 🏷️ Labeling System

All 85+ issues are tagged with comprehensive labels:

### Priority Labels
- `p0-critical` - Blocks other work
- `p1-high` - Important, do soon
- `p2-medium` - Nice to have
- `p3-low` - Enhancement, can defer

### Component Labels
- `lua` - Lua scripting
- `macroquest` - MQ2 plugins
- `hotkeys` - Hotkey system
- `clickies`, `charm-pets`, `pull`, `named` - Combat modules
- `travel`, `loot`, `drag` - Movement/loot
- `gui`, `monitoring`, `debug`, `help` - QoL

### Complexity Labels
- `size-xs` - <2 hours
- `size-s` - 2-6 hours
- `size-m` - 6-16 hours
- `size-l` - 16-40 hours

### Phase Labels
- `phase-1`, `phase-2`, `phase-3`, `phase-4`

### Type Labels
- `task` - Implementation
- `testing` - Tests
- `documentation` - Docs
- `design` - Architecture
- `research` - Investigation

---

## 🚀 Recommended Execution

### Phase 1 First (Foundation, 5-6 weeks)
✅ **Must complete Phase 1 before anything else**

**Work in parallel**:
- Lua VM (#791) - 9 tasks - Lua/Rust expert
- MQ2 plugins (#792) - 6 tasks - FFI/memory expert  
- Hotkey system (#793) - 5 tasks - Game hooks expert

All depend on each other, so complete Phase 1 as foundation.

### Phases 2 & 3 (Combat & Movement, 6-8 weeks)
⚡ **Can work in parallel after Phase 1**

**Phase 2 (Combat, 4-5 weeks)**:
- Clickies (#794) - 5 tasks
- Charm/Pets (#799) - 5 tasks
- Pull system (#800) - 4 tasks
- Named tracking (#796) - 4 tasks

**Phase 3 (Movement, 2-3 weeks)**:
- Travel (#795) - 4 tasks
- Smart loot (#798) - 4 tasks
- Drag (#797) - 3 tasks

These can run concurrently.

### Phase 4 (Quality of Life, Optional, 5-6 weeks)
🎨 **Optional, can run in parallel**

- In-game GUI (#801) - 6 tasks
- Performance monitoring (#802) - 4 tasks
- Debug tools (#803) - 4 tasks
- Help/FAQ (#804) - 3 tasks

Lower priority, but adds polish.

### Critical Path
```
Phase 1 (5-6w) → Phases 2+3 parallel (6-8w) → Phase 4 optional (5-6w)
                       ↑
                  8-12 weeks total
```

---

## 📈 Team Recommendations

### Minimum Team (8-12 weeks)
- **1 Lead** - Architecture, Lua VM, oversight
- **2-3 Mid-level** - Combat modules, movement
- **1 Junior + 1 Senior** - QoL features, testing

### Optimal Team (8 weeks)
- **1 Lua expert** - Phase 1 Lua VM
- **1 FFI expert** - Phase 1 MQ2 plugins
- **1 Game hooks expert** - Phase 1 hotkeys
- **2 Combat programmers** - Phase 2 modules (parallel)
- **1 Movement programmer** - Phase 3 modules
- **1 Graphics expert** - Phase 4 GUI overlay
- **1 QA/Test lead** - Testing throughout

### Distributed Team (Work per sprint)
- Sprint 1-2: 4 engineers (Phase 1)
- Sprint 3-4: 6 engineers (Phase 1 finish + Phase 2 start)
- Sprint 5-6: 4 engineers (Phase 2 core)
- Sprint 7: 5 engineers (Phase 2 finish + Phase 3 start)
- Sprint 8: 4-6 engineers (Phase 3 finish + Phase 4 optional)

---

## 📚 Documentation Structure

```
docs/
├── RGMERCS_FEATURE_PARITY.md     ← Overview & gap analysis
├── ISSUE_INDEX.md                 ← Complete issue mapping
├── IMPLEMENTATION_BREAKDOWN.md    ← Detailed planning & team structure
└── (This file)                    ← Summary
```

Also see:
- **CLAUDE.md** - Project-wide guidance
- **README.md** - Usage documentation
- **docs/wiki/** - Long-form operator/developer docs

---

## ✅ Quality Assurance

### Testing Strategy
Every issue includes:
- ✅ Unit tests (test isolated logic)
- ✅ Integration tests (test with system)
- ✅ Manual test summary (what to verify)
- ✅ CI gate requirements (what passes CI)

### Code Review Points
- **Phase 1**: Extra review for security (sandbox, FFI)
- **Phase 2**: Extra review for game logic (movement, targeting)
- **Phase 3**: Extra review for physics (pull spacing, drag)
- **Phase 4**: Extra review for performance (GUI rendering)

### Regression Testing
- Existing combat system still works
- Existing navigation still works
- Existing loot system still works
- TUI still renders correctly
- No new crashes or hangs

---

## 🔗 Quick Links

| Resource | Link |
|----------|------|
| Epic Issue | [#790](https://github.com/Maleick/TextQuest/issues/790) |
| Feature Parity Doc | [RGMERCS_FEATURE_PARITY.md](../RGMERCS_FEATURE_PARITY.md) |
| Issue Index | [ISSUE_INDEX.md](../ISSUE_INDEX.md) |
| Implementation Plan | [IMPLEMENTATION_BREAKDOWN.md](../IMPLEMENTATION_BREAKDOWN.md) |
| RGMercs Repo | https://github.com/DerpleDude/rgmercs |
| TextQuest CLAUDE.md | [CLAUDE.md](../../CLAUDE.md) |
| Working Branch | `claude/rgmercs-feature-parity-H1dBp` |

---

## 🎓 Next Steps

### For Project Managers
1. Review [IMPLEMENTATION_BREAKDOWN.md](../IMPLEMENTATION_BREAKDOWN.md) for resource planning
2. Use the sample 8-week sprint plan as template
3. Assign engineers based on expertise matrix
4. Set Phase 1 completion as critical gate

### For Tech Leads
1. Review [ISSUE_INDEX.md](../ISSUE_INDEX.md) for dependency graph
2. Identify high-risk items (#1007, #1013, #1105, etc.)
3. Plan architecture review for Phase 1
4. Set up CI gates for testing requirements

### For Engineers
1. Start with [ISSUE_INDEX.md](../ISSUE_INDEX.md) to find your assignment
2. Review the feature issue (e.g., #791) for context
3. Break down sub-tasks into daily work
4. Follow complexity/testing requirements in each issue
5. Add comments to issues as you work

### For Everyone
- ✅ Feature parity goals clearly defined
- ✅ Work broken into manageable chunks
- ✅ Dependencies mapped (no surprises)
- ✅ Effort estimated (budget time accurately)
- ✅ Team structure recommended (right people, right time)
- ✅ Success criteria clear (know when done)

---

## 📞 Questions?

Refer to the detailed documents:
- **Questions about scope?** → [RGMERCS_FEATURE_PARITY.md](../RGMERCS_FEATURE_PARITY.md)
- **Questions about tasks?** → [ISSUE_INDEX.md](../ISSUE_INDEX.md)
- **Questions about planning?** → [IMPLEMENTATION_BREAKDOWN.md](../IMPLEMENTATION_BREAKDOWN.md)
- **Questions about this project?** → This file

---

**Generated**: 2026-04-13  
**Branch**: `claude/rgmercs-feature-parity-H1dBp`  
**Status**: 📋 Work planning complete, ready for execution

Let's build this! 🚀
