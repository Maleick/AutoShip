# M10 Economy Milestone — Gap Analysis & QA Plan

**Last Updated**: 2026-04-13  
**Status**: Implementation in progress — core loops done, web API and wishlist rules in flight

---

## Implemented (Evidence-Backed)

| Module | File | Tests | Notes |
|--------|------|-------|-------|
| Loot Queue | `textquest/src/loot/queue.rs` | 6 | FIFO intake, DroppedItem |
| Ownership Model | `textquest/src/loot/ownership.rs` | 7 | 6-state FSM, AuditLog |
| Loot Distributor | `textquest/src/loot/distributor.rs` | 9 | Reserve→Assign→Execute, 3x retry backoff |
| Economy Ledger | `textquest/src/loot/ledger.rs` | — | Tracks loot/vendor/plat, TrendReport |
| Banking Cycle | `textquest/src/camp/banking.rs` | 18 | Two-level FSM, PlatLedger, consolidation |
| Wishlist Rules | `textquest/src/loot/wishlist.rs` | 8+ | WishlistManager, ReserveRule, priority resolution |
| Failure Routing | `textquest/src/economy/failure_handling.rs` | 10+ | FailureRouter, FailureHistory, escalation |

---

## In Progress / PRs Open

| Issue | Title | PR | Status |
|-------|-------|-----|--------|
| #1285 | Economy TUI controls panel | #1362 | CI pending |
| #1345 | Web API economy endpoints | #1376 | CI pending |

---

## Planned / Not Started

| Issue | Title | Priority | Notes |
|-------|-------|----------|-------|
| #1227 | Vendor Cycle Controller | P1 | Depends on loot intake |
| #1295 | Regression budget + guardrails | P2 | After baselines defined |
| #1297 | Tuning loop infrastructure | P2 | After vendor cycle |

---

## QA Coverage Targets

| Module | Current Tests | Target | Gap |
|--------|---------------|--------|-----|
| `loot/queue.rs` | 6 | 10 | +4 edge cases (empty queue, concurrent pop) |
| `loot/ownership.rs` | 7 | 12 | +5 state transition edge cases |
| `loot/distributor.rs` | 9 | 15 | +6 (retry exhaustion, idempotency) |
| `loot/ledger.rs` | 0 | 8 | +8 (all functions) |
| `camp/banking.rs` | 18 | 18 | ✅ target met |
| `loot/wishlist.rs` | 8 | 12 | +4 (condition edge cases) |
| `economy/failure_handling.rs` | 10 | 10 | ✅ target met |

---

## Recommended Implementation Order

1. **Vendor Cycle Controller** (#1227) — critical path, unblocks economy loop E2E
2. **Web API wiring** — connect economy state to real IPC instead of stubs
3. **Ledger test coverage** — bring ledger.rs tests to target
4. **Regression guardrails** (#1295) — protect against economy loop regressions
5. **Tuning infrastructure** (#1297) — metrics-driven behavior optimization
