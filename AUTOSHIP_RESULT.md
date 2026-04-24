# AutoShip Result — PR #2416 wiki-gate fixer

## Status
COMPLETE (awaiting CI)

## Actions
- Added `docs/wiki/Derived-Struct-Offsets-From-Bytecode.md` summarizing the
  `ExtractDisplacement` resolve mode (satisfies CI wiki-update gate).
- Sign-extended displacement extraction in `scan_engine::resolve_displacement`
  and rejected negative struct-field offsets (codex P2).
- Clarified `pattern_db::ScanEntry::{resolve,expected_preferred}` doc comments
  to describe the address-vs-offset duality.
- Assigned `OffsetCategory::PlayerZoneField` explicitly on the two
  `ExtractDisplacement` tests so DB-merge routing is covered.

## Verification
- `cargo check -p textquest-common` — clean.
- `cargo test -p textquest-common --lib scan_engine` — 41 passed.
- Commit `pushed` on `autoship/issue-749`; CI re-running.

## Inline comments addressed
- 3135108299 (docs on ScanEntry fields) — updated.
- 3135108320 (missing wiki doc) — added wiki page.
- 3135108335, 3135108350 (explicit test category) — applied.
- 3135113768 (codex P2 sign-extend) — applied as signed + reject-negative.

DO NOT merge.
