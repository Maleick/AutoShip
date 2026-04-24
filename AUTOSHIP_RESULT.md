# AutoShip Result - Issue #758: Binary Diff Phase 2 Call Graph Propagation

## Status: COMPLETE

## Files Created / Modified

| File | Action |
|------|--------|
| `tools/eqdiff/src/callgraph.rs` | Created call graph construction, propagation, confidence, report types, and tests |
| `tools/eqdiff/src/lib.rs` | Exported the new call graph API |

## What Was Implemented

- Added `BinaryFunction`, `CallEdge`, and `CallGraph` for RVA-based function body and call adjacency modeling.
- Added `build_call_graph` to scan known function bodies for direct `E8 rel32` calls and `FF 15 rel32` memory-indirect calls.
- Added `propagate_function_matches` to iteratively expand seed string matches through same-offset caller/callee relationships until no new matches are found.
- Added confidence scoring: seed matches are `1.0`, first-hop propagation is `0.9`, and multi-hop propagation decays by `0.85` per additional hop.
- Added `MatchReport` with matched functions plus `unmatched_old` and `unmatched_new` function RVA lists.

## Tests Added

- Direct relative call graph extraction to known function targets.
- `FF 15` indirect call discovery.
- Iterative propagation across multiple hops with confidence decay.
- Unmatched function reporting when call-site propagation cannot match a callee.

## Verification

- `cargo test -p eqdiff callgraph` - passed, 4/4 callgraph tests.
- `cargo fmt --check` - passed.
- `cargo test -p eqdiff` - passed, 17/17 eqdiff tests.
- `cargo clippy -p eqdiff --all-targets --all-features -- -D warnings` - passed.
- `cargo clippy --all-targets --all-features -- -D warnings` - passed.
- `cargo test` - passed, 4155 passed, 12 ignored.
- `python3 scripts/dev-preflight.py` - passed on final run, 18 passed, 0 warnings, 0 failed.

## Notes

- An earlier preflight run failed in an unrelated `textquest-web` config-path test after generated config artifacts were present. After removing those generated artifacts, the exact failing test passed when rerun, and the final full preflight passed.
- `docs/wiki/Eqdiff-Callgraph-Propagation.md` added to satisfy the wiki-gate CI check (public API + usage sketch for the new callgraph module).
- Wiki-gate fixer pass: `cargo check -p eqdiff` clean; commit pushed to `autoship/issue-758`; PR #2419 rollup has no failing checks (semgrep pass, auto-* workflows skipping/pending as expected).
