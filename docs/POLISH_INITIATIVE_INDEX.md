# Polish Initiative: Master Index & Quick Reference

**Single source of truth** for the entire polish initiative. All issues, epics, tasks, dependencies, and milestones are listed here.

---

## Quick Start

**Starting implementation?** Begin here:

1. Read this index (you are here)
2. Review #1181 for detailed task breakdown
3. Check #998 for roadmap and phase gates
4. Check #999 for gap analysis
5. Start with Week 1 tasks (Help + Theme systems)

**Looking for a specific task?** Use the tables below to find issue numbers and dependencies.

---

## Milestones at a Glance

| Week    | Focus                     | Tasks | Status  | Issues                   |
| ------- | ------------------------- | ----- | ------- | ------------------------ |
| **1**   | Help + Theme Systems      | 11    | Pending | #1080-#1117, #1126-#1133 |
| **2**   | Metrics + Audio/Hotkeys   | 12+   | Pending | #1142-#1150, #1178-#1179 |
| **3**   | Web Foundation + Debugger | 12+   | Pending | #1160-#1176, #1000       |
| **4-5** | Web Features              | 10+   | Pending | #1171-#1173              |
| **5-6** | Real-Time Monitoring      | 5+    | Pending | #1174, #1176             |

---

## Main Documentation Issues

- **#998** - Complete Polish Roadmap & Tracking
- **#999** - Gap Analysis & Missing Features
- **#1181** - Comprehensive Task Breakdown with Milestones & Tags
- **#1182** - Master Index & Quick Reference (this document)

---

## Epic Structure

### Epic #970: TUI Polish & User Experience

- #971: In-Game Help & FAQ
- #975: Performance Monitoring
- #989: Hotkey & Audio Alert Systems
- #993: Enhanced Error Messages
- #994: Theme System
- #997: Keyboard Shortcuts & Accessibility

### Epic #978: Web Dashboard Foundation

- #979: Frontend Build Setup
- #981: Credential Management UI
- #983: Group & Camp Configuration
- #986: Session Monitoring Dashboard

### Epic #987: In-Game Overlay GUI Windows

- #988: Direct3D Rendering System

### Epic #995: Debugger & Packet Tools Integration

- #996: Extract Debugger from Test Branch

---

## Detailed Task Matrix

### Week 1: Help System (6 Tasks)

| Issue | Title                     | Type | Est. Hours | Status |
| ----- | ------------------------- | ---- | ---------- | ------ |
| #1080 | HelpDatabase structures   | task | 2          | Ready  |
| #1086 | TOML format & parser      | task | 3          | Ready  |
| #1095 | Load into App state       | task | 2          | Ready  |
| #1103 | Search & fuzzy matching   | task | 4          | Ready  |
| #1110 | HelpPanelState management | task | 2          | Ready  |
| #1117 | Render help panel         | task | 5          | Ready  |

**Dependencies**: Sequential chain (#1080 → #1086 → #1095 → #1103 → #1110 → #1117)

### Week 1: Theme System (5 Tasks)

| Issue | Title                    | Type | Est. Hours | Status    |
| ----- | ------------------------ | ---- | ---------- | --------- |
| #1126 | Color palette structures | task | 3          | Ready     |
| #1133 | TOML files & loader      | task | 3          | Ready     |
| TBD   | Apply to components      | task | 6          | To Create |
| TBD   | Theme toggle command     | task | 2          | To Create |
| TBD   | Persist selection        | task | 1          | To Create |

**Dependencies**: #1126 → #1133 → Apply → Toggle → Persist

### Week 2: Metrics System (6+ Tasks)

| Issue | Title                    | Type | Est. Hours | Status    |
| ----- | ------------------------ | ---- | ---------- | --------- |
| #1142 | Data structures          | task | 3          | Ready     |
| #1150 | Collector & aggregation  | task | 5          | Ready     |
| TBD   | Orchestrator integration | task | 3          | To Create |
| TBD   | Event hooks              | task | 4          | To Create |
| TBD   | TUI panel                | task | 6          | To Create |
| TBD   | SQLite persistence       | task | 4          | To Create |

**Dependencies**: #1142 → #1150 → Wire → Hook → Panel → Persist

### Week 2: Audio & Hotkeys (6+ Tasks)

| Issue | Title              | Type | Est. Hours | Status    |
| ----- | ------------------ | ---- | ---------- | --------- |
| #1178 | Audio playback     | task | 4          | Ready     |
| TBD   | Default sounds     | task | 2          | To Create |
| TBD   | Event routing      | task | 3          | To Create |
| #1179 | Hotkey registry    | task | 5          | Ready     |
| TBD   | Conflict detection | task | 2          | To Create |
| TBD   | Configuration UI   | task | 3          | To Create |

**Dependencies**: #1178 → Sounds → Route (parallel); #1179 → Conflict → Config UI

### Week 3: Web Foundation (6+ Tasks)

| Issue | Title                 | Type | Est. Hours | Status    |
| ----- | --------------------- | ---- | ---------- | --------- |
| #1160 | Vite + React scaffold | task | 3          | Ready     |
| #1165 | API client library    | task | 4          | Ready     |
| #1166 | Component library     | task | 8          | Ready     |
| TBD   | Form handling         | task | 3          | To Create |
| TBD   | TypeScript config     | task | 2          | To Create |
| TBD   | Dark mode theming     | task | 3          | To Create |

**Dependencies**: #1160 is base; #1165 & #1166 depend on #1160; others parallel

### Week 3: Debugger Integration (6+ Tasks)

| Issue | Title                | Type | Est. Hours | Status    |
| ----- | -------------------- | ---- | ---------- | --------- |
| TBD   | Diagnostic panel     | task | 3          | To Create |
| TBD   | Error context        | task | 4          | To Create |
| TBD   | Recovery suggestions | task | 2          | To Create |
| #1000 | Extract debugger     | task | 5          | To Create |
| TBD   | Integrate debugger   | task | 3          | To Create |
| TBD   | Packet monitor UI    | task | 4          | To Create |

**Dependencies**: #1000 must complete before Integrate

### Week 4-5: Web Pages (5+ Tasks)

| Issue | Title               | Type | Est. Hours | Status    |
| ----- | ------------------- | ---- | ---------- | --------- |
| #1171 | Credentials page    | task | 6          | Ready     |
| #1173 | Groups & camps page | task | 8          | Ready     |
| TBD   | Navigation/routing  | task | 3          | To Create |
| TBD   | Form persistence    | task | 3          | To Create |
| TBD   | API integration     | task | 5          | To Create |

**Dependencies**: #1171 & #1173 require #1166 & #1165; API integration last

### Week 5-6: Real-Time Monitoring (5+ Tasks)

| Issue | Title             | Type | Est. Hours | Status    |
| ----- | ----------------- | ---- | ---------- | --------- |
| #1176 | useWebSocket hook | task | 3          | Ready     |
| #1174 | Session dashboard | task | 8          | Ready     |
| TBD   | Real-time updates | task | 3          | To Create |
| TBD   | Alert feed        | task | 3          | To Create |
| TBD   | Mobile layout     | task | 4          | To Create |

**Dependencies**: #1176 before #1174; Events/Alerts → #1174; Mobile last

### Accessibility (6+ Tasks)

| Issue | Title            | Type | Est. Hours | Status    |
| ----- | ---------------- | ---- | ---------- | --------- |
| TBD   | Shortcut map     | task | 2          | To Create |
| TBD   | Vi mode          | task | 3          | To Create |
| TBD   | Emacs mode       | task | 3          | To Create |
| TBD   | Focus indicators | task | 2          | To Create |
| TBD   | High contrast    | task | 2          | To Create |
| TBD   | Documentation    | task | 2          | To Create |

---

## Tag Reference

### Domain Tags (Use on all issues)

- `help-system` - Help/documentation features
- `theme-system` - Theming and customization
- `metrics` - Performance metrics
- `audio-alerts` - Audio notification system
- `hotkeys` - Hotkey registration
- `web-frontend` - Web dashboard frontend
- `debugging` - Debugger and diagnostics
- `accessibility` - Keyboard/accessibility features

### Type Tags

- `task` - Concrete work
- `infrastructure` - Backend plumbing
- `ui` - User interface
- `state-management` - State handling
- `search` - Search functionality
- `monitoring` - Real-time monitoring
- `assets` - Audio/visual assets

### Priority Tags

- `high-priority` - Critical path
- `agent:ready` - Autonomous work candidate
- `blocked` - Waiting on dependency

### Status Tags (Milestone)

- `week-1`, `week-2`, `week-3`, `week-4-5`, `week-5-6`

---

## Statistics

### Issues by Status

- ✅ Created: 20 (#1080-#1181)
- ❓ To Create: 32+ (marked TBD)
- 📋 Total Planned: 52+

### Hours Estimation

- Week 1: 18 hours (Help) + 15 hours (Theme) = 33 hours
- Week 2: 22 hours (Metrics) + 17 hours (Audio/Hotkeys) = 39 hours
- Week 3: 23 hours (Web) + 21 hours (Debugger) = 44 hours
- Week 4-5: 25 hours (Pages)
- Week 5-6: 21 hours (Monitoring)
- **Total: 177 estimated hours (~6 weeks)**

---

## Next Steps

### Immediate (Today)

1. ✅ Review all created issues
2. ⏳ Create remaining (TBD) issues from #1181
3. ⏳ Assign all issues to milestones in GitHub UI
4. ⏳ Apply domain/type/priority tags to all issues

### This Week

5. ⏳ Mark #1080-#1133 as `agent:ready`
6. ⏳ Link dependent issues
7. ⏳ Begin Week 1 parallel work
8. ⏳ Set up GitHub Projects for tracking

### Ongoing

- Track progress by milestone
- Update issues with blockers immediately
- Link PRs to related issues
- Review & merge PRs for each task
- Weekly progress check-in

---

## Key Decision Points

1. **Parallel Work**: Help System and Theme System can run in parallel (Week 1)
2. **Web First**: Complete web foundation before pages (Week 3 before 4)
3. **Dependencies**: Metrics collection before dashboard; WebSocket before real-time
4. **Content**: Help content writing is separate (outside these tasks)
5. **Audio Files**: Default sounds need to be sourced/created

---

## How to Find Things

**By Epic**: Check section above or search GitHub label
**By Milestone**: Use GitHub milestone filter (Week 1, 2, 3, etc.)
**By Domain**: Use domain tag (e.g., `web-frontend`)
**By Status**: See roadmap #998 or this index
**By Dependencies**: Check #1181 dependency graph section

---

## Critical Dependencies

```
Help System: Sequential (each task blocks next)
Theme System: Sequential (structures → loader → apply)
Metrics: Sequential (structure → collector → integration)
Audio/Hotkeys: Parallel trees (audio tree + hotkey tree)
Web: Star pattern (scaffold → API, Components, Config)
Web Pages: Need API + Components
Real-Time: Needs WebSocket hook + Dashboard
```

---

## Questions?

- **What should I work on?** Start with #1080 (Week 1, Help System)
- **Which tasks are independent?** Theme tasks can run parallel to Help
- **How do I know when done?** Check acceptance criteria in each issue
- **What if blocked?** Document in issue, escalate to #1181 comment thread
- **Need to reestimate?** Update hours in issue and comment with reason

---

## Related Documents

- Full Roadmap: #998
- Gap Analysis: #999
- Detailed Breakdown: #1181
- This Index: #1182 (YOU ARE HERE)

---

**Last Updated**: 2026-04-13  
**Status**: Planning Complete  
**Next**: Create (TBD) issues and begin Week 1 work
