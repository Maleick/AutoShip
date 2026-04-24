# EQDiff Binary Matching: Byte Similarity & RTTI

`eqdiff` is the TextQuest PE binary-diff toolkit used to identify and track
EverQuest client functions across client builds. This page documents the
Phase 3 matching primitives added in issue #759: **byte similarity** and
**RTTI (Run-Time Type Information) matching**.

## Why matching matters

EverQuest ships new client builds on an irregular cadence. Offsets for the
functions TextQuest hooks (combat, chat, spawn processing, etc.) move every
patch. Re-identifying those functions by hand is slow and error-prone.

`eqdiff` automates the "find the same function in the new binary" step by
combining several independent signals, each of which survives a different
class of compiler change:

| Signal              | Survives                             | Defeated by                             |
| ------------------- | ------------------------------------ | --------------------------------------- |
| String xrefs        | Reordering, inlining of siblings     | Removed/renamed log strings            |
| Import xrefs        | Most refactors                       | Switching C runtime                     |
| **Byte similarity** | Minor patches, small edits           | Major refactors, optimizer changes     |
| **RTTI names**      | Almost everything short of stripping | RTTI-stripped builds                    |

Phase 1 (#... prior) delivered PE parsing and string extraction. Phase 2
added string/import xref indexing. Phase 3 (this change) layers byte
similarity scoring and RTTI-driven vtable matching on top.

## Byte similarity

Source: `tools/eqdiff/src/similarity.rs`

### Algorithm

1. **Wildcard-mask the instruction stream.** We disassemble the function's
   bytes with `iced-x86` and mark bytes belonging to immediate operands,
   displacements, and relative jump/call targets as "wildcards". Opcode
   bytes remain literal.
2. **Hamming-compare the literal bytes** of two candidate functions over
   the length of the shorter one. Bytes where either side is wildcarded
   are excluded from the score denominator.
3. **Return a ratio in `[0.0, 1.0]`** where 1.0 means every non-wildcard
   byte matches.

This yields a score that is robust to trivial address fixups (which move
every patch because ASLR and section layout shift) while still penalizing
real instruction changes.

### Public API

- `FunctionBytes { rva, bytes }` — a function extracted from a PE.
- `byte_similarity(old: &FunctionBytes, new: &FunctionBytes) -> f64`
- `rank_candidates_by_byte_similarity(old, candidates) -> Vec<ByteSimilarityMatch>`
  Returns `(candidate_rva, score)` pairs sorted descending by score.

### Known limitations

- The wildcard decoder currently runs in 64-bit mode. For a 32-bit PE this
  can over-size moffs-class displacements and spill wildcards into the
  next instruction. Tracking fix: thread PE bitness through
  `FunctionBytes` → `wildcard_mask()`.
- `rank_candidates_by_byte_similarity()` rebuilds the `old` wildcard mask
  per candidate. Fine for tens of candidates; noticeable with thousands.
  Planned: cache the mask once per `old` function.

## RTTI matching

Source: `tools/eqdiff/src/rtti.rs`

MSVC emits RTTI descriptors in `.rdata` for every polymorphic class. Each
descriptor contains a mangled type name like `.?AVSpawn@@` (the `@@`
marks the end of the qualified name). These names are extremely stable
across builds because they come from the source — if `CSpawn` is still
called `CSpawn`, its RTTI string is identical across a decade of patches.

### What Phase 3 extracts

- `extract_rtti_type_descriptors_from_rdata(pe, bytes) -> Vec<RttiClass>`
  Walks `.rdata` looking for `.?AV` (class) and `.?AU` (struct) markers,
  reads through the terminating `@@` + NUL, and records the RVA where
  the descriptor begins.
- `match_classes_by_rtti_name(old, new) -> Vec<ClassRttiMatch>`
  Produces `(old_rva, new_rva, name)` tuples for names that appear in
  both images. This gives us ground-truth class identity across builds
  even when every other offset moved.

### Vtable slot matching

Once we have matched class descriptors, we can walk each class's vtable
(pointer array in `.rdata`, typed function pointers into `.text`) and
emit per-index matches:

- `VTable { class_rva, slot_rvas: Vec<u32> }`
- `parse_vtable_slots(pe, bytes, vtable_ea, text_rva_range) -> VTable`
- `match_vtable_slots_by_index(old_vt, new_vt) -> Vec<VTableSlotMatch>`

Because virtual functions share a fixed slot ordering for the lifetime of
a class hierarchy, matching by index hands us a direct old-function →
new-function map for every overridden virtual — typically dozens per
class — as soon as RTTI identifies the class.

### Known limitations

- `parse_vtable_slots()` computes `function_rva = (pointer - image_base) as u32`
  without checking the subtraction fits in `u32`. For a well-formed PE this
  is always true; we intend to add an explicit bound before the cast for
  defense in depth.
- A `.?AV`/`.?AU` prefix without a later NUL terminator currently halts the
  scan. A malformed or truncated `.rdata` would hide downstream names.
  Planned: advance one byte and continue on malformed markers.

## Composition

Byte similarity and RTTI matching are complementary:

1. Match classes by RTTI name → get class identity "for free".
2. Walk vtables → get a slot-indexed map of virtual functions.
3. Byte-similarity-rank any non-virtual / free functions using string and
   import xrefs as a candidate pool.

The result is a ranked, auditable old→new offset table that the rest of
TextQuest (hook table, signature tests) consumes.

## Tests

End-to-end coverage lives in
`tools/eqdiff/tests/phase3_matching.rs`. Fixtures are small hand-built
PE images that exercise each extractor in isolation plus a combined
happy path.

## See also

- `Architecture-Overview.md` — where `eqdiff` sits in the pipeline.
- `Offsets-EQ-Internals-and-MacroQuest-References.md` — downstream
  consumer of the offset map.
