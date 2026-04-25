# Result: #1107 — Load and render active camp center marker on map

Status: DONE

Changes Made:
- Made the map renderer use an explicit active camp overlay lookup from app map state.
- Rendered active camp center and pull point overlays through the camp overlay path.
- Added legend entries for camp center, pull point, camp radius, and pull radius symbols.
- Added focused map unit coverage for camp marker/radius rendering and the no-active-camp case.
- Updated an existing map test fixture to the current `CampOverlay` field shape.

Tests:
- `rtk cargo check` passed.
- `rtk cargo check --tests -p textquest` passed.
- `rtk git diff --check` passed.
- `rtk /verify` was attempted, but this Codex shell does not expose `/verify` as an executable command.

Notes:
- Per issue instructions, no `cargo test` run was performed.
- The existing camp-start command already populates `map_state.camp_overlay`, and camp stop clears it, so the map overlay follows the selected active camp state.

COMPLETE
