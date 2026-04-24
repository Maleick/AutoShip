# Result: #2492 — tui(wizard): full-screen Clear wipes frame — use centered_popup + dim pass

Status: DONE

Changes Made:
- Updated the wizard overlay in `textquest/src/tui/ui/mod.rs` to use a centered 96x40 popup area.
- Added a dim pass over the underlying TUI buffer before clearing only the wizard popup rectangle.
- Rendered `WizardWidget` into the popup area instead of the full terminal frame.

Tests:
- `cargo check` passed.

Notes:
- Full `cargo test` and preflight were intentionally not run per issue instructions.

COMPLETE
