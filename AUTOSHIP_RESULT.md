# Result: #2490 — tui(toast): no dismiss timer / auto-expire progress bar

- Kept `/melody` fallback behavior for low-song configs.
- Made `TwistEngine` hold mode persistent until explicit release and added hold-suspension behavior while the held song is cooling down.
- Routed `CastResult::Recovering` through bard cast outcome handling so interrupted songs are re-queued for twist recovery.
- Updated bard/tests and twist tests to reflect persistent hold/unhold behavior and recovering outcome recovery.

Changes Made:
- Added `expires_at: Instant` to the active TUI toast state and refresh it when duplicate toast messages are renewed.
- Updated toast expiry clearing to honor wall-clock expiration while preserving the existing tick-based fallback.
- Rendered a bottom-line `█▓░` progress bar in the toast panel based on remaining time until `expires_at`.

Tests:
- `rustfmt --check textquest/src/tui/app.rs textquest/src/tui/ui/mod.rs` passed.
- `cargo check` passed.
- Full `cargo fmt --check` was not clean due pre-existing unrelated formatting drift outside this issue scope.

Notes:
- Full cargo tests were intentionally skipped per issue instructions.

COMPLETE
