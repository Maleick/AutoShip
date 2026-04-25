# Result: #2426 — feat(backend): implement /api/raid/config for web UI

## Scope completed
- Implemented `GET /api/raid/config` and `PUT /api/raid/config` in `textquest-web/src/api.rs` using a persisted `RaidConfig` schema.
- Added production routing for raid config in `textquest-web/src/main.rs` and wired state fields for in-memory + disk-backed config.
- Added startup loading and test-state loading for `raid_config` using a dedicated `config/raid-config.toml` path.
- Updated existing API test expectation for `/api/raid/config` now to expect `200 OK`.

## Verification
- Ran `cargo check` after implementation and route/state updates.
