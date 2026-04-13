# TextQuest Performance Baselines

This document defines the target performance baselines for the TextQuest build and test pipeline.
All timings are measured on a modern developer workstation (e.g., Frostreaver mini PC).

## Target Baselines

| Metric                       | Target       | Notes                                                   |
| ---------------------------- | ------------ | ------------------------------------------------------- |
| Debug build (`cargo build`)  | < 30 seconds | Clean incremental build on a warm cache                 |
| Release build                | < 90 seconds | Requires nightly MSVC on Windows; not measured on macOS |
| Incremental recompile        | < 10 seconds | Single-file change, no dep graph invalidation           |
| Unit tests (textquest crate) | < 2 minutes  | `cargo test -p textquest`                               |
| Full test suite              | < 10 minutes | `cargo test` across all four workspace crates           |

## Measurement Script

Use `scripts/measure-baselines.sh` to capture actual timings and append them to `.autoship/baselines.json`.

```bash
bash scripts/measure-baselines.sh
```

The script outputs a JSON object to stdout and appends it to `.autoship/baselines.json`:

```json
{
  "debug_build_s": 18,
  "unit_tests_s": 45,
  "full_suite_s": 320,
  "timestamp": "2026-04-13T12:00:00Z"
}
```

## Interpreting Results

- Numbers **below** target: healthy, no action needed.
- Numbers **within 20% of** target: monitor for regression.
- Numbers **above** target: investigate — common causes are dep graph growth, new proc-macros, or test suite bloat.

## History

Baseline history is stored in `.autoship/baselines.json` as a JSON array. Each run appends one entry.
Review the file periodically to catch gradual regressions before they become CI bottlenecks.
