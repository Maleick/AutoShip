# Performance Targets and Benchmarking

This document defines baseline performance targets for critical multibox operations. These targets guide optimization efforts and help detect performance regressions during development.

## Running Benchmarks

To run the full benchmark suite:

```bash
cargo bench --benches
```

To save a baseline for tracking:

```bash
cargo bench --benches -- --save-baseline main
```

To run a baseline comparison against `main`:

```bash
cargo bench --benches -- --baseline-lenient main --save-baseline current
```

To run a specific benchmark:

```bash
cargo bench --bench ipc_serialization
cargo bench --bench spawninfo_reads
cargo bench --bench config_parsing
```

To save results with HTML reports:

```bash
cargo bench --benches -- --verbose
```

Criterion will generate an HTML report in `target/criterion/` with charts and statistical analysis.

## Baseline Targets

### IPC Message Serialization

**Purpose**: IPC is the backbone of the orchestrator ↔ client communication. Serialization overhead directly impacts command latency and state frame throughput.

| Operation                            | Target Latency | Notes                          |
| ------------------------------------ | -------------- | ------------------------------ |
| IPC command encode/decode roundtrip  | < 10 µs        | bincode binary format          |
| IPC response encode/decode roundtrip | < 10 µs        | bincode binary format          |
| Message frame encode (1 KB)          | < 5 µs         | Frame header + magic + version |
| Message frame decode (1 KB)          | < 5 µs         | Frame validation + CRC check   |
| Message frame encode (5 KB)          | < 20 µs        | Larger payload                 |
| Message frame decode (5 KB)          | < 20 µs        | Larger payload                 |

**Rationale**:

- The server tick is 6 seconds; state frames are sent every frame on a 60 Hz client, resulting in ~16 ms between frames.
- At 36 clients, serialization should consume < 1% of frame time per client (< 160 µs per command).
- These targets ensure sub-millisecond latency for individual IPC operations.

### SpawnInfo Field-by-Field Reads

**Purpose**: SpawnInfo is populated via sequential field reads (`proc.read::<T>(addr + OFFSET)` calls). Each read is a Windows API call; batch performance is critical for responsive spawn list updates.

| Operation                         | Target Latency | Notes                                         |
| --------------------------------- | -------------- | --------------------------------------------- |
| Single field read simulation      | < 1 µs         | Represents one proc.read::<T>() call overhead |
| Full SpawnInfo batch (~30 fields) | < 50 µs        | Complete spawn read cycle                     |
| Spawn list update (36 spawns)     | < 2 ms         | 36 × 50 µs + overhead                         |

**Rationale**:

- Each `proc.read::<T>()` is a Windows debug read API call with scheduler overhead.
- Realistic spawn reads touch 20-30 fields per spawn (name, level, position, type, status, etc.).
- Target is to keep spawn list updates under 2 ms so the TUI remains responsive at 60 Hz.

### Config Parsing (TOML)

**Purpose**: Config files are loaded at startup and occasionally reloaded. Parsing performance affects launch time and hot-reload responsiveness.

| Operation                              | Target Latency | Notes                           |
| -------------------------------------- | -------------- | ------------------------------- |
| Small accounts.toml (3 accounts)       | < 100 µs       | Cold parse from string          |
| Large accounts.toml (36 accounts)      | < 500 µs       | Typical multibox setup          |
| Field access after parse               | < 5 µs         | Lookup by group, hotkey, name   |
| Config reload cycle (parse + validate) | < 10 ms        | Full reload with all operations |

**Rationale**:

- TOML parsing is linear in file size; expect O(n) behavior.
- Startup is not performance-critical (happens once per session), but responsiveness matters for live reloads.
- Field access should be O(log n) for group lookups; targets reflect in-memory table operations.

## Regression Detection

Benchmarks run in CI on PRs to `master`, on `master` pushes, and via manual dispatch (advisory, not blocking). PR runs also compare against the merge-target baseline when available.

If a benchmark degrades by > 10% from baseline:

1. **Check recent changes**: Review PRs merged since the last baseline measurement.
2. **Identify the culprit**: Use `git bisect` with the benchmark to pinpoint the change.
3. **Analyze the regression**: Profile with `perf` (Linux) or Instruments (macOS) to find the hot spot.
4. **File an issue**: Link to benchmark results and create a task to fix the regression.

## Adding New Benchmarks

When adding a new critical operation:

1. Create a benchmark in `benches/*.rs` using `criterion`.
2. Define a target latency in this document.
3. Document the rationale (why this latency matters).
4. Ensure the benchmark is deterministic (no I/O, no random timing).
5. Run `cargo bench` locally to establish a baseline.

## Measurement Methodology

- **Harness**: Criterion.rs with statistical analysis (mean, std deviation, confidence intervals).
- **Iterations**: Criterion auto-tunes iteration count; each benchmark runs at least 100 samples.
- **Warmup**: Criterion includes automatic warmup before measurement.
- **Output**: HTML reports with trend charts stored in `target/criterion/`.

## Known Limitations

- Benchmarks run on the developer's machine; absolute latencies vary by hardware.
- Windows API overhead (proc.read) is simulated in `spawninfo_reads` but not measured directly without a live EQ client.
- TOML parsing is measured in isolation; real config loads may include file I/O, which is excluded.

## Future Enhancements

- Add benchmarks for spawn filter queries (e.g., "all mages in group 1").
- Add benchmarks for Discord webhook serialization/sending.
- Add benchmarks for navmesh pathfinding (A\* overhead).
- Run benchmarks on CI with hardware-normalized baseline comparisons.
- Add memory allocation profiling to detect unexpected allocations in hot paths.
