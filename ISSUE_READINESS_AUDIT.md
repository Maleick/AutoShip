# Issue Readiness Audit & Classification Framework

## Part 1: Audit of Documentation Issues

### #1298 - AUTO-FIX: All Config Files & Workflows Ready to Copy-Paste

**Status:** ✅ **DISPATCHABLE** (with minor caveats)

**Completeness Check:**
- [x] Clear scope: Provide ready-to-copy config files and GitHub Actions workflows
- [x] Acceptance criteria explicit: "Create X files, enable Y settings, that's it"
- [x] No external dependencies: All files are self-contained
- [x] Ready-made code blocks: Copy-paste workflows provided inline
- [x] Action items enumerated: 9 ACTIONs with specific file paths

**Gaps Found:**
1. **ACTION 1 is manual** - Requires UI navigation to Settings. Should note this is one-time manual step
2. **Workflow files have placeholder text** - e.g., `github.head_ref` and `GITHUB_HEAD_REF` inconsistencies, some workflows may need tweaking for actual repo
3. **Missing: Pre-commit hook installation verification** - ACTION 8 script doesn't verify hooks were installed correctly
4. **Missing: Troubleshooting section** - What if workflows fail? What if settings are already partially configured?

**Why it's still DISPATCHABLE:**
- Scope is narrow: "Create these files, enable this setting"
- Even with gaps, an agent can implement the core deliverable
- Caveats are "nice to have", not blockers

**Recommendation:** Keep as-is, but add a "Known Issues" section noting:
- Manual repo settings step required first
- Workflows may need minor tweaks per actual branch/label naming
- Pre-commit hooks optional but recommended

---

### #1184 - Standard: Testing, Quality, Polish & Unit Tests

**Status:** ✅ **DISPATCHABLE** (comprehensive but meta)

**Completeness Check:**
- [x] Detailed testing standards by issue type (research, implementation, validation, docs)
- [x] Unit test template with concrete examples
- [x] Code quality standards (formatting, documentation, error handling)
- [x] Test coverage requirements per milestone
- [x] CI/CD workflow example provided
- [x] Developer onboarding workflow explained

**Gaps Found:**
1. **Gap: Doesn't define "size-s" or "size-xs" issues** - References 80%+ coverage but doesn't explain how to size an issue for someone to grab
2. **Gap: No guidance on "when to write tests first vs implement first"** - Test-driven development implied but not explicit
3. **Gap: Criterion benchmarking setup not detailed** - References `criterion` crate but doesn't explain setup/CI integration
4. **Gap: "Quality Gates by Milestone" section assumes M7-M11 exist** - References milestones that may not be active yet
5. **Gap: Thread safety testing** - Mentions "if concurrent" but no guidance on identifying concurrency needs

**Why it's DISPATCHABLE:**
- It's a reference document, not an action item
- Teams can follow its guidance even with the gaps
- Gaps are refinements, not blockers

**Recommendation:** Add sections:
- Quick reference: "Are you writing a test-first or feature-first issue?"
- Concurrency checklist: "Does your feature touch shared state?"
- Benchmark CI setup: How to integrate criterion benchmarks in workflows

---

### #1180 - Guide: Tagging, Organization & GitHub Project Setup

**Status:** ⚠️ **PARTIALLY DISPATCHABLE** (gaps in implementation)

**Completeness Check:**
- [x] Complete label taxonomy defined (20+ domain labels, type labels, status labels)
- [x] Application examples for 2 features
- [x] GitHub Projects structure outlined (4 projects, column definitions)
- [x] Query examples provided
- [x] Automation rules described

**Gaps Found:**
1. **Gap: Labels don't exist yet** - Document says "Create these labels" but doesn't provide a script or CLI one-liner to create them. Manual UI work required.
2. **Gap: No enforcement mechanism** - What happens if someone files an issue without a size label? No rule to prevent it.
3. **Gap: Sub-issue linking not detailed** - Says "link parent in description" but GitHub has native sub-issue feature that would be clearer
4. **Gap: Project automation incomplete** - Rules are described but no actual GitHub Project automation configuration provided
5. **Gap: No bulk-tagging workflow** - Says "apply label" 7 times per issue, but no tool/script to do this at scale (35+ issues to tag)
6. **Gap: Effort estimate labels underspecified** - `effort-5` = "1 person-day" but what about effort-10, 20, 30? Linear or exponential?

**Why it's only PARTIALLY dispatchable:**
- Can't implement without creating labels first (manual step)
- Can't fully automate without tooling
- Workflow is documented but not automated

**Recommendation:** Add or create:
1. **gh CLI script** to create all labels at once
2. **GitHub Actions workflow** to auto-tag issues missing size labels (warn, don't enforce yet)
3. **Bulk tagging checklist** - How to tag 35 issues efficiently
4. **Sub-issue linking guide** - Use GitHub's native feature vs inline references
5. **Effort scale justification** - Linear: 5→10→20→30→40→50+, or log scale?

---

### #1177 - Index: OpenVanilla Parity - Complete Issue Organization & Roadmap

**Status:** ✅ **DISPATCHABLE** (reference document)

**Completeness Check:**
- [x] All 31 features indexed and organized by milestone
- [x] 4 milestones defined with goals and durations
- [x] Slice structure clear (M7 has 3 slices, M8 has 4, etc.)
- [x] Complete issue mapping table (all 31 features + 4 milestones)
- [x] Dependency chain documented
- [x] Tagging strategy cross-referenced

**Gaps Found:**
1. **Gap: Sub-issue count inconsistent** - Says "3 sub-issues" for #828 but then describes 5-6 actual work items. Should clarify: are these estimates or fixed?
2. **Gap: Duration estimates lack confidence** - "8-12 weeks" for M7, but no rationale or risk factors listed
3. **Gap: No critical path algorithm** - Says "critical path identified" but doesn't explain which features block which (beyond high-level dependencies)
4. **Gap: "Questions & Next Steps" are open-ended** - Should answer them or file issues for each
5. **Gap: No cross-reference to actual GitHub issues** - Links to #1102, #1111, etc. but some don't exist yet (not verified)

**Why it's still DISPATCHABLE:**
- It's a planning document, not an implementation task
- Gaps are planning details, not scope creep
- Can be used as-is to guide milestone work

**Recommendation:** Clarify:
1. Sub-issue counts: Are these minimum/target/maximum?
2. Duration basis: Story points? Team velocity? Historical data?
3. Risk factors: What could delay M7? M8?
4. Link verification: Ensure all referenced issues exist
5. Answer the open questions or file issues for each

---

## Part 2: Proposed Labeling Scheme

### Current Labels (from #1180 and #1177)
You already have a label system! But it's incomplete. Here's what's missing:

### **MISSING: Size/Effort Labels**
These are critical for "dispatchability":

```
size-xs      = 1-4 hours (can finish in a morning)
size-s       = 4-16 hours (fits in 1-2 days)
size-m       = 16-40 hours (1 week sprint)
size-l       = 40-80 hours (2-3 week sprint)
size-xl      = 80+ hours (epic, needs decomposition)
```

**Why this matters:** An agent or developer looking at 100 open issues needs to know "which ones can I finish this afternoon?" vs "which ones need a team?"

**How to apply:**
- Research issues: typically `size-s` to `size-m`
- Documentation issues: typically `size-s` to `size-m`
- Implementation issues: typically `size-m` to `size-xl`
- Validation issues: typically `size-m` to `size-l`

---

### **MISSING: Blocker Status Labels**
Current status labels (`status-open`, `status-in-progress`, etc.) don't explain *why* something is stuck:

```
blocker-parent-issue     = Waiting on parent epic decomposition
blocker-external-dep     = Waiting on external code/tool (e.g., #1773 blocking #1569)
blocker-data-dep         = Waiting on data/research (e.g., #1577 waiting on zone data)
blocker-human-required   = Requires human action (e.g., EQ Test access)
blocker-waiting-approval  = Ready but waiting for design/architecture approval
blocker-tech-debt       = Blocked on cleanup/refactor of related code
```

**Why this matters:** "Blocked" is useless without context. Are you blocked on code? Data? Decisions? Knowing which helps you:
- Find workarounds (design-approved work while waiting for architecture)
- Know when something can ship (vs waiting on external changes)
- Identify critical path bottlenecks

**How to apply:**
- #1569 (SDK): Label `blocker-external-dep` (waiting on #1773)
- #1568, #1559, #1557 (Epics): Label `blocker-parent-issue` (need decomposition)
- #1270, #1526 (Live testing): Label `blocker-human-required`

---

### **MISSING: Actionability Label**
To clearly mark dispatchable work:

```
ready-to-dispatch = No blockers, clear scope, can start immediately
ready-for-review  = Implementation done, waiting for code review
needs-refinement  = Scope unclear, needs design discussion first
needs-decomposition = Too large, needs to be broken into sub-issues
```

**How to apply:**
- #1298, #1184, #1180, #1177: `ready-to-dispatch`
- #1569 (SDK): `needs-decomposition` + `blocker-external-dep`
- #1568, #1559, #1557 (Epics): `needs-decomposition`

---

### **COMPLETE LABELING FRAMEWORK**

Here's how to organize your label system:

```
Category: Milestone
├─ milestone-m7
├─ milestone-m8
├─ milestone-m9
├─ milestone-m10
├─ milestone-m11

Category: Domain (20 labels from #1180)
├─ plugin-system
├─ lua
├─ eqbc
├─ combat
├─ [... 15 more from #1180]

Category: Type
├─ research
├─ implementation
├─ validation
├─ documentation
├─ bug-fix
├─ sub-issue

Category: Size (NEW)
├─ size-xs (1-4h)
├─ size-s (4-16h)
├─ size-m (16-40h)
├─ size-l (40-80h)
├─ size-xl (80h+)

Category: Status
├─ status-open
├─ status-in-progress
├─ status-blocked
├─ status-review
├─ status-done

Category: Blocker (NEW)
├─ blocker-parent-issue
├─ blocker-external-dep
├─ blocker-data-dep
├─ blocker-human-required
├─ blocker-waiting-approval
├─ blocker-tech-debt

Category: Actionability (NEW)
├─ ready-to-dispatch
├─ ready-for-review
├─ needs-refinement
├─ needs-decomposition

Category: Priority (from #1180)
├─ priority-critical-path
├─ priority-high
├─ priority-medium
├─ priority-nice-to-have

Category: Relationship (from #1180)
├─ blocking-{ISSUE}
├─ blocked-by-{ISSUE}
├─ depends-on-{ISSUE}
├─ parent-issue-{ISSUE}
```

---

## Part 3: Template for Epic Dispatchability

An **epic is dispatchable** when it can be assigned to a team/agent. Most of yours aren't yet. Here's the template:

### **Epic Readiness Checklist**

```markdown
## Dispatchability Checklist

- [ ] **Acceptance Criteria**: Clear definition of "done"
  - Example: "All 3 slices have child issues filed and at least 1 is in progress"
  - Example: "All linked child issues are labeled with size, blocker status, and priority"

- [ ] **Decomposition**: Broken into 2-4 week slices or child issues
  - Example: Don't say "Plugin System" without listing #1131, #1135, #1139, #1144
  - Each child should have 1-3 person-weeks of work
  - Must have at least 3 children (if not, it's not an epic, it's a feature)

- [ ] **Dependencies Explicit**: Knows what blocks it, what it blocks
  - Blocker section: "This epic cannot start until X is done"
  - Unblocks section: "This epic unblocks X, Y, Z"
  - Example: "M8 epic cannot start until M7 plugin system is stable"

- [ ] **Success Metrics**: How you'll know when it's complete
  - Metrics should be measurable, not fuzzy
  - Example: ✅ "All 12 sub-issues labeled status-done"
  - Example: ❌ "Plugin system feels good"

- [ ] **Critical Path**: Identifies which child issues are on critical path
  - Example: "#1135 (Plugin Loader) blocks #1139 (API Surface) blocks everything else"
  - Helps teams know what to prioritize

- [ ] **Release Criteria**: When is it okay to ship this epic?
  - Example: "All combat features tested in live raid, zero crashes for 8 hours"
  - Links to #1184 quality gates

- [ ] **Owner/Slack Handle**: Who is responsible for this epic?
  - Single DRI (Directly Responsible Individual)
  - Makes sure status is updated weekly
```

### **Example: How #1568 (Zone Implementation) Should Look**

**CURRENT (Not Dispatchable):**
```
Epic: Zone Implementation Tracking (10 Priority Zones)
Description: Coordinate zone implementation tracking for 10 zones under M7/M10
Acceptance Criteria: "Leaf issues are linked for coordinates, navigation/config, and farming validation"
Dependencies: None listed
Child Issues: None filed yet
```

**FIXED (Dispatchable):**
```
Epic: Zone Implementation Tracking (10 Priority Zones)
Description: Coordinate zone implementation tracking for 10 zones under M7/M10

Size: size-xl (80+ hours, 8-12 weeks)
Status: needs-decomposition
Blocker: blocker-parent-issue (no child issues filed yet)
Owner: @Maleick

## Decomposition
10 zones × 3 slices each = 30 child issues:
- Slice 1: Offset Discovery & Zone Map (research)
  ├─ #XXXX Howling Stones: Offset & Map Discovery (size-m)
  ├─ #XXXX Guk: Offset & Map Discovery (size-m)
  └─ ... [8 more zones]
- Slice 2: Navigation & Camp Config (implementation)
  ├─ #XXXX Howling Stones: Camp Config & Routing (size-l)
  └─ ... [9 more]
- Slice 3: Farming Validation (validation)
  ├─ #XXXX Howling Stones: DPS & Loot Validation (size-m)
  └─ ... [9 more]

## Dependencies
Blockers: None - can start immediately
Unblocks: #1559 (Camp Configuration), #1526 (Farming validation tracking)

## Critical Path
1. Offset discovery (Slice 1) - Required before navigation can be designed
2. Camp config (Slice 2) - Required before farming validation
3. Farming validation (Slice 3) - Last, but gates release

## Success Metrics
- [ ] All 30 child issues created and linked
- [ ] All child issues have: size, blocker status, priority, acceptance criteria
- [ ] At least 1 zone (Howling Stones) fully implemented and tested
- [ ] Coordination infrastructure (docs, templates) created

## Release Criteria
- [ ] All 10 zones have offset, map, camp config, and farming validation
- [ ] DPS/loot data collected for each zone
- [ ] Wiki documentation updated
- [ ] Zero crashes during 8-hour farming run per zone

## Owner
@Maleick (responsible for filing child issues, tracking status, resolving blockers)
```

---

## Part 4: Recommended Actions

### Immediate (This Week)
1. **Add 3 new label categories** to GitHub:
   - Size labels (size-xs, size-s, size-m, size-l, size-xl)
   - Blocker labels (blocker-parent-issue, blocker-external-dep, blocker-data-dep, blocker-human-required, blocker-waiting-approval, blocker-tech-debt)
   - Actionability labels (ready-to-dispatch, ready-for-review, needs-refinement, needs-decomposition)

2. **Audit the 4 documentation issues** (#1298, #1184, #1180, #1177):
   - Add "Known Gaps" sections
   - Cross-reference each other
   - Ensure acceptance criteria are clear

3. **Decompose 3 epics** (#1568, #1559, #1557):
   - File 3-4 child issues per epic
   - Label with: size, blocker status, priority, actionability
   - Link as native GitHub sub-issues (not just in body text)

### Short Term (Next 2 weeks)
4. **Tag all 46 existing issues** with the new labels
   - Apply size estimates
   - Mark blocker status
   - Mark actionability (ready-to-dispatch vs needs-X)

5. **Create a "Dispatchable Issues" view** in GitHub
   - Filter: `label:ready-to-dispatch AND -label:status-done`
   - This shows what an agent can grab immediately

6. **Document blockers with owners**
   - For each blocker issue, note: "This unblocks X, Y, Z - ETA 2 weeks"

---

## Summary: What Makes Something Dispatchable

An issue is **ready-to-dispatch** when:

| Criteria | What it means |
|----------|--------------|
| **Clear scope** | Acceptance criteria fit in 1-2 sentences |
| **Sized** | Labeled with size-xs through size-l (not size-xl) |
| **No blockers** | No `blocker-*` labels, or blockers have clear ETAs |
| **No design needed** | Doesn't say "figure out the best approach" |
| **Has child issues or none needed** | If it's an epic, children are filed and sized |
| **Owner identified** | Someone is responsible for it |
| **Ready to implement, not research** | Acceptance criteria describe implementation, not investigation |

Your **dispatchable issues right now**: #1298, #1184, #1180, #1177 (with caveats noted in audit above)

Your **issues that need work before dispatch**: #1568, #1559, #1557, #1569, #1566 (need decomposition + labeling)
