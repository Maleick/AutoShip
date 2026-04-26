# Guide: Tagging, Organization & GitHub Project Setup

## Overview

This document defines the complete tagging scheme and GitHub project structure for organizing 35+ issues (31 features + 4 milestones) across 4 major milestones and 12 slices. The tagging system ensures:

- **Visibility**: Every issue has clear milestone, domain, type, priority, and effort labels
- **Dependency tracking**: Blocking relationships are explicitly captured
- **Automation**: GitHub Projects automatically move issues through workflow columns
- **Queryability**: Filters and searches work consistently across the codebase

---

## Label Categories (Create these labels in GitHub)

### Milestone Labels (Required)

- `milestone-m7` - Plugin Ecosystem & Core Extensibility
- `milestone-m8` - Combat & Coordination Hardening
- `milestone-m9` - Automation & Learning Systems
- `milestone-m10` - Economy & Inventory Management
- `milestone-m11` - System Stability & Integration

### Domain Labels (Issue categorization)

- `plugin-system` - Plugin architecture, loading, APIs
- `lua` - Lua scripting, interpreter, bindings
- `eqbc` - EQBC protocol, relay server, multi-char communication
- `combat` - Combat automation, spell casting, aggro
- `spell-casting` - Spell execution, GCD, interrupts, mana
- `spell-optimization` - Focus effects, mana efficiency, rotations
- `debuff` - Crowd control, debuff tracking, dispel
- `support` - Healing, buffs, resurrection, recovery
- `group` - Group management, targeting, role detection
- `raid` - Raid coordination, multi-group, formation
- `loot` - Item pickup, filtering, distribution, corpse handling
- `inventory` - Item management, stacking, space optimization
- `economy` - Platinum, vendor, profit tracking, banking
- `progression` - AA spend, skill training, quest tracking, XP
- `events` - Event system, triggers, callbacks, automation
- `metrics` - DPS, kill tracking, performance analytics
- `ui` - TUI panels, displays, configuration
- `hud` - HUD overlays, windows, customization, theming
- `audio` - Sound effects, alerts, notifications
- `communication` - Chat, tells, Discord, broadcasting
- `integration` - External tools, APIs, third-party support
- `stability` - Crash reporting, recovery, reliability

### Issue Type Labels

- `research` - Investigation, design, analysis work
- `implementation` - Code work to build feature
- `validation` - Testing, proof-of-concept, live verification
- `documentation` - Guides, API docs, tutorials
- `bug-fix` - Defect correction
- `sub-issue` - Smaller decomposed work unit

### Status Labels

- `status-open` - Issue not started
- `status-in-progress` - Currently being worked
- `status-blocked` - Waiting for blocker to clear
- `status-review` - Ready for review
- `status-done` - Completed and merged
- `status-on-hold` - Intentionally paused

### Relationship Labels

- `blocking-{ISSUE}` - This issue blocks issue #N
- `blocked-by-{ISSUE}` - This issue blocked by issue #N
- `depends-on-{ISSUE}` - This issue depends on issue #N
- `parent-issue-{ISSUE}` - This is sub-issue of issue #N

### Priority Labels

- `priority-critical-path` - On critical path for milestone
- `priority-high` - High impact, should do early
- `priority-medium` - Standard priority
- `priority-nice-to-have` - Optimization or nice-to-have

### Effort Estimate Labels (Story Points)

- `effort-5` - Very small (1 person-day)
- `effort-10` - Small (2-3 person-days)
- `effort-20` - Medium (1 week)
- `effort-30` - Large (1.5 weeks)
- `effort-40` - Very large (2 weeks)
- `effort-50+` - Epic scope (2+ weeks)

---

## Label Application Examples

### Example 1: #819 Plugin System

- `milestone-m7`
- `plugin-system`
- `implementation` (issue itself is implementation-focused)
- `priority-critical-path` (gates all other work)
- `effort-40` (40-60 points estimated)
- `status-open`
- **Sub-issues:**
  - #1131: `plugin-system` `research` `sub-issue` `effort-10`
  - #1135: `plugin-system` `implementation` `sub-issue` `effort-20` `blocking-1139`
  - #1139: `plugin-system` `implementation` `sub-issue` `effort-15` `blocked-by-1135`
  - #1144: `plugin-system` `validation` `sub-issue` `effort-15` `documentation`

### Example 2: #828 Debuff Tracking

- `milestone-m8`
- `debuff` `combat`
- `implementation`
- `priority-high` (combat feature)
- `effort-20` (20-30 points estimated)
- `status-open`
- **Sub-issues:**
  - #1167: `debuff` `research` `sub-issue` `effort-10`
  - #1168: `debuff` `implementation` `sub-issue` `effort-10` `blocked-by-1167`
  - #1170: `debuff` `implementation` `sub-issue` `effort-10` `depends-on-1168`

---

## GitHub Project Structure

Create **4 GitHub Projects** (one per milestone):

### Project M7: Plugin Ecosystem (Kanban board)

**Columns**: Backlog | Research | Implementation | Testing | Review | Done

**Issues in project:**

- #1102 (Milestone M7 overview)
- #819 (Plugin System) + 4 sub-issues
- #820 (Lua Scripting) + 4 sub-issues
- #822 (EQBC) + 3-4 sub-issues

**Automation:**

- Auto-move to "Done" when marked `status-done`
- Auto-move to "Review" when PR created
- Auto-move to "In Progress" when labeled `status-in-progress`

### Project M8: Combat & Coordination

**Columns**: Backlog | Research | Implementation | Live Testing | Review | Done

**Issues in project:**

- #1111 (Milestone M8 overview)
- #828-#852 (9 features) + 20+ sub-issues
- Filter by: `milestone-m8` + domain labels

### Project M9: Automation & Learning

**Columns**: Backlog | Implementation | Testing | Review | Done

**Issues in project:**

- #1115 (Milestone M9 overview)
- #843, #847, #848, #854, #855, #856 (6 features) + 15+ sub-issues

### Project M10: Economy & Inventory

**Columns**: Backlog | Implementation | Testing | Review | Done

**Issues in project:**

- #1121 (Milestone M10 overview)
- #824-#859 (11 features) + 25+ sub-issues

---

## Tagging Template (Copy-Paste for New Issues)

### For Feature Issues (35 total)

```
Labels: [milestone-mX] [domain1] [domain2] [type] [priority-X] [effort-X] [status-open]
Blocking: Specify if issue blocks others
Blocked By: Specify dependencies
```

### For Sub-Issues (80+ total)

```
Labels: [milestone-mX] [domain] [type] [sub-issue] [effort-X] [status-open]
Parent: Link to parent issue (title should include reference)
Blocking: Sub-issues may block siblings or parent
Blocked By: Sub-issues may block siblings
```

### For Milestone Issues (4 total)

```
Labels: [milestone-mX] [documentation] [status-open]
Issues: Link to all related feature issues
Timeline: Estimated start/end dates
```

---

## Dependency Declaration Strategy

### In Issue Description:

```markdown
## Dependencies

- **Blocks**: #XYZ, #ABC
- **Blocked By**: #DEF
- **Depends On**: #GHI

## Critical Path

This issue is on critical path for M8. Must complete before:

- #852 can start
- #838 can start
```

### In Commit Messages:

```
Fix #828: Debuff tracking implementation

This implements the core debuff detection system that unblocks #1170
and enables #834 AA spend automation logic.

Depends on: #1167 (debuff database)
Blocks: #1170 (dispel recommendations)
```

### In PR Description:

```markdown
Closes #1135 (Plugin Loader Implementation)

Related issues:

- Unblocks: #1139 (Core Plugin API Surface)
- Depends on: #1131 (Plugin Architecture Design)
- Resolves sub-issue of: #819 (Plugin System)

## Testing

- [ ] Plugin loads successfully
- [ ] Plugin lifecycle hooks called
- [ ] No crashes on plugin errors
```

---

## Bulk Tagging Workflow

### Initial Setup (One-time)

1. Create all 4 milestone labels
2. Create all 20 domain labels
3. Create all issue type labels
4. Create all relationship labels
5. Create effort estimate labels

### For Each Feature Issue

1. Apply `milestone-m{X}` label
2. Apply 1-3 domain labels
3. Apply `issue-type` (research/implementation/validation)
4. Apply `priority-{X}` based on roadmap
5. Apply `effort-{X}` from estimation
6. Apply `status-open`
7. Add blocking/blocked-by labels
8. Create 3-4 sub-issues with parent linking

### For Each Sub-Issue

1. Apply same milestone as parent
2. Apply same domain as parent
3. Apply `sub-issue` type
4. Apply `effort-{X}` (smaller than parent)
5. Title should include parent reference: "Sub: {description} (#{PARENT})"
6. Link parent in description using "parent-issue-{N}"
7. Add blocking relationships to sibling sub-issues if sequential

---

## Query Examples (Using GitHub Filter)

### Find all M7 work

```
label:milestone-m7
```

### Find blocking issues on critical path

```
label:milestone-m8 label:priority-critical-path
```

### Find implementation work in M8

```
label:milestone-m8 label:implementation
```

### Find all research issues

```
label:research state:open
```

### Find issues blocking a feature

```
blocking-#828
```

### Find sub-issues ready to start

```
label:sub-issue label:status-open -label:blocked-by-*
```

### Find effort > 30 points

```
label:effort-40 OR label:effort-50+
```

---

## Tagging Maintenance

### Weekly Review

- [ ] Move issues to appropriate status
- [ ] Check for cycle dependencies (A blocks B, B blocks A)
- [ ] Verify milestone assignments still correct
- [ ] Update effort estimates if work discovered

### Per-Release

- [ ] Ensure all merged issues labeled `status-done`
- [ ] Archive completed milestones
- [ ] Update dependency labels for next milestone

---

## Success Criteria for Tagging

- [ ] Every issue has exactly 1 `milestone-mX` label
- [ ] Every issue has 1-3 domain labels
- [ ] Every issue has exactly 1 type label
- [ ] Every issue has exactly 1 `priority-X` label
- [ ] Every issue has exactly 1 `effort-X` label
- [ ] Every issue has exactly 1 `status-` label
- [ ] Parent/sub-issue relationships documented
- [ ] Critical path issues clearly marked
- [ ] Blocking relationships captured in labels
- [ ] All queries return expected results

---

## GitHub Project Automation Rules

Create automation rules for each project:

```
When: Issue labeled with 'status-done'
Then: Move to 'Done' column

When: Issue labeled with 'status-in-progress'
Then: Move to 'Implementation' or 'Testing' column (depending on type)

When: Issue labeled with 'status-blocked'
Then: Move to separate 'Blocked' column

When: Pull request references issue (#N)
Then: Move issue to 'Review' column

When: Pull request merged and closes issue
Then: Label issue with 'status-done'
```

---

## Tips & Best Practices

### Tip 1: Sub-Issue Decomposition

Break large features (effort > 20) into 3-4 sub-issues:

- One **research** (design/architecture) sub-issue
- 1-2 **implementation** sub-issues (sequential)
- One **validation** sub-issue (testing/verification)

Example breakdown for #828 (Debuff Tracking):

- Research: Database schema design (effort-10)
- Implementation: Core tracking (effort-10)
- Implementation: Dispel logic (effort-10)
- Total parent: effort-30 ≈ 1 week

### Tip 2: Blocking Relationships

Use `blocking-N` and `blocked-by-N` for **sequential work only**:

✅ Use when: Task A must finish before Task B starts
❌ Don't use when: Just related by topic

Example (good):

```
#1135 (Plugin Loader) blocks #1139 (Plugin API)
  → Loader must be implemented before API can be written
```

### Tip 3: Critical Path Tracking

Every milestone should have 2-4 `priority-critical-path` issues that:

- Gate other work
- Appear in multiple dependency chains
- Must complete on schedule to hit milestone dates

### Tip 4: Effort Estimation

Use Fibonacci-like scale (5, 10, 20, 30, 40, 50+):

- **effort-5**: One function, no dependencies
- **effort-10**: Single module, known architecture
- **effort-20**: Full subsystem, some unknowns
- **effort-30**: Major feature, cross-system impact
- **effort-40+**: Epic scope, needs decomposition

### Tip 5: Domain Label Combinations

**Combat feature** → `combat` + `spell-casting` or `debuff` (specific)
**Raid feature** → `raid` + related domain (e.g., `raid` + `group`)
**Economy feature** → `economy` + `inventory` (if inventory-heavy)

---

## Troubleshooting

### Q: What if an issue needs 2 milestone labels?

A: It should not. Split it into separate issues. Milestones define release cycles; an issue should belong to one cycle only.

### Q: Can a sub-issue have its own sub-issues?

A: No. Sub-issues should be atomic. If a sub-issue needs breakdown, it's too large—reconsider parent decomposition.

### Q: How do I handle refactoring/tech-debt?

A: Use `type:bug-fix` or `type:implementation` with `priority-nice-to-have`. Document in issue description why it's necessary.

### Q: What if blocking relationship is discovered mid-sprint?

A: Add the label immediately and update GitHub Project. Post comment explaining blocker. Move issue to Blocked column if necessary.

### Q: How do I track research outcomes?

A: Create research issue first (e.g., #1131), then reference findings in dependent implementation issues. Link via `depends-on` labels.

---

## Table: Label Combinations Quick Reference

| Scenario                     | Milestone          | Domain            | Type               | Priority                 | Effort      | Status        |
| ---------------------------- | ------------------ | ----------------- | ------------------ | ------------------------ | ----------- | ------------- |
| New combat feature           | `milestone-m8`     | `combat` + domain | `implementation`   | varies                   | varies      | `status-open` |
| Plugin architecture research | `milestone-m7`     | `plugin-system`   | `research`         | `priority-critical-path` | `effort-10` | `status-open` |
| Economy sub-task             | `milestone-m10`    | `economy`         | `sub-issue` + type | `priority-high`          | `effort-10` | `status-open` |
| UI/UX documentation          | any                | `ui` or `hud`     | `documentation`    | `priority-medium`        | `effort-5`  | `status-open` |
| Bug fix (unplanned)          | milestone of issue | relevant          | `bug-fix`          | `priority-high`          | `effort-5`  | `status-open` |
| Raid testing/validation      | `milestone-m8`     | `raid`            | `validation`       | `priority-high`          | `effort-20` | `status-open` |

---

## GitHub Actions Integration (Optional)

Consider automating label enforcement with GitHub Actions:

1. **Validate label presence**: CI check that verifies issues have required labels before closing
2. **Auto-label PRs**: Automatically apply milestone/domain labels to PRs based on branch name
3. **Generate burndown reports**: Weekly reports of progress by effort/priority
4. **Blocked issue notifications**: Automated comments when `status-blocked` is applied

---

## Document Maintenance

This document should be reviewed and updated:

- **Monthly:** During milestone planning to ensure label accuracy
- **Per-release:** After each milestone to capture lessons learned
- **Ad-hoc:** When adding new label categories or automation rules

**Last Updated:** 2026-04-25

---

## Tags

`documentation` `tagging-strategy` `organization` `github` `methodology`
