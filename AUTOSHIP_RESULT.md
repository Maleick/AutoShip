# Result: #1125 — Profile map rendering with large geometry sets

Status: PARTIAL

## Changes Made

- Added `TEXTQUEST_PERF_TRACE`-gated map render timing spans in `textquest/src/tui/ui/map.rs`:
  - `bounds_calculation`
  - `transform_calculation`
  - `geometry_rendering`
  - `spawn_rendering`
  - `overlay_rendering`
  - `minimap_rendering`
  - `map_render_total`
- Added focused unit coverage locking the required issue #1125 span labels.
- Added `docs/wiki/map-performance-baseline.md` with trace usage, capture matrix, acceptable ranges, regression threshold, and reporting template.
- Updated `feature-list.json` to track the partial state and remaining live measurement work.

## Tests

- `cargo check -p textquest` passed.
- `~/.Codex/bin/verify` passed 7/7 rules.
- Unit test added but not run because the issue instructions required cargo check only.

## Notes

- The checked-in map samples are not the large geometry sets requested by the issue: Permafrost is about 2.1K checked-in lines, ECommons about 1.0K, and East Wastes 166.
- Remaining acceptance work requires collecting 10+ live timing samples per condition for the large Permafrost, ECommons, and East Wastes geometry sets with 100+ visible spawns.

COMPLETE
