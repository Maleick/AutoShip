# Result: #991 — Feature: Audio Alert System & Event Routing

Status: PARTIAL

Changes Made:
- Added `textquest-common::audio_alerts` with alert kinds, TOML-backed rule config, priorities, thresholds, channel fan-out, custom sound paths, and event-to-playback request routing.
- Added a nonblocking registry dispatch method backed by an `AudioAlertBackend` trait.
- Added a Windows `PlaySoundW` system backend scaffold with fallback beep behavior and a non-Windows no-op logging fallback.
- Wired audio alert settings into the existing `AlertingConfig` so they persist under TOML config.
- Added focused unit coverage for low-mana threshold routing, custom alert matching, channel fan-out, custom FLAC paths, and volume clamping.
- Updated `feature-list.json` to record the partial issue state.

Tests:
- `rustfmt --edition 2024 textquest-common/src/audio_alerts.rs textquest-common/src/lib.rs textquest/src/config.rs`
- `cargo check`
- `cargo check -p textquest-common --tests`

Notes:
- Partial by scope: live event producers, dashboard UI, bundled default sounds, true per-sound volume control, MP3/FLAC decoder-backed playback, and end-to-end runtime integration remain follow-up work.

COMPLETE
