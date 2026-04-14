# Evidence-State Label System

The evidence-state label system tracks the validation maturity of implemented features, bug fixes, and research work. Each label represents a progression from initial implementation through full validation, with a regression state for previously-validated work that has broken.

## Label Definitions

### evidence:provisional

**Color:** #fbca04 (yellow)

Initial implementation or code change without any testing or validation on a target system. Use this label when:

- Code is newly written and merged but untested
- A feature is implemented in theory but not executed in practice
- An approach is taken without external validation
- The work may break or need significant revision

**Progression:** Apply this when a PR is merged. Remove once testing begins.

---

### evidence:research

**Color:** #0e8a16 (green)

Research backing exists for the implementation, including documentation, offset verification, or reference material from authoritative sources (MQ2, EQ reverse engineering docs, community knowledge). Use this label when:

- Offsets have been verified against MQ2 headers or live process inspection
- A strategy is documented with reasoning and citations
- External sources confirm the approach is sound
- The implementation matches published patterns or specifications

**Progression:** Apply when research validates the approach. This is a prerequisite for live testing.

---

### evidence:live-proof

**Color:** #1d76db (blue)

The implementation has been tested on a live system (real EverQuest client, live TLP, or production environment). Use this label when:

- Code has been executed in a real game environment
- Live testing confirms the feature works or the fix resolves the reported issue
- Data collected from live execution validates the approach
- The implementation survived at least one live session

**Progression:** Apply when live testing succeeds. This is required before `evidence:validated`.

---

### evidence:validated

**Color:** #5319e7 (purple)

Full validation is complete: the implementation is live-tested, regression-tested, and considered production-ready. Use this label when:

- Multiple live test sessions confirm stability
- Edge cases have been explored and handled
- The feature integrates without breaking other systems
- The fix has been confirmed across different conditions or class compositions

**Progression:** Apply when all validation gates pass. Represents the final state before removal.

---

### evidence:regression

**Color:** #d93f0b (red)

A previously-validated feature has broken or a regression has been introduced. Use this label when:

- A validated feature stops working after a patch or update
- An offsets change breaks a previously-working integration
- A regression test fails on code that previously passed
- A bug report indicates a validated feature is now failing

**Progression:** Apply immediately upon detection. Remove once the issue is fixed and re-validated.

---

## Progression Workflow

```
provisional
    ↓
research (concurrent or sequential)
    ↓
live-proof
    ↓
validated
    ↓
[remove all labels or archive issue]
    ↑
    └← regression (reapply when caught)
```

### Typical Journey

1. **PR merged** → `evidence:provisional`
2. **Offsets verified** → add `evidence:research`
3. **Tested on live TLP** → add `evidence:live-proof`
4. **Multiple sessions stable** → add `evidence:validated`
5. **After patch breaks it** → remove all, add `evidence:regression`
6. **Fixed and re-tested** → remove `evidence:regression`, add `evidence:live-proof` + `evidence:validated`

## When to Apply Each Label

| Scenario                         | Label                  | Reason                      |
| -------------------------------- | ---------------------- | --------------------------- |
| Just merged, no testing          | `evidence:provisional` | Starting point for all work |
| Offsets verified against MQ2     | `evidence:research`    | Backing material exists     |
| Tested on Teek or live character | `evidence:live-proof`  | Real execution validated    |
| Stable across 3+ sessions        | `evidence:validated`   | Confidence in durability    |
| Breaks after patch               | `evidence:regression`  | Known breakage documented   |

## Integration with GitHub Workflow

### For Issue Authors

- When closing an issue, add the appropriate evidence label to document confidence level
- Use `evidence:provisional` if merging partial/experimental work
- Use `evidence:validated` for stable, production-ready fixes

### For Reviewers

- Expect `evidence:research` before approving complex changes involving offsets
- Require `evidence:live-proof` before marking an issue as "ready to ship"
- Flag missing evidence labels in review comments

### For Autonomous Agents

- Agents may apply `evidence:provisional` when merging their own PRs
- Agents must apply `evidence:live-proof` when autonomous testing succeeds
- Regression detection should trigger `evidence:regression` + `agent:blocked`

## Examples

### Feature: AutoLogin Credential Capture

- Created with `evidence:provisional`
- Added `evidence:research` after offset verification against MQ2 headers
- Tested on Teek (alt server, no truebox) → `evidence:live-proof`
- Stable across 5+ characters → `evidence:validated`

### Bug: Spawn list corruption on zone-in

- Reported with `evidence:regression` (was working, now broken after patch)
- Fixed and tested on live → `evidence:live-proof`
- Verified on 4 zone-ins → `evidence:validated`

### Optimization: Memory read batching

- Implemented → `evidence:provisional`
- Documented with benchmarks → `evidence:research`
- Live perf test confirms 20% speedup → `evidence:live-proof`
- No regressions after 3 sessions → `evidence:validated`

## Notes

- Labels are **orthogonal** to milestone labels (m6, m7, m8, post-m7)
- Labels are **independent** of agent status labels (agent:ready, agent:working, agent:blocked)
- An issue can have multiple evidence labels (e.g., `evidence:research` + `evidence:live-proof`)
- Remove all evidence labels when archiving or closing an issue, unless closing with a specific state (e.g., keep `evidence:validated` to show the final state)

---

_Last updated: 2026-04-14_
