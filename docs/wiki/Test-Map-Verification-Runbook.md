# Test Map Verification Runbook

Use this runbook on patch day to verify that Tactical map discovery, zone-map loading, spawn overlay rendering, and map controls still behave correctly in Test and demo mode.

## What Worked

- `config/maps` discovery probes CWD ancestors, executable ancestors, and macOS bundle-style `Contents/Resources/config/maps`.
- `permafrost` loads as the known-good map baseline in the current repo state.
- Tactical map status now distinguishes loaded geometry from point-only overlays that have no drawable linework.
- Tactical map status now shows explicit banners for hidden geometry, no drawable geometry, and unavailable navmesh data.
- Tactical `r` remains the primary reset key.
- `[` and `]` remain reserved for previous/next client navigation.

## What Failed

- `crescent` map assets are missing from the current `config/maps` directory.
- Demo bounds clamping on Mac cannot use map bounds for zones that do not have loadable map data.
- Zones that resolve to `Unknown` still indicate a real lookup problem and should not be treated as valid map input.

## Repro: Test

1. Start the Test build on Windows with a live client attached.
2. Select a client in `Permafrost`.
3. Open Tactical view.
4. Confirm the map header shows the active zone and a non-empty load status.
5. Confirm geometry, spawn markers, and any overlay data render together.
6. Press `+` and `-` and confirm zoom changes are reflected in the header/status.
7. Press `<` and `>` and confirm the depth slice changes are reflected in the header/status.
8. Press `r` and confirm the Tactical map resets without requiring `Home`.
9. Toggle navmesh with `n` and confirm the UI reports either loaded navmesh data or `mesh:n/a`.

## Repro: Mac Demo

1. Launch the TUI on macOS with no live EQ clients attached so demo mode loads automatically.
2. Select a demo client whose zone is `Permafrost` or another mapped zone.
3. Open Tactical view.
4. Confirm the map is clamped to the loaded demo bounds when map data is available.
5. Confirm the map status makes missing data explicit if a zone has no map files.
6. Repeat the same `+`, `-`, `<`, `>`, and `r` checks as Test.

## Commands

```bash
cargo fmt --all
cargo build -p textquest
cargo test -p textquest -- --nocapture
```

For a local TUI smoke test on macOS or Linux:

```bash
cargo run -p textquest -- tui
```

## Pass Criteria

- `Permafrost` shows geometry and spawns in Test and demo mode.
- The Tactical map header reports the current zoom and depth state after user input.
- `r` resets the Tactical map view and prints an explicit reset confirmation.
- `[` and `]` continue to switch clients globally.
- Missing map files are reported as missing instead of rendering a silent partial map.
- Navmesh availability is explicit when enabled or unavailable.

## Fail Criteria

- The zone displays `Unknown` when a resolvable zone name is available.
- The Tactical map silently falls back to a partial outline with no diagnostic status.
- `r` does not reset the Tactical map view.
- `[` or `]` are repurposed for map actions.
- Demo mode on Mac stops loading demo map data or silently skips missing-bounds cases.

## Patch-Day Notes

- If `crescent` or another expected zone is missing, record the missing filename(s) before sign-off.
- Treat point-only map files differently from true no-data cases.
- Use the on-screen Tactical status line as the operator-facing source of truth for zoom, depth, geometry, and navmesh state.
