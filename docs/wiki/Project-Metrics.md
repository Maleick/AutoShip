# Project Metrics

Current workspace totals: 285,998 Rust lines, 4,163 exact tests, and 8 workspace crates. Latest release: v0.7.0-alpha. This page and the README badges are auto-refreshed by `scripts/update_readme_metrics.py`.

## What This Tracks

- tracked Rust source lines across the workspace
- exact test count from `cargo test --workspace -- --list` when Cargo is available
- workspace crate count from the root `Cargo.toml`
- newest semver-style release tag and source commit badge values
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
