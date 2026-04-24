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

The `eqdiff` crate can now convert binary-diff function matches into this same
schema. Its offset export layer maps matched function names such as
`CharacterZoneClient::CastSpell` to TextQuest function keys such as `castSpell`,
then returns an `OffsetDatabase` that is ready for `save_to_file()` or
`load_from_file()` validation. Review the generated diff report for moved,
added, removed, and unmapped functions before promoting an offset snapshot.

## Recommended Investigation Workflow

1. inspect the current checked-in code path using the offset
2. compare against the current code path using the offset
3. for binary-diff evidence, export matches through `eqdiff` and inspect the generated report
4. update `textquest-common/src/offsets.rs` and, if needed, `config/offsets.json`
5. validate on a live Windows client
6. update the relevant wiki page if the operator or developer workflow changed
7. if the work changes roadmap assumptions or evidence state, update `docs/implementation-roadmap.md`

## Current Behavior vs Roadmap

### Current behavior

- Public upstream references can still be useful for comparison work, but checked-in TextQuest code and docs remain the primary source of truth.
- The repo does not rely on deleted local vendor paths for normal build, test, or runtime work.

### Validation notes

- Offset work is inherently patch-sensitive.
- Treat any successful build without live EQ validation as incomplete proof for offset changes.

## Module-Scoped Offset Parity (issue #762 / PR #2422)

TextQuest now tracks offsets per EQ module, not just `eqgame.exe`. The offset database and scan engine understand addresses that live inside auxiliary modules loaded at independent runtime bases.

### Supported module scopes

`config/offsets.json` now carries module-scoped sections alongside the existing `eqgame` groups:

- `eqmain_globals` — login-stage globals in `eqmain.dll` (e.g. `sidlManager`, `loginServerApi`, `cxwndManager`, `loginViewManager`, `pinstLoginClient`, `pinstLoginController`)
- `eqmain_functions` — login-stage entry points in `eqmain.dll` (e.g. `joinServer`, `charSelectEnterWorld`, `serverSelect`, `handleSplash`, `charSelectSelectCharacter`, `charSelectSetFocus`, `loginControllerGiveTime`)
- `eqgraphics_globals` / `eqgraphics_functions` — render-pipeline hooks in `eqgraphicsdx9.dll` (e.g. `realRenderWorld`, `deviceReset`, `initRender`, `renderFrame`, `dxPresent`)

Each entry is stored at its module's preferred base — same rule as the pre-existing `eqgame` groups. `rebase(preferred_addr, actual_module_base)` is still mandatory, but the `actual_module_base` must now come from the matching module handle (`GetModuleHandleA("eqmain.dll")` / `GetModuleHandleA("eqgraphicsdx9.dll")`) rather than the main-image base.

### Scan engine behavior

`textquest-common/src/scan_engine.rs` accepts a module scope on every resolution request and resolves the active base per module. Mixing modules in a single scan pass is supported; each address is rebased against its declaring module.

### Validation

`scripts/validate_offsets_sync.py` validates that every module-scoped group in `config/offsets.json` has a matching compiled entry in `textquest-common/src/offset_db.rs`. Coverage is checked by `tests/test_validate_offsets_sync.py`. Run:

```
python scripts/validate_offsets_sync.py
```

before landing new module-scoped offsets, and re-validate on a live Windows client since offsets remain patch-sensitive.

## Related References

- [EQ Coordinate System](../eq-coordinate-system.md) — Mapping between game coordinates and navigation systems
