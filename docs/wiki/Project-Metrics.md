# Project Metrics

Current workspace totals: 166,532 Rust lines and ~3,990 tests. This page and the README badges are auto-refreshed by `scripts/update_readme_metrics.py`.

## What This Tracks

- tracked Rust source lines across the workspace
- exact test count from `cargo test --workspace -- --list` when Cargo is available
- source scan fallback for the test count when the cargo runner is unavailable

## Refresh Flow

```mermaid
flowchart LR
    A[Tracked Rust files] --> B[scripts/update_readme_metrics.py]
    B --> C[README badges]
    B --> D[Project Metrics page]
    B --> E[GitHub Actions push + nightly]
```

## Notes

- The metrics workflow updates the README and this page from the same generator.
- If the cargo test listing fails, the script surfaces the error instead of silently inventing a value.
- The public docs site shows the same numbers that land in the repo README.
