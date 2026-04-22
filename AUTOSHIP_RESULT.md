# Result: #1010 — Phase 1.1h: Add unit tests for Lua VM

## Status: COMPLETE

## Test delta: +19 new tests (33 → 52 Lua unit tests)

### New tests added

**sandbox.rs** (+3):
- `loadfile_is_blocked` — verifies `loadfile()` global is nil-ed out
- `package_is_blocked` — verifies `package.path` indexing fails
- `utf8_is_allowed` — verifies `utf8.len()` works in sandboxed VM

**bindings.rs** (+12):
- `new_does_not_panic` — LuaBindings::new() no-panic smoke
- `default_does_not_panic` — LuaBindings::default() no-panic smoke
- `player_api_all_fns_callable` — all 20 player API fns called from Lua
- `group_api_all_fns_callable` — all 6 group API fns called from Lua
- `nav_api_all_fns_callable_using_bracket_for_goto` — nav API with bracket notation for reserved `goto` keyword
- `combat_api_all_fns_callable` — all combat API fns called from Lua
- `state_api_all_fns_callable` — all state API fns called from Lua
- `config_api_all_fns_callable` — all config API fns called from Lua
- `log_api_all_fns_callable` — all log API fns (info/warn/error/debug) called
- `events_api_all_fns_callable` — all events API fns (on/off/emit) called
- `malformed_syntax_returns_error` — syntax error propagates as Err
- `runtime_error_in_script_returns_error` — runtime error propagates with message

**loader.rs** (+4):
- `test_syntax_error_transitions_to_error_state` — broken script → ScriptState::Error
- `test_runtime_error_transitions_to_error_state` — runtime error → ScriptState::Error
- `test_unload_memory_cleanup_smoke` — 10 load/unload cycles with large allocations stay within 64 MiB sandbox limit
- `test_rapid_load_unload_stress_100_cycles` — 100 load/unload cycles, no panic

## Notes

- Error budget (10 errors/60s → ScriptState::Error) is implemented in `textquest/src/plugins/mod.rs`, not the Lua loader — that feature has its own tests there. The loader error state is covered by direct error-path tests above.
- `nav['goto']` bracket notation confirmed required (Lua 5.4 reserved keyword).
- All 319 library tests pass; clippy reports no issues.
