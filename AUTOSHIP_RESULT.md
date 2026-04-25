# AutoShip Result - Issue #981

Status: COMPLETE

## Summary

Implemented a minimal credential management dashboard backed by the existing account API shape.

Changes:
- Replaced mock credentials with live account loading, create, update, delete, and test actions.
- Added client-side form validation, secure password input, visibility toggle, delete confirmation, encrypted-storage confirmation, and credential availability status.
- Added character mapping fields for primary character, class, group, server, and account status.
- Added a local launch sequence editor with order controls and per-account stagger seconds.
- Updated the frontend API helper to handle 204 responses and expose status-aware API errors.
- Added a backend credential test status response and `POST /accounts/{name}/test` route for encrypted credential availability checks.

## Verification

Passed:
- `npm ci`
- `npm run build` from `textquest-web/frontend`
- `cargo test -p textquest-web credential_test_status_reflects_encrypted_password_availability`
- `cargo fmt --check -p textquest-web`
- `cargo clippy -p textquest-web --all-targets --all-features --no-deps -- -D warnings`

Repo-level checks attempted but blocked by existing unrelated failures:
- `cargo fmt --check` fails on formatting drift in `textquest/src/lua/bindings.rs` and `tools/etw-consumer/src/lib.rs`.
- `cargo clippy --all-targets --all-features -- -D warnings` fails on existing lints in `tools/etw-consumer/src/lib.rs`, `textquest-common/src/combat.rs`, and `textquest-common/src/inventory_utility.rs`.
- `cargo test` fails in existing `textquest` class-config tests because expected `target/debug/deps/config/classes/*.toml` files are missing.
- `cargo test -p textquest-web` fails in existing chat log, Discord, loot, and main route tests unrelated to this credential page.
- `python3 scripts/dev-preflight.py` reports the same repo-level fmt, clippy, cargo test, and Python workflow-contract failures.

## Notes

The frontend tries the requested `/api/credentials` endpoints first and falls back to the currently mounted `/api/accounts` endpoints when `/credentials` is not available. This keeps the UI compatible with the existing `textquest-web/src/accounts.rs` backend without expanding the issue beyond the three-file exploration limit.
