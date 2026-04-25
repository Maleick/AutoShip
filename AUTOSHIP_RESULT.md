# Result: #1247 — Polish: Refactor common patterns and eliminate duplication

## Scope completed
- Added reusable JSON/TOML config load/save helpers in `textquest-common/src/persistence.rs`.
- Refactored `character_config` to use shared JSON config helpers.
- Refactored `chat_pattern_rules` to use shared TOML config helpers.
- Added focused persistence tests covering default-on-missing-file and round-trip behavior for both JSON and TOML helpers.

## Verification
- Ran `cargo check` successfully.

## Notes
- Refactor stays within the 1–3 file scope for production changes.
- Existing serialization behavior is preserved through round-trip tests and unchanged public APIs.
