# Result: #1139 — Sub: Core Plugin API Surface (#819-3)
Status: PARTIAL

- Implemented Lua core plugin API surface wiring in `textquest/src/lua/bindings.rs` for combat, nav, state, IPC, config, events, and logging integrations.
- Added runtime-backed plugin-facing data model fields in `textquest/src/lua/types.rs` to support request metadata, group/state/nav/cache snapshots.
- Updated command request API shape to `command`, `target_box`, and `via_ipc`; updated queue callers and tests for metadata assertions.
- Added doc examples and an error-handling example to `docs/wiki/Plugin-System.md`.
- Added example script coverage at `scripts/lua/examples/06_textquest_api_smoke.lua`.
- Updated `feature-list.json` with `issue-1139-sub-core-plugin-api-surface` as `complete`.
- Validation: `cargo check --package textquest`, `cargo check`.
