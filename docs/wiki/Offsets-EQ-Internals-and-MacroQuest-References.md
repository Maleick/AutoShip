# Offsets, EQ Internals, and MacroQuest References

## Current Rules

The repository no longer treats local vendored reference trees as part of the normal workflow.

Use these sources in order:

- current checked-in code under `textquest/`, `textquest-dll/`, and `textquest-common/`
- `docs/implementation-roadmap.md` and the matching wiki pages
- public upstream eqlib or MacroQuest references when you need outside comparison material

For roadmap-facing research promotion, combine those references with:

- `docs/implementation-roadmap.md`
- `docs/external-research/automation-source-ledger.md`

## Offset Model

The compiled offsets live in `textquest-common/src/offsets.rs`.

Current repo rules:

- addresses are stored at EQ's preferred base
- they are not ready-to-dereference runtime pointers
- call `rebase(preferred_addr, actual_base)` before use

This is one of the most important codebase conventions.

## Field Reading Strategy

TextQuest intentionally reads many EQ fields one by one rather than casting large C structs wholesale.

Reason:

- MQ2 and eqlib layouts can contain padding, gaps, or version-sensitive fields
- TextQuest often only needs selected offsets
- field-by-field reads are safer when layouts are not perfectly contiguous

If you see code like:

```rust
proc.read::<T>(addr + OFFSET)
```

repeated across a type, that is usually deliberate.

## Spawn and Zone Internals

Important current patterns:

- spawn traversal is based on the linked-list style structures exposed through EQ
- zone and nav routing information is shared through `textquest-common/src/nav.rs`
- login internals and widget behavior can be cross-checked against public upstream references when needed

## Offsets Database

`config/offsets.json` is the checked-in offset database snapshot.

Use it for:

- maintaining the shared JSON schema in `textquest-common/src/offset_db.rs`
- tracking live-client corrections in a reviewable file
- comparing JSON-backed offset data with the compiled defaults in `textquest-common/src/offsets.rs`

## Recommended Investigation Workflow

1. inspect the current checked-in code path using the offset
2. compare against the current code path using the offset
3. update `textquest-common/src/offsets.rs` and, if needed, `config/offsets.json`
4. validate on a live Windows client
5. update the relevant wiki page if the operator or developer workflow changed
6. if the work changes roadmap assumptions or evidence state, update `docs/implementation-roadmap.md`

## Current Behavior vs Roadmap

### Current behavior

- Public upstream references can still be useful for comparison work, but checked-in TextQuest code and docs remain the primary source of truth.
- The repo does not rely on deleted local vendor paths for normal build, test, or runtime work.

### Validation notes

- Offset work is inherently patch-sensitive.
- Treat any successful build without live EQ validation as incomplete proof for offset changes.

## Related References

- [EQ Coordinate System](../eq-coordinate-system.md) — Mapping between game coordinates and navigation systems
