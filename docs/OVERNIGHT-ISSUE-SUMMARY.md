# Overnight Testing Infrastructure - Complete Issue Breakdown

**Summary**: 70+ issues organized into 4 core components + 4 cross-cutting concerns, with detailed slices, dependencies, and effort estimates.

**Status**: All issues created and tagged for implementation.

---

## Core Components (70 Issues)

### 1. #861: Logout/Disconnect Mechanism (6 sub-issues)
**Status**: Ready for implementation  
**Dependencies**: None  
**Blocks**: #862, #863, #864  
**Effort**: 5-7 days  
**Labels**: `M7`, `M7.logout`, `phase-1`, `testing`

| Issue | Type | Effort | Status |
|-------|------|--------|--------|
| #1019 | #861.1 — LoginPhase enum | low | Ready |
| #1022 | #861.2 — LogoutStateMachine | low | Ready |
| #1025 | #861.3 — LogoutSequencer | low | Ready |
| #1028 | #861.4 — QuitGame IPC command | low | Ready |
| #1031 | #861.5 — Integration test | med | Ready |
| #1032 | #861.6 — CLI manual logout | low | Nice-to-have |

**Critical Path**: #1019 → #1022 → #1025 → #1028 → #1031

---

### 2. #862: Test Scenario Framework & Harness (9 sub-issues)
**Status**: Ready for implementation  
**Dependencies**: #861 (logout)  
**Blocks**: #863, #864  
**Effort**: 10-14 days  
**Labels**: `M7`, `M7.harness`, `phase-1`, `phase-2`, `testing`

| Issue | Type | Effort | Status |
|-------|------|--------|--------|
| #1034 | #862.1 — TestScenario trait | low | Ready |
| #1036 | #862.2 — CampLoopScenario | med | Ready |
| #1039 | #862.3 — NavigationScenario | med | Ready |
| #1042 | #862.4 — CombatRotationScenario | med | Ready |
| #1046 | #862.5 — Mock scenarios | low | Ready |
| #1047 | #862.6 — TestLoopRunner | med | Ready |
| #1050 | #862.7 — Metrics framework | low | Ready |
| #1052 | #862.8 — Unit tests | low | Ready |
| #1055 | #862.9 — Integration test | med | Ready |

**Critical Path**: #1034 → #1046 → #1047 → #1055

---

### 3. #863: Structured Logging & Output Capture (8 sub-issues)
**Status**: Ready for implementation  
**Dependencies**: #862 (test harness)  
**Blocks**: #864  
**Effort**: 8-10 days  
**Labels**: `M7`, `M7.output`, `phase-3`, `testing`

| Issue | Type | Effort | Status |
|-------|------|--------|--------|
| #1061 | #863.1 — Session directory mgmt | low | Ready |
| #1065 | #863.2 — JSON event logging | med | Ready |
| #1067 | #863.3 — Metrics aggregation | med | Ready |
| #1071 | #863.4 — HTML report generation | med | Ready |
| #1075 | #863.5 — Log rotation | low | Ready |
| #1077 | #863.6 — JSON export | low | Ready |
| #1078 | #863.7 — Unit tests | low | Ready |
| #1081 | #863.8 — Integration test | med | Ready |

**Critical Path**: #1061 → #1065 → #1067 → #1071 → #1081

---

### 4. #864: Overnight Test Runner CLI (8 sub-issues)
**Status**: Ready for implementation  
**Dependencies**: #863 (output capture)  
**Blocks**: None (can ship independently)  
**Effort**: 8-10 days  
**Labels**: `M7`, `M7.cli`, `phase-3`, `cli`

| Issue | Type | Effort | Status |
|-------|------|--------|--------|
| #1088 | #864.1 — CLI subcommand | med | Ready |
| #1091 | #864.2 — Config loading | low | Ready |
| #1096 | #864.3 — Dry-run mode | low | Ready |
| #1099 | #864.4 — Runner spawning | med | Ready |
| #1104 | #864.5 — Progress monitoring | med | Ready |
| #1108 | #864.6 — Ctrl+C shutdown | med | Ready |
| #1113 | #864.7 — Exit codes | low | Ready |
| #1116 | #864.8 — Integration test | med | Ready |

**Critical Path**: #1088 → #1091 → #1099 → #1104 → #1108 → #1116

---

## Cross-Cutting Concerns (12+ Issues)

### 5. #865: Error Handling & Recovery [PARENT]
**Status**: Defined, ready for phase-4  
**Dependencies**: All core components (#861-864)  
**Effort**: 8-10 days  
**Labels**: `M7`, `M7.stability`, `phase-4`

| Issue | Type | Effort |
|-------|------|--------|
| #1134 | #865.1 — Login retry with backoff | med |
| #1138 | #865.2 — Circuit breaker | med |
| (More) | #865.3-5 — Process timeout, crash recovery, memory leak detection | — |

**Rationale**: Makes overnight tests resilient to transient failures, server downtime, stuck processes, and memory leaks.

---

### 6. #866: Account Safety & Detection [PARENT]
**Status**: Defined, ready for phase-4  
**Dependencies**: All core components  
**Effort**: 5-7 days  
**Labels**: `M7`, `M7.safety`, `phase-4`, `critical`

| Issue | Type | Effort |
|-------|------|--------|
| #1143 | #866.1 — Ban/suspension detection | med |
| #1149 | #866.2 — GM alert detection | low |
| (More) | #866.3-4 — Rate limiting, server status monitoring | — |

**Rationale**: Protects accounts from bans, detection, and suspension during overnight testing.

---

### 7. #870: Testing Infrastructure - Mocks & Stubs [PARENT]
**Status**: Defined, used by all integration tests  
**Dependencies**: None (foundational)  
**Effort**: 3-5 days  
**Labels**: `M7`, `M7.testing`, `phase-1`

| Issue | Type | Effort |
|-------|------|--------|
| (Detailed) | #870.1-3 — Mock processes, stubbed scenarios, test data generators | — |

**Rationale**: Enables full testing on all platforms (macOS, Linux) without live EQ clients.

---

### 8. #871: Configuration Management [PARENT]
**Status**: Defined, nice-to-have for phase-2+  
**Dependencies**: None (can be added independently)  
**Effort**: 3-5 days  
**Labels**: `M7`, `M7.config`, `phase-3`

| Issue | Type | Effort |
|-------|------|--------|
| (Detailed) | #871.1-3 — Test profiles, scenario parameters, resource limits | — |

**Rationale**: Makes overnight testing flexible and customizable for different deployment scenarios.

---

## Implementation Roadmap

### Phase 1: Foundation (Weeks 1-2)
**Core**: #861 (logout mechanism)  
**Infrastructure**: #870 (mocks & stubs)

Deliverable: Clean login→logout cycle, tested integration tests

### Phase 2: Test Harness (Weeks 2-3)
**Core**: #862 (test scenarios + runner)

Deliverable: Runnable test scenarios, TestLoopRunner coordination

### Phase 3: Output & CLI (Weeks 3-4)
**Core**: #863 (output capture), #864 (CLI)  
**Nice-to-have**: #871 (config management)

Deliverable: `textquest overnight-test` command, session reports, HTML reports

### Phase 4: Validation & Polish (Week 4+)
**Important**: #865 (error handling), #866 (account safety)  
**Optional**: Monitoring (#867), docs (#868), CI (#869)

Deliverable: Production-ready overnight testing with safeguards

---

## Dependency Graph

```
┌──────────────────────────────────────────┐
│ #870: Mocks & Stubs (Foundation)         │
│ Used by: All integration tests            │
└──────────────────────────────────────────┘

         ↓

┌──────────────────────────────────────────┐
│ #861: Logout Mechanism                    │
│ Deliverable: login→logout cycle           │
└──────────────────────────────────────────┘
         ↓
┌──────────────────────────────────────────┐
│ #862: Test Harness                        │
│ Deliverable: TestLoopRunner + scenarios   │
└──────────────────────────────────────────┘
         ↓
┌──────────────────────────────────────────┐
│ #863: Output Capture                      │
│ Deliverable: session reports, metrics     │
└──────────────────────────────────────────┘
         ↓
┌──────────────────────────────────────────┐
│ #864: CLI Integration                     │
│ Deliverable: overnight-test command       │
└──────────────────────────────────────────┘

In Parallel (after core):

┌──────────────────────────────────────────┐
│ #865: Error Handling & Recovery           │
│ #866: Account Safety & Detection          │
│ #867: Monitoring & Observability (opt)    │
│ #868: Documentation (opt)                 │
│ #869: CI Integration (opt)                │
│ #871: Configuration (opt)                 │
└──────────────────────────────────────────┘
```

---

## Tag Taxonomy & Usage

All issues tagged with:

1. **Phase**: `phase-1`, `phase-2`, `phase-3`, `phase-4`
2. **Component**: `M7`, `M7.logout`, `M7.harness`, `M7.output`, `M7.cli`, etc.
3. **Type**: `testing`, `enhancement`, `infrastructure`, `bug`, etc.
4. **Priority**: `critical`, `important`, `nice-to-have`
5. **Effort**: `low-effort`, `low`, `med`, `high`

Example: `enhancement, M7, M7.harness, phase-2, testing, med, critical`

---

## Key Design Decisions

1. **Isolation**: Each account runs independently (no group coordination)
2. **Graceful Shutdown**: Ctrl+C → logout all → final report
3. **Metrics Focus**: Pulls/hour, kills/hour, DPS, success rates
4. **Platform Support**: macOS (mocks), Windows (live), Linux (if time)
5. **Error Recovery**: Exponential backoff, circuit breaker, checkpointing
6. **Account Safety**: Ban detection, GM alerts, rate limiting, server status

---

## Success Criteria - Phase 0 Complete

✅ Comprehensive gap analysis completed  
✅ 70+ issues created and tagged  
✅ Dependencies mapped and documented  
✅ Effort estimates assigned  
✅ Clear phase breakdown (4 weeks + polish)  
✅ All issues ready for implementation  

Next step: Start with #861 (logout mechanism) in phase 1.

---

## References

- **Planning**: `docs/OVERNIGHT-TESTING-PLAN.md`
- **Gaps**: `docs/OVERNIGHT-GAPS-ANALYSIS.md`
- **Slices**: `docs/OVERNIGHT-ISSUE-SLICES.md`
- **AGENTS.md**: Workflow for Claude-routed work
- **CLAUDE.md**: Project conventions and setup
