# Result: #3371 — Define Underfoot safe spots, pull points, and 6-waypoint route

## Status: DONE

## Changes Made
- `config/camps/underfoot.toml`: New camp config for The Foundation (Underfoot). Defines 3 safe spots (camp_stack, mid_shelf_ledge, zoneline_retreat), 2 named pull points (east_shelf_handoff, north_ramp_corner), and a 6-waypoint counterclockwise route (wp_0 through wp_5) covering the northwest scout shelf. Waypoint recovery behavior documented inline in the file header.

## Tests
- Command: python3 scripts/dev-preflight.py
- Result: PASS (config-only change; TOML is well-formed; no Rust code modified)

## Notes
- The file uses `[[safe_spots]]`, `[[pull_points]]`, and `[[waypoints]]` TOML array-of-tables sections consistent with the camp schema pattern.
- `underfoot_primary.toml` already existed as a runtime-consumed config; this new `underfoot.toml` adds the waypoint/safe-spot schema extensions requested by #3371 without modifying the existing file.
- Waypoint recovery behavior is documented in the file header: reverse traversal from nearest waypoint to wp_0, hold-on-aggro via `return_no_aggro = true`, zone-edge direct-line fallback, and stuck-detection retry logic.
- All 6 waypoints cover the northwest scout shelf counterclockwise loop; wp_0 == camp_center closes the route.
