# Result: #2486 — tui(ch_chain): cast state colors bypass theme — thread &Theme into widget

Status: DONE

Changes Made:
- Added `cast_state_color(&CastState, &Theme) -> Color` in `textquest/src/tui/ui/widgets.rs`.
- Threaded `&Theme` into `ChChainWidget` and its render path.
- Updated the CH chain overlay call site to pass `app.theme`.
- Replaced CH chain cast, cast-bar, timing, and health visualization colors with theme semantics.

Tests:
- `rtk cargo check -p textquest --all-targets`

Notes:
- Full cargo tests were skipped per issue instructions.
- `textquest/src/tui/ui/ch_chain.rs` retains only `Color::Black`/`Color::Reset` literals for selected-row contrast/reset behavior.

COMPLETE
