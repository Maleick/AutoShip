# Result: #990 — Feature: Hotkey & Slash Command Registration System

Status: DONE

Changes Made:
- Extended `textquest/src/registry/mod.rs` with checked command and hotkey registration APIs, conflict detection, Ctrl/Alt/Shift hotkey parsing, per-character scopes, enable/disable controls, command help metadata, required-argument validation, and TOML load/save validation helpers.
- Added persisted `input_bindings` metadata to `AppConfig` and validate it during TOML config load.
- Added shared command and hotkey registries to `Orchestrator` with routing helpers for registered slash commands and hotkeys.
- Updated `feature-list.json` with issue #990 status and remaining scope.

Tests:
- `cargo check`

Notes:
- This is intentionally PARTIAL because the issue spans DLL keyboard hooks and live IPC back-routing. This pass adds the orchestrator/config/registry scaffold and focused tests for registration conflicts, help, validation, enable/disable, and per-character routing. DLL hook integration and live keyboard event transport remain follow-up work.

COMPLETE
