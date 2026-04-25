# Map Performance Baseline

This page defines the profiling surface for TUI map rendering and the threshold
used to detect regressions in large geometry sets.

## Trace Enablement

Set the shared performance trace environment variable before launching the TUI:

```bash
TEXTQUEST_PERF_TRACE=1 cargo run -p textquest
```

Map rendering emits `textquest::perf` events with `span` set to:

| Span | Scope |
| --- | --- |
| `bounds_calculation` | Combined zone, navmesh, and spawn bounds selection. |
| `transform_calculation` | Viewport sizing, active view mode, and map-to-grid transform. |
| `geometry_rendering` | Zone map and navmesh line projection and painting. |
| `spawn_rendering` | Spawn cache rebuild, spawn marker painting, and spawn labels. |
| `overlay_rendering` | Paths, target line, player marker, radius, camp, named, highlight, legend, and compass overlays. |
| `minimap_rendering` | Minimap projection and widget render. |
| `map_render_total` | End-to-end `draw_map_view` wall time. |

Each event includes `zone`, `width`, `height`, and `elapsed_ms`.

## Baseline Capture Matrix

Capture at least 10 frames for each condition:

| Zone | Target geometry set | Zoom levels | Spawn load | Layers |
| --- | ---: | --- | ---: | --- |
| Permafrost | 125K+ lines | 0.5x, 1x, 2x, 4x | 100+ visible | all enabled |
| ECommons | 60K+ lines | 0.5x, 1x, 2x, 4x | 100+ visible | all enabled |
| East Wastes | 10K+ lines | 0.5x, 1x, 2x, 4x | 100+ visible | all enabled |

Repeat each zoom level at center, near-edge, and dense-geometry pan positions.
Report median, minimum, and maximum `elapsed_ms` for each span.

## Current Repo Data

The checked-in sample files in this worktree are not the large target geometry
sets from the issue:

| File | Lines |
| --- | ---: |
| `config/maps/permafrost.txt` | 2,076 |
| `config/maps/permafrost_1.txt` | 34 |
| `config/maps/ecommons.txt` | 1,013 |
| `config/maps/ecommons_1.txt` | 15 |
| `config/maps/eastwastes.txt` | 166 |

Because the large Brewall/MQ2 map corpus is not present here, the issue #1125
large-zone timing table remains pending. Do not treat timings from the sample
files above as the production performance baseline.

## Regression Threshold

For the large-zone matrix, the baseline is acceptable when:

| Metric | Target |
| --- | ---: |
| `map_render_total` median | <= 50 ms |
| `map_render_total` max | <= 75 ms |
| `geometry_rendering` cached median | <= 30 ms |
| `spawn_rendering` median with 100+ visible spawns | <= 8 ms |
| `overlay_rendering` median with all overlays enabled | <= 8 ms |
| `minimap_rendering` median | <= 4 ms |
| `bounds_calculation` + `transform_calculation` median | <= 4 ms combined |

A change is a regression if any span median increases by more than 20 percent
against the recorded baseline for the same zone, zoom, pan, terminal size, and
spawn load, or if `map_render_total` exceeds the 50 ms median target.

## Reporting Template

Use this table format in the PR or follow-up issue after collecting live
samples:

| Zone | Zoom | Pan | Span | Min ms | Median ms | Max ms | Notes |
| --- | ---: | --- | --- | ---: | ---: | ---: | --- |
| permafrost | 1x | center | `map_render_total` | TBD | TBD | TBD | 100+ spawns, all layers |

## Future Optimization Candidates

- Viewport culling for line segments before projection.
- Spawn marker cache reuse across pan positions that do not change visibility.
- Layer caches keyed by zoom bucket instead of exact floating-point scale.
- Geometry pre-processing for large static zone files.
