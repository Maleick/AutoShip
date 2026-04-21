# Result: #2177 — A3 — File integrity handler (0x8bdc / 0xe91d / 0x9562) with PRNG-aware hash cache

## Status: DONE

## Changes Made

- `textquest-dll/src/hooks/file_integrity_dispatcher.rs` (new, ~490 lines):
  - **`LfgPrng`** — Rust port of `FUN_14025ABD0` (additive lagged Fibonacci
    generator, p=55 q=24, 55-element `u32` table).  Matches the in-process
    layout at `DAT_140E8D148`.
    - `from_state(&[u32; 55])` — construct from a live memory snapshot for
      cross-check against the binary (1 000 iterations)
    - `seed(u32)` — LCG expander for unit tests
    - `next() -> u32` — advance one step; recurrence:
      `X[i] = X[(i-55)%55] + X[(i-24)%55]` (mod 2^32)
    - `sample_positions(file_len) -> [u32; 256]` — generate the 256-DWORD
      sampling sequence used per integrity check
  - **`FileHashCache`** — precomputed `(full_hash, samples)` per file
    (`EqGameExe` / `BaseDataTxt` / `SkillCapsTxt`).  Set via `set_cache()`,
    gated behind `CACHE_ACTIVE: AtomicBool` (default `false` = passthrough).
  - **Byte-patch detour** (`install` / `remove`) via `retour::static_detour!`
    — same pattern as `hooks::fingerprint` (also in this crate).  Passthrough
    default: `dispatcher_detour` calls the original via
    `IntegrityDispatcherHook.call(ctx)` so all three EQ integrity checks
    execute normally.
  - `OPCODE_EXE_HASH = 0x8bdc`, `OPCODE_BASEDATA_HASH = 0xe91d`,
    `OPCODE_SKILLCAPS_HASH = 0x9562` constants.

- `textquest-dll/src/hooks/mod.rs` — added `pub mod file_integrity_dispatcher`
  to the module list.

## Tests

- Command: `cargo test --lib`
- Result: PASS (3279 tests, 11 new in `file_integrity_dispatcher`)
- Clippy: clean (`-- -D warnings`)
- New tests added: yes

### New test coverage

| Test | Verifies |
|------|----------|
| `prng_deterministic_from_seed` | Same seed → same 1000-iteration sequence |
| `prng_different_seeds_differ` | Different seeds produce different sequences |
| `sample_positions_count` | Returns exactly 256 positions |
| `sample_positions_in_range` | All positions < file_len/4 |
| `sample_positions_empty_file` | No division by zero on empty file |
| `from_state_parity_1000_iterations` | `from_state` snapshot matches live-seeded ref for 1000 calls |
| `lfg_recurrence_property` | `X[n] == X[n-55] + X[n-24]` for n ≥ 55 |
| `cache_roundtrip` | `set_cache` / `get_cache` round-trip |
| `cache_miss_returns_none` | No panic on unloaded cache |
| `opcode_mapping` | All three opcode constants correct |
| `install_remove_stub` (non-Windows) | Detour install/remove stub doesn't panic |

## Notes

- **Hook mechanism:** byte-patch detour via `retour::static_detour!`, NOT
  HWBP.  All four debug registers (DR0–DR3) are reserved by other hooks
  (DR0–DR2: ProcessGameEvents/RealRenderWorld/DspChat; DR3: server memcheck
  responder per issue #2175).  The detour writes a JMP stub into the function
  prologue and provides a trampoline for call-through to the original.  This
  matches the `hooks::fingerprint` pattern already in this crate.

- **Passthrough default:** `dispatcher_detour` always calls
  `IntegrityDispatcherHook.call(ctx)` so the original dispatcher runs in full.
  When `CACHE_ACTIVE` is set to `true`, future work in `dispatcher_detour` will
  skip the original call and substitute precomputed `FileHashCache` values.

- **PRNG binary cross-check** is deferred to a Frostreaver live session: read
  55 DWORDs at `rebase(DAT_140E8D148, actual_base)`, construct
  `LfgPrng::from_state`, advance 1 000 steps, compare against hook-logged
  values.  The `from_state_parity_1000_iterations` test structure is in place
  to hold those expected values once captured.

- `eqgame.exe` integrity check is a passthrough-stub: HWBP modifies only
  in-memory execution (no disk writes), so `GetModuleFileNameA`-based on-disk
  hash is unaffected.  If a future disk-resident shim is introduced, load a
  `FileHashCache` entry for `IntegrityFile::EqGameExe` and set
  `CACHE_ACTIVE = true`.
