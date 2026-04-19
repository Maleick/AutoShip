# Overnight Testing System - Complete Specification

**Status**: ✅ **READY FOR IMPLEMENTATION**  
**Last Updated**: 2026-04-13  
**Total Issues**: 118+ (70 core + 48 infrastructure)  
**Estimated Duration**: 6-8 weeks (4 weeks core + 2-4 weeks polish)

---

## Executive Summary

Complete specification for autonomous overnight EverQuest testing infrastructure with:
- ✅ 70+ core issues (login, scenarios, output, CLI)
- ✅ 48 infrastructure issues (testing, quality, safety)
- ✅ Automated branch cleanup
- ✅ Code quality enforcement (clippy -D warnings)
- ✅ Unit test coverage requirements (80%+)
- ✅ CI/CD pipeline with autofix
- ✅ Cross-platform testing (macOS, Windows, Linux)
- ✅ Comprehensive documentation

---

## Part 1: Core System (70 Issues)

### Phase 1: Foundation (5-7 days)

**#861: Logout Mechanism** (6 issues)
- `#1019`: LoginPhase enum with logout phases
- `#1022`: LogoutStateMachine
- `#1025`: LogoutSequencer
- `#1028`: QuitGame IPC command
- `#1031`: Integration test
- `#1032`: CLI manual logout

**#870: Testing Infrastructure** (3 issues)
- Mock processes for macOS
- Stubbed scenarios
- Test data generators

**Status**: Ready to implement

### Phase 2: Test Harness (10-14 days)

**#862: Test Scenarios** (9 issues)
- `#1034`: TestScenario trait
- `#1036`: CampLoopScenario
- `#1039`: NavigationScenario
- `#1042`: CombatRotationScenario
- `#1046`: Mock scenarios
- `#1047`: TestLoopRunner
- `#1050`: Metrics framework
- `#1052`: Unit tests
- `#1055`: Integration test

**Status**: Blocks output capture

### Phase 3: Output & CLI (8-10 days)

**#863: Output Capture** (8 issues)
- `#1061`: Session directory management
- `#1065`: JSON event logging
- `#1067`: Metrics aggregation
- `#1071`: HTML report generation
- `#1075`: Log rotation
- `#1077`: JSON export
- `#1078`: Unit tests
- `#1081`: Integration test

**#864: CLI Integration** (8 issues)
- `#1088`: overnight-test subcommand
- `#1091`: Config loading
- `#1096`: Dry-run mode
- `#1099`: Runner spawning
- `#1104`: Progress monitoring
- `#1108`: Graceful shutdown
- `#1113`: Exit codes
- `#1116`: Integration test

**Status**: Production-ready after phase 3

### Phase 4: Error Handling & Safety

**#865: Error Handling** (5 issues)
- `#1134`: Login retry with backoff
- `#1138`: Circuit breaker
- More: process timeout, crash recovery, memory leaks

**#866: Account Safety** (4 issues)
- `#1143`: Ban detection
- `#1149`: GM alerts
- More: rate limiting, server status

**Status**: Completes reliability

---

## Part 2: Infrastructure & Quality (48 Issues)

### Branch Cleanup (#900-902)

**#1200: #900 - Auto-delete merged branches**
- Implementation: `.github/workflows/branch-cleanup.yml` ✅
- Protects: master, main, develop
- Targets: claude/*, codex/*, feature/*

### Testing Standards (#903-906)

**#1202: #903 - Unit test coverage 80%+**
- Setup coverage tracking (tarpaulin)
- Enforce minimum coverage
- Per-module thresholds
- Block PRs if below 80%

### Code Quality (#907-910)

**#1204: #907 - Clippy -D warnings in CI**
- Fail CI if any clippy warnings
- Block merge on violations
- Create exemption list with justification

**#1207: #911 - Auto-apply clippy fixes**
- Run `cargo clippy --fix` in CI
- Commit results to PR branch
- Safe fixes only (no semantics)

### Enhanced Testing (#915-919)

**#1210: #915 - Cross-platform matrix**
- Test: macOS, Windows, Linux
- Rust: stable, nightly
- Smart exclusions to save CI time

### Documentation (#924-928)

**#1214: #924 - Architecture Decision Records**
- ADR-001: Logout sequencer design
- ADR-002: TestScenario architecture
- ADR-003: JSON event streaming
- ADR-004: Per-account runners
- ADR-005: Graceful shutdown
- ADR-006: Metrics percentiles

**#1212: #925 - Troubleshooting Decision Tree**
- 15+ failure mode diagnosis
- Diagnostic commands
- Remediation steps
- Escalation path

---

## Complete Issue Map

```
Core System (70 issues)
├─ Phase 1: #861 (6) + #870 (3) ................... 5-7 days
├─ Phase 2: #862 (9) ............................. 10-14 days
├─ Phase 3: #863 (8) + #864 (8) .................. 8-10 days
└─ Phase 4: #865 (5) + #866 (4) .................. 5-7 days

Infrastructure & Quality (48 issues)
├─ Branch Cleanup: #900-902 (3)
├─ Testing Standards: #903-906 (4)
├─ Code Quality: #907-910 (4)
├─ Autofix: #911-914 (4)
├─ Enhanced Testing: #915-919 (5)
├─ CI/CD: #920-923 (4)
├─ Documentation: #924-928 (5)
├─ Monitoring: #929-932 (4)
├─ Safety: #933-936 (4)
├─ Logging: #937-940 (4)
├─ Compatibility: #941-944 (4)
└─ Dependencies: #945-948 (4)
```

---

## Critical Success Factors

### 1. Unit Testing Requirement (ALL ISSUES)

Every issue must include:

```markdown
## Unit Tests
- [ ] Module initialization (no panic)
- [ ] Happy path (core functionality works)
- [ ] Edge case: empty/minimal input
- [ ] Edge case: large input (100K items)
- [ ] Error case: invalid state transition
- [ ] Error case: timeout condition
- [ ] Concurrency: thread-safe with 100 threads
- [ ] Memory: stable over 1000 iterations (<5% growth)
- [ ] Coverage: >80% line coverage (use tarpaulin)
- [ ] Flakiness: passes 10x consistently
```

### 2. Code Quality Requirement (ALL ISSUES)

Every issue must include:

```markdown
## Code Quality Checklist
- [ ] Clippy: no warnings (cargo clippy --all -- -D warnings)
- [ ] Format: passes (cargo fmt --check)
- [ ] Docs: public API documented (doc comments)
- [ ] Error messages: clear and actionable
- [ ] No unwrap() in libraries (only in bins)
- [ ] No panic in async code
- [ ] No unnecessary .clone() calls
- [ ] Type-safe (no `as` casts without reason)
- [ ] No unsafe blocks without safety comment
```

### 3. Integration Testing Requirement (ALL MAJOR ISSUES)

Every major issue must include:

```markdown
## Integration Tests
- [ ] Full flow works end-to-end
- [ ] Cross-platform: macOS (mock), Windows (real)
- [ ] Failure mode: handles gracefully
- [ ] Performance: completes in <X seconds
- [ ] No resource leaks (processes, files, memory)
- [ ] Ctrl+C shutdown: clean and complete
```

---

## Implementation Timeline

| Phase | Duration | Issues | Deliverable |
|-------|----------|--------|-------------|
| **Phase 1** | 5-7 days | 9 (#861, #870) | Login→logout cycle working |
| **Phase 2** | 10-14 days | 9 (#862) | TestLoopRunner + 4 scenarios |
| **Phase 3** | 8-10 days | 16 (#863, #864) | `textquest overnight-test` CLI |
| **Phase 4** | 5-7 days | 9 (#865, #866) | Error handling + safety |
| **Infrastructure** | 2-3 weeks | 48 (#900-948) | Quality gates + automation |
| **Polish** | 1-2 weeks | Documentation + manual testing | Production-ready |
| **TOTAL** | **6-8 weeks** | **118+** | **Overnight testing system** |

---

## Branch Cleanup (Already Implemented)

✅ **File**: `.github/workflows/branch-cleanup.yml`

Features:
- Auto-deletes merged feature branches (claude/*, codex/*, feature/*)
- Protects: master, main, develop, release/*, hotfix/*
- Manual `workflow_dispatch` for stale branch cleanup
- Safe comparison checks to prevent false positives
- Clear logging: deleted/skipped/kept counts

---

## Tag Schema (Used on All 118+ Issues)

```
Phase:          phase-1, phase-2, phase-3, phase-4
Component:      M7, M7.logout, M7.harness, M7.output, M7.cli
                M7.stability, M7.safety, M7.testing, M7.config
Infrastructure: testing, quality, ci, automation, infrastructure
Priority:       critical, important, nice-to-have
Effort:         low-effort, low, med, high
Type:           enhancement, testing, infrastructure, documentation
```

---

## Success Metrics

### Phase 1 Gate (All #861 issues complete)
- ✓ Login→logout cycle works on Windows
- ✓ Integration tests pass (macOS mock, Windows real)
- ✓ No hanging processes
- ✓ >80% code coverage
- ✓ Clippy -D warnings clean

### Phase 2 Gate (All #862 issues complete)
- ✓ TestLoopRunner functional
- ✓ 4 scenarios implemented
- ✓ Metrics collected correctly
- ✓ 3 iterations in <5 minutes
- ✓ Cross-platform tests pass

### Phase 3 Gate (All #863, #864 issues complete)
- ✓ `textquest overnight-test` works
- ✓ Session reports generated
- ✓ HTML reports readable
- ✓ 1-hour test run completes
- ✓ Ctrl+C shutdown clean

### Phase 4 Gate (All #865, #866 issues complete)
- ✓ 99%+ login success rate
- ✓ 95%+ scenario completion
- ✓ Ban detection works
- ✓ Memory stable (<5% per hour)
- ✓ 8+ hour run without restart

### Production Gate (Infrastructure complete)
- ✓ Branch cleanup automation working
- ✓ Code quality: Clippy -D clean, 80%+ coverage
- ✓ Unit tests: all flaky tests quarantined
- ✓ Cross-platform: tests pass macOS/Windows/Linux
- ✓ Documentation: ADRs + troubleshooting guide complete
- ✓ Manual testing: 2+ hour Windows run successful

---

## Files & Locations

### Planning Documents
- `docs/OVERNIGHT-TESTING-PLAN.md` — Architecture & design
- `docs/OVERNIGHT-GAPS-ANALYSIS.md` — Original 10 gap categories
- `docs/OVERNIGHT-ADDITIONAL-GAPS.md` — 48 additional gaps
- `docs/OVERNIGHT-ISSUE-SLICES.md` — Detailed issue breakdown
- `docs/OVERNIGHT-ISSUE-SUMMARY.md` — Issue roadmap
- `docs/OVERNIGHT-COMPLETE-SYSTEM.md` — This file

### Automation
- `.github/workflows/branch-cleanup.yml` — Auto-delete merged branches ✅
- `.github/workflows/automation.yml` — Post-merge cleanup (existing)

### Branch
- `claude/refine-automated-testing-q1h46` — All work pushed here

---

## How to Use This Document

1. **For Phase Planning**: Read timeline table above
2. **For Issue Details**: Check GitHub (#1019-#1214+)
3. **For Code Standards**: See "Critical Success Factors" section
4. **For Architecture**: Read ADRs (to be created in #924)
5. **For Troubleshooting**: See troubleshooting guide (to be created in #925)

---

## Automated Checks (CI/CD)

All PRs must pass:
```
✓ cargo fmt --check
✓ cargo clippy --all -- -D warnings
✓ cargo test --all
✓ cargo tarpaulin (>80% coverage)
✓ cargo audit (no security issues)
✓ Cross-platform tests (macOS, Windows, Linux)
```

---

## Conclusion

This document defines a **complete, production-grade overnight testing system** with:

- **118+ issues** covering all aspects
- **Comprehensive unit test requirements** (80%+ coverage)
- **Strict code quality enforcement** (clippy -D warnings)
- **Automated branch cleanup** and CI/CD pipeline
- **Cross-platform testing** on all major OS
- **6-8 week timeline** with clear gates
- **Documentation and troubleshooting** guides
- **Architecture Decision Records** for maintainability

**Status**: ✅ Ready to implement. Start with Phase 1 (#861 logout mechanism).

---

**Session**: https://claude.ai/code/session_01BXjhDtTw3cp2rneLUvSGT9
