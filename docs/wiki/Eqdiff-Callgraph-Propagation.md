# Eqdiff Call Graph Propagation

Call graph construction and match propagation for binary diffing between
EverQuest client builds. Lives in `tools/eqdiff/src/callgraph.rs` (exposed via
`tools/eqdiff/src/lib.rs`) and closes issue #758.

## Purpose

Given two PE images of the EverQuest client (old + new), seed a small set of
high-confidence function matches (e.g. from exports, anchors, or exact byte
matches) and propagate those matches outward through the call graph. This lets
signatures, hooks, and RE notes survive client patches without re-anchoring by
hand.

## Public Types

- `BinaryFunction { rva, size }` — discovered function body in a PE image.
- `CallEdge { caller_rva, call_site_rva, call_site_offset, target_rva, is_indirect }`
  — one call instruction inside a known function. `target_rva` is `None` for
  calls that do not resolve to a known function start; `is_indirect` is set for
  `FF 15 rel32` and similar memory-indirect forms.
- `CallGraph { functions, outgoing }` — adjacency lists keyed by caller RVA.
- `MatchConfidence { score, hop }` — `score` in `[0.0, 1.0]`, `hop` is BFS
  distance from the seed set.
- `FunctionMatch { old_rva, new_rva, confidence }`.
- `MatchReport { matches, unmatched_old, unmatched_new }`.

## Entry Points

- `build_call_graph(pe, bytes, functions) -> CallGraph` — scans function bodies
  for direct (`E8 rel32`) and memory-indirect (`FF 15 rel32`) calls, resolves
  direct targets against the supplied function table, and returns adjacency
  lists.
- `propagate_function_matches(old_graph, new_graph, seeds) -> MatchReport` —
  BFS from each seed match. A candidate old/new pair is accepted when their
  outgoing call shapes agree (same callee ordering at equivalent offsets,
  matching `is_indirect` flags). Confidence decays by hop, and ties are broken
  by call-site offset agreement.

## Usage Sketch

```rust
use eqdiff::callgraph::{build_call_graph, propagate_function_matches, FunctionMatch};

let old_graph = build_call_graph(&old_pe, &old_bytes, &old_functions);
let new_graph = build_call_graph(&new_pe, &new_bytes, &new_functions);
let report = propagate_function_matches(&old_graph, &new_graph, &seeds);
```

`report.matches` is the propagated set (including seeds); `unmatched_old` /
`unmatched_new` are the functions the propagation could not pair.

## Notes

- Indirect calls contribute to shape comparison but do not themselves produce
  new matches, since their targets are not statically resolvable.
- The algorithm is intentionally conservative: ambiguous neighbors are dropped
  rather than guessed, so downstream tooling can trust a reported match.
- See `tools/eqdiff/src/lib.rs` for re-exports and the public surface.
