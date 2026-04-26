# OpenVanilla Parity — Complete Issue Index & Roadmap

**Issue**: #1177  
**Last Updated**: 2026-04-25  
**Total Issues**: 4 Milestones + 31 Feature Issues + 14 Concrete Sub-Issues (M7/M8 Slice 1)  
**Reference**: [`openvanilla-macroquest-coverage-gap-plan.md`](openvanilla-macroquest-coverage-gap-plan.md)

---

## Milestone Overview

| Milestone                                                 | Title                                 | Points  | Duration  | Status         |
| --------------------------------------------------------- | ------------------------------------- | ------- | --------- | -------------- |
| [M7 #1102](#m7-plugin-ecosystem--core-extensibility-1102) | Plugin Ecosystem & Core Extensibility | 100–140 | 8–12 wks  | Foundation     |
| [M8 #1111](#m8-combat--coordination-hardening-1111)       | Combat & Coordination Hardening       | 225–290 | 14–20 wks | Primary        |
| [M9 #1115](#m9-automation--learning-systems-1115)         | Automation & Learning Systems         | 120–155 | 10–14 wks | Automation     |
| [M10 #1121](#m10-economy--inventory-management-1121)      | Economy & Inventory Management        | 215–270 | 16–22 wks | Optimization   |
| [M11 #1123](#m11-system-stability--integration-1123)      | System Stability & Integration        | 45–55   | 4–6 wks   | Infrastructure |

**Depends On**: M8 → M7 complete. M9 + M10 → M8 stable (can overlap). M11 → all milestones.

---

## M7: Plugin Ecosystem & Core Extensibility (#1102)

**Goal**: Enable community plugins and Lua scripting for extensibility.

### Slice 1: Plugin System (#819)

| Issue | Title                                        | Tags                                         | Points |
| ----- | -------------------------------------------- | -------------------------------------------- | ------ |
| #1131 | Plugin Architecture Design Research          | `plugin-system` `research` `architecture`    | 8–12   |
| #1135 | Plugin Loader Implementation                 | `plugin-system` `implementation`             | 12–18  |
| #1139 | Core Plugin API Surface (Combat, Nav, State) | `plugin-system` `implementation`             | 12–18  |
| #1144 | First Community Plugin & Developer Guide     | `plugin-system` `validation` `documentation` | 8–12   |

**Dependencies**: None (foundational). **Effort**: 40–60 pts.

### Slice 2: Lua Scripting (#820)

| Issue | Title                               | Tags                               | Points |
| ----- | ----------------------------------- | ---------------------------------- | ------ |
| #1151 | Lua Integration Strategy & Research | `lua` `research`                   | 6–10   |
| #1156 | Lua Interpreter & Script Loader     | `lua` `implementation`             | 10–14  |
| #1161 | Core Lua API Bindings               | `lua` `implementation`             | 10–12  |
| #1164 | Example Lua Scripts & Library       | `lua` `validation` `documentation` | 4–6    |

**Dependencies**: #819 (plugin system). **Effort**: 30–40 pts.

### Slice 3: EQBC Inter-Character Communication (#822)

Sub-issues TBD (recommend 3–4):

- EQBC Protocol Research
- Relay Server Implementation
- DLL-side Client Implementation
- Multi-character Testing & Validation

**Dependencies**: None (parallel with #819/#820). **Effort**: 30–40 pts.

---

## M8: Combat & Coordination Hardening (#1111)

**Goal**: Complete combat automation and raid coordination parity.  
**Depends On**: M7 complete.

### Slice 1: Debuff & Support Systems (#828, #833, #834)

#### #828 — Debuff Tracking

| Issue | Title                                | Tags                               | Points |
| ----- | ------------------------------------ | ---------------------------------- | ------ |
| #1167 | Debuff Detection & Database Research | `debuff` `research` `combat`       | 6–10   |
| #1168 | Debuff Tracking Implementation       | `debuff` `implementation` `combat` | 8–12   |
| #1170 | Dispel & CC Mitigation Engine        | `debuff` `implementation` `combat` | 8–12   |

#### #833 — Resurrection & Recovery

Sub-issues TBD (2–3). Tags: `support` `recovery`.

#### #834 — AA Spend Automation

Sub-issues TBD (2–3). Tags: `progression` `aa`.

**Effort**: 60–85 pts.

### Slice 2: Combat Spell System (#831, #852)

- **#831** Spell Twist & Medley Support — 3–4 sub-issues TBD. Tags: `spell` `casting`.
- **#852** Advanced Spell Database & Optimization — 3–4 sub-issues TBD. Tags: `spell` `optimization`.

**Effort**: 55–75 pts.

### Slice 3: Group & Raid Coordination (#836, #838)

- **#836** Advanced Group Management — 2–3 sub-issues TBD. Tags: `group` `targeting`.
- **#838** Raid & Multi-Group Coordination — 3–4 sub-issues TBD. Tags: `raid` `orchestration`.

**Effort**: 55–70 pts.

### Slice 4: Combat Metrics & Communication (#840, #851)

- **#840** Kill Tracking & DPS Metrics — 2–3 sub-issues TBD. Tags: `metrics` `dps`.
- **#851** Tell Relaying & Chat Forwarding — 2 sub-issues TBD. Tags: `communication` `relay`.

**Effort**: 40–50 pts.

---

## M9: Automation & Learning Systems (#1115)

**Goal**: Event-driven automation and foundational learning infrastructure.  
**Depends On**: M8 stable (can overlap).

### Slice 1: Events & Triggers System (#843)

- **#843** Events & Triggers — 3–4 sub-issues TBD. Tags: `events` `automation`.

**Effort**: 25–35 pts.

### Slice 2: Resource Gathering (#847, #848)

- **#847** Auto-Forage & Resource Gathering — 2–3 sub-issues TBD. Tags: `gathering` `automation`.
- **#848** Collectible & Tribute Management — 2–3 sub-issues TBD. Tags: `progression` `quests`.

**Effort**: 35–45 pts.

### Slice 3: Progression Systems (#854, #855, #856)

- **#854** Skill Leveling & Training — 2–3 sub-issues TBD. Tags: `skills` `progression`.
- **#855** Quest Tracking & Task Automation — 2–3 sub-issues TBD. Tags: `quests` `automation`.
- **#856** XP Tracking & Leveling Analytics — 2 sub-issues TBD. Tags: `leveling` `metrics`.

**Effort**: 60–75 pts.

---

## M10: Economy & Inventory Management (#1121)

**Goal**: Self-sustaining economy and inventory optimization.  
**Depends On**: M8 stable (can overlap).

### Slice 1: Loot & Inventory (#824, #825, #826, #830)

- **#824** Auto-Loot Sorting — 2–3 sub-issues TBD. Tags: `loot` `inventory`.
- **#825** Auto-Banking — 2–3 sub-issues TBD. Tags: `inventory` `economy`.
- **#826** Auto-Accept Dialogs — 2 sub-issues TBD. Tags: `dialog` `automation`.
- **#830** Vendor Management & Auto-Sell — 2–3 sub-issues TBD. Tags: `vendor` `economy`.

**Effort**: 75–95 pts.

### Slice 2: Visibility & Tracking (#842, #849)

- **#842** Spawn Tracking & Management — 2–3 sub-issues TBD. Tags: `spawn` `hunting`.
- **#849** Platinum & Economy Tracking — 2 sub-issues TBD. Tags: `economy` `metrics`.

**Effort**: 45–55 pts.

### Slice 3: UI & Tools (#845, #857, #858, #859)

- **#845** HUD Customization & Windows — 3–4 sub-issues TBD. Tags: `hud` `ui`.
- **#857** Window Management & HUD Integration — 2–3 sub-issues TBD. Tags: `windows` `ui`.
- **#858** Equipment Sets & Focus Effects — 2–3 sub-issues TBD. Tags: `equipment` `focus-effects`.
- **#859** Sound Alerts & Notifications — 1–2 sub-issues TBD. Tags: `audio` `alerts`.

**Effort**: 95–120 pts.

---

## M11: System Stability & Integration (#1123)

**Goal**: Production-grade stability and 3rd-party integration.  
**Depends On**: All other milestones.

### Slice 1: Discord Integration (#839)

- **#839** Discord Webhooks & Chat Integration — 2–3 sub-issues TBD. Tags: `discord` `integration`.

**Effort**: 20–25 pts.

### Slice 2: Crash Reporting (#853)

- **#853** Crash Reporting & Session Recovery — 2–3 sub-issues TBD. Tags: `stability` `crash-reporting`.

**Effort**: 25–30 pts.

---

## Complete Feature Issue Map (31 Features + 1 Research)

| Issue | Title               | Milestone | Sub-Issues | Points | Tags                           |
| ----- | ------------------- | --------- | ---------- | ------ | ------------------------------ |
| #819  | Plugin System       | M7        | 4          | 40–60  | `plugin-system` `architecture` |
| #820  | Lua Scripting       | M7        | 4          | 30–40  | `lua` `scripting`              |
| #822  | EQBC Communication  | M7        | 3–4        | 30–40  | `eqbc` `network`               |
| #828  | Debuff Tracking     | M8        | 3          | 20–30  | `debuff` `combat`              |
| #831  | Spell Twist         | M8        | 3–4        | 25–35  | `spell` `casting`              |
| #833  | Resurrection        | M8        | 2–3        | 20–25  | `support` `recovery`           |
| #834  | AA Spending         | M8        | 2–3        | 15–20  | `progression` `aa`             |
| #836  | Group Management    | M8        | 2–3        | 25–30  | `group` `targeting`            |
| #838  | Raid Coordination   | M8        | 3–4        | 30–40  | `raid` `orchestration`         |
| #840  | Kill Tracking       | M8        | 2–3        | 20–25  | `metrics` `dps`                |
| #851  | Tell Relaying       | M8        | 2          | 20–25  | `communication` `relay`        |
| #852  | Spell Optimization  | M8        | 3–4        | 30–40  | `spell` `optimization`         |
| #824  | Auto-Loot           | M10       | 2–3        | 20–25  | `loot` `inventory`             |
| #825  | Auto-Banking        | M10       | 2–3        | 20–25  | `inventory` `economy`          |
| #826  | Auto-Accept         | M10       | 2          | 15–20  | `dialog` `automation`          |
| #830  | Vendor Management   | M10       | 2–3        | 20–25  | `vendor` `economy`             |
| #842  | Spawn Tracking      | M10       | 2–3        | 25–30  | `spawn` `hunting`              |
| #845  | HUD Customization   | M10       | 3–4        | 30–40  | `hud` `ui`                     |
| #849  | Plat Tracking       | M10       | 2          | 20–25  | `economy` `metrics`            |
| #857  | Window Management   | M10       | 2–3        | 20–25  | `windows` `ui`                 |
| #858  | Equipment Sets      | M10       | 2–3        | 15–20  | `equipment` `focus-effects`    |
| #859  | Sound Alerts        | M10       | 1–2        | 10–15  | `audio` `alerts`               |
| #843  | Events & Triggers   | M9        | 3–4        | 25–35  | `events` `automation`          |
| #847  | Auto-Forage         | M9        | 2–3        | 15–20  | `gathering` `automation`       |
| #848  | Collectibles        | M9        | 2–3        | 20–25  | `progression` `quests`         |
| #854  | Skill Training      | M9        | 2–3        | 25–30  | `skills` `progression`         |
| #855  | Quest Tracking      | M9        | 2–3        | 20–25  | `quests` `automation`          |
| #856  | XP Tracking         | M9        | 2          | 15–20  | `leveling` `metrics`           |
| #839  | Discord Integration | M11       | 2–3        | 20–25  | `discord` `integration`        |
| #853  | Crash Reporting     | M11       | 2–3        | 25–30  | `stability` `crash-reporting`  |
| #860  | Parity Matrix Doc   | Research  | 0          | —      | `documentation` `research`     |

---

## Dependency Chain & Critical Path

**Foundational (no dependencies — start first):**

- M7: #819, #820, #822

**Secondary (depend on M7):**

- M8 #828–#852 — Combat depends on plugin system for class strategies
- M9 #843–#856 — Events depend on plugin event system
- M10 #824–#859 — Inventory depends on plugin system

**Tertiary (depend on M8 stable):**

- M11 #839, #853 — Stability depends on M8 being stable

**Critical path**: #819 → #1131 → #1135 → #1139 → #1144 → M8 combat slices → M11 stability

---

## Tagging Strategy

| Category  | Tags                                                                                                                                                 |
| --------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Domain    | `plugin-system` `lua` `eqbc` `combat` `spell` `group` `raid` `loot` `inventory` `economy` `metrics` `events` `hud` `audio` `integration` `stability` |
| Type      | `research` `implementation` `validation` `sub-issue-of-{ISSUE}`                                                                                      |
| Status    | `open` `in-progress` `blocked` `review` `done`                                                                                                       |
| Milestone | `milestone-m7` `milestone-m8` `milestone-m9` `milestone-m10` `milestone-m11`                                                                         |
| Priority  | `critical-path` `high-impact` `nice-to-have`                                                                                                         |

---

## Execution Roadmap

### Phase 1 — Foundation (M7, Weeks 1–12)

- Establish plugin system as primary extensibility mechanism (#819)
- Get Lua interpreter working and core APIs bound (#820)
- Implement EQBC relay for multi-box communication (#822)
- All 3 slices can work in parallel after initial design

### Phase 2 — Combat (M8, Weeks 13–32)

- Implement debuff tracking, CC detection, dispel automation (#828)
- Complete spell twist/medley support (#831)
- Build raid coordination on top of M7 foundation (#838)
- 4 slices can run in parallel

### Phase 3 — Automation + Economy (M9 + M10, Weeks 33–56)

- Add event triggers and automation hooks (#843)
- Implement progression systems (#854, #855, #856)
- Build economy and inventory management (#824–#830, #842–#859)
- M9 and M10 run in parallel

### Phase 4 — Stability (M11, Weeks 57–62)

- Discord integration and crash reporting (#839, #853)
- Production hardening, monitoring, community testing

---

## Success Metrics

- [ ] All 31 feature issues have 3–4 sub-issues each
- [ ] All sub-issues have clear acceptance criteria
- [ ] Milestone dependencies documented and respected
- [ ] Every issue tagged with: type, domain, milestone, effort estimate
- [ ] Critical path identified and tracked
- [ ] Community can build plugins using #819 APIs
- [ ] Community can write Lua scripts using #820 APIs
- [ ] 36-box setups coordinated via #822 EQBC
- [ ] OpenVanilla feature parity achieved by end of M10

---

## Related Documents

- [`openvanilla-macroquest-coverage-gap-plan.md`](openvanilla-macroquest-coverage-gap-plan.md) — MacroQuest/OpenVanilla coverage gap audit
- [`openvanilla-redguides-2026-04-25-audit.md`](openvanilla-redguides-2026-04-25-audit.md) — RedGuides audit (2026-04-25)
- [`FEATURE_PARITY_MATRIX.md`](FEATURE_PARITY_MATRIX.md) — Full feature parity matrix
- [`MILESTONE_PLAN.md`](MILESTONE_PLAN.md) — Milestone planning details
- [`implementation-roadmap.md`](implementation-roadmap.md) — Implementation roadmap
