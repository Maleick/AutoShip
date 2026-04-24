# Derived Struct Offsets From Bytecode

This page documents the `ExtractDisplacement` resolution mode in
`textquest-common` that lets the offset scanner recover struct-field byte
offsets directly from accessor-function bytecode, rather than relying on
hand-curated compile-time constants.

## Motivation

Struct layouts in the EQ client change more often than function entry points:
any patch that adds, removes, or reorders a field shifts the displacement used
inside accessor instructions such as:

```asm
mov eax, [rcx+0x3A0]   ; CPlayerZone::hpCurrent
```

Previously, every field in `offsets::player_zone`, `offsets::player_base`,
`offsets::spawn_manager`, etc. had to be re-read by a human and re-committed
whenever the client changed. With `ExtractDisplacement`, the scanner finds the
accessor by IDA pattern and reads the displacement byte(s) out of the matched
instruction, producing a struct-field offset that can be merged straight into
`OffsetDatabase`.

## How It Works

`ScanEntry::resolve` may now be one of:

- `Direct` — resolved value is the preferred-base address of the match itself
  (functions).
- `RipRelative { ... }` — decode a RIP-relative displacement to obtain a
  preferred-base address (globals).
- `ExtractDisplacement { disp_offset, disp_size }` — read a `disp_size`-byte
  little-endian displacement at `match_off + disp_offset`. The value is a
  **struct-field byte offset**, not a module address.

The extractor sign-extends the decoded value to `i64` (since x86/x64 memory
displacements are signed two's-complement) and rejects negative displacements,
because struct fields never live at a negative offset from their containing
object. This guards `apply_to_offset_db` from ever persisting a bogus huge
positive value like `0xFFFF_FFF0` into the struct-field maps.

`ScanEntry::category` disambiguates where the resolved value lands:

- `Function` / `Global` → `OffsetDatabase::functions` / `globals`
  (preferred-base addresses).
- `PlayerBaseField`, `PlayerZoneField`, `SpawnManagerField`,
  `ContextMenuManagerField`, `ContextMenuField` → the corresponding
  `HashMap<String, usize>` of struct-field byte offsets.

## Example Entry

```rust
ScanEntry {
    name: "hpCurrent".to_string(),
    module: ScanModule::EqGame,
    pattern: "8B 81 ?? ?? 00 00 3B 81".to_string(),
    category: OffsetCategory::PlayerZoneField,
    resolve: ResolveMode::ExtractDisplacement { disp_offset: 2, disp_size: 4 },
    expected_preferred: Some(offsets::player_zone::HP_CURRENT as u64),
}
```

The seed catalog lives in `pattern_db::field_displacement_scan_entries()` and
can be grown with additional Ghidra-exported accessor patterns.

## Testing

`scan_engine.rs` covers the happy paths and edge cases:

- 4-byte displacement (`mov eax, [rcx+0x3A0]`).
- 1-byte displacement (`movzx eax, byte ptr [rcx+0x64]`).
- Out-of-bounds displacement bytes record a failure rather than panicking.

Tests assign `OffsetCategory::PlayerZoneField` explicitly so the
category-dependent DB-merge path in `apply_to_offset_db` is exercised end to
end.

## Related Modules

- `textquest-common/src/pattern_db.rs` — `ResolveMode`, `ScanEntry`, seed
  catalog.
- `textquest-common/src/scan_engine.rs` — `resolve_displacement`,
  `apply_to_offset_db`, tests.
- `textquest-common/src/offsets.rs` — compile-time fallbacks used as
  `expected_preferred` anchors.
