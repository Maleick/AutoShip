# Offsets, EQ Internals, and MacroQuest References

## Current Rules

The reference submodules are not required for normal build, test, or runtime work.

Before doing any offset or struct work, sync them with:

```bash
git submodule update --init --recursive
```

Then use:

- `third_party/eqlib` as the canonical local eqlib reference tree
- `third_party/macroquest` as the broader upstream MacroQuest reference tree

Do not treat the vendored `third_party/macroquest/src/eqlib` copy as the primary TextQuest citation path.

For roadmap-facing research promotion, combine these local references with:

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

- MQ2/eqlib layouts can contain padding, gaps, or version-sensitive fields
- TextQuest often only needs selected offsets
- field-by-field reads are safer when layouts are not perfectly contiguous

If you see code like:

```rust
proc.read::<T>(addr + OFFSET)
```

repeated across a type, that is usually deliberate.

## Spawn and Zone Internals

Important current patterns:

- spawn traversal is based on the linked-list style structures exposed through EQ and documented in the reference trees
- zone and nav routing information is shared through `textquest-common/src/nav.rs`
- login internals and widget behavior are cross-checked against local MacroQuest references when needed

## Offsets Database

`config/offsets.json` is the hot-updatable offset database.

Use it for:

- updating data without recompiling everything
- tracking live-client corrections
- keeping runtime offset overrides separate from compiled defaults

## Recommended Investigation Workflow

1. sync submodules
2. inspect `third_party/eqlib`
3. compare against the current code path using the offset
4. update `textquest-common/src/offsets.rs` and, if needed, `config/offsets.json`
5. validate on a live Windows client
6. update the relevant wiki page if the operator or developer workflow changed
7. if the work changes roadmap assumptions or evidence state, update `docs/implementation-roadmap.md`

## Current Behavior vs Roadmap

### Current behavior

- The submodule-based reference layout is the current reality of this repo for research work.
- Those reference trees are not required for normal build, test, or runtime work.
- `third_party/eqlib` is the canonical eqlib path and should be cited that way in docs and PRs.

### Validation notes

- Offset work is inherently patch-sensitive.
- Treat any successful build without live EQ validation as incomplete proof for offset changes.
