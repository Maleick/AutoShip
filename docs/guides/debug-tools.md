# Debug Tools Guide

> **Phase 4.3d** — Reference for TextQuest's built-in debug tooling.
> Parent epic: [#803](https://github.com/Maleick/TextQuest/issues/803)

---

## Overview

TextQuest ships several debug facilities that help you inspect EQ memory offsets,
trace client-session health, and troubleshoot automation failures without
attaching an external debugger. All tools are accessible from the running TUI
and from unit/integration tests.

| Tool | Access | Purpose |
|------|--------|---------|
| **EQ Internals panel** | TUI `Debug` screen | Browse compiled EQ memory offsets |
| **Hex-dump panel** | TUI `addr` command | Inspect raw memory at an arbitrary address |
| **Admin monitoring store** | Library API | IPC latency, memory usage, error rates per client |
| **`sample_process_memory_bytes`** | Library API | Resident-memory probe for a live client PID |
| **Structured tracing** | Log files | Correlated `tracing` events written to rolling log |

---

## EQ Internals Panel

The EQ Internals panel is a scrollable offset browser available on the `Debug`
screen. It reads offsets from `OffsetDatabase::from_compiled_offsets()` once at
startup and supports category filtering and text search.

### Keybindings

| Key | Action |
|-----|--------|
| `↑` / `↓` | Scroll through entries |
| `Enter` | Jump hex-dump panel to the selected address |
| `c` | Cycle category filter (All → Globals → PlayerBase → PlayerZone → SpawnMgr → Functions → All) |
| `/` | Enter search mode; type substring to filter by name |
| `Esc` | Exit search mode |

### Categories

| Category | Contents |
|----------|----------|
| `Globals` | Pointer addresses (e.g. `pinstLocalPlayer`) |
| `PlayerBase` | Player struct field offsets |
| `PlayerZone` | Zone-related player fields |
| `SpawnManager` | Spawn list / spawn count offsets |
| `Functions` | Callable EQ function addresses |

### Usage example

```
# Navigate to the Debug screen then type in the TUI command bar:
addr 0x140A3B210
# The hex-dump panel scrolls to that address.
```

---

## Hex-Dump Panel (`addr` command)

The `addr` command sets the target address in the Debug panel's hex-dump viewer
and triggers an immediate `ReadMemory` poll.

```
addr <hex_address>
```

**Example:**

```
addr 0x00A3B210
```

This is available only when a client connection is active (Windows runtime).
On macOS the panel renders but memory reads are stubbed.

---

## Admin Monitoring Store

`AdminMonitoringStore` (in `textquest/src/metrics/admin_monitoring.rs`) tracks
per-session IPC latency, process memory, and error rates. It is designed to be
queried for operator dashboards and automated health checks.

### Key types

| Type | Role |
|------|------|
| `AdminMonitoringStore` | Central store; create with `with_retention()` |
| `AdminMonitoringRetention` | Configures rolling-window sizes |
| `SessionMonitoringSnapshot` | Point-in-time read of one session's metrics |
| `MonitoredSessionState` | `Active` / `Exited` / `Absent` |

### Lifecycle

```rust
let retention = AdminMonitoringRetention { ipc_samples: 60, memory_samples: 30, error_events: 100 };
let mut store = AdminMonitoringStore::with_retention(retention);

// Register a client by internal ID and OS PID.
store.register_session(client_id, pid);

// Record observations.
store.record_ipc_latency(client_id, latency_ms);
store.record_memory_sample(client_id, bytes);
store.record_error(client_id, error_kind);

// Read a point-in-time snapshot.
let snapshot = store.snapshot(client_id, Instant::now());
```

### Memory inspection

The `memory` field of `SessionMonitoringSnapshot` exposes:

| Field | Meaning |
|-------|---------|
| `min_bytes` | Minimum resident memory across the retention window |
| `max_bytes` | Maximum resident memory across the retention window |
| `avg_bytes` | Mean resident memory across the retention window |

All fields are `Option<u64>` — `None` until at least one sample has been recorded.

### IPC latency

The `ipc_latency` field exposes P50/P95 latency in milliseconds:

```rust
snapshot.ipc_latency.p50_ms  // Option<u64>
snapshot.ipc_latency.p95_ms  // Option<u64>
```

---

## Process Memory Sampling

`sample_process_memory_bytes(pid: u32) -> Option<u64>` probes resident memory
for any running process by PID.

| Platform | Implementation |
|----------|---------------|
| Linux | Reads `VmRSS` from `/proc/<pid>/status` |
| Windows | Calls `GetProcessMemoryInfo` via the Win32 API |
| macOS / other | Always returns `None` (stub) |

**Usage:**

```rust
use textquest::metrics::sample_process_memory_bytes;

if let Some(bytes) = sample_process_memory_bytes(client_pid) {
    tracing::info!(pid = client_pid, bytes, "client memory sample");
}
```

---

## Structured Tracing

TextQuest writes structured `tracing` events to a daily-rolling log file at
`<log_dir>/textquest-dump.log`. The default filter is `debug`, capturing all
spans and events at debug level and above.

### Enabling verbose tracing

Set `RUST_LOG` before launching to override the default filter:

```bash
RUST_LOG=textquest=trace,textquest_common=debug ./textquest
```

Common useful filters:

| Filter | Effect |
|--------|--------|
| `debug` (default) | All debug + info + warn + error events |
| `trace` | Adds low-level frame/tick events (very verbose) |
| `textquest_dll=debug` | DLL-side events only |
| `textquest::metrics=trace` | Metrics subsystem trace |

### Log location

Logs are written to the directory specified by `--log-dir` (default: `logs/`
adjacent to the binary). Files rotate daily and the 7 most recent files are
kept.

---

## Troubleshooting Guide

### Client shows as `Absent` in monitoring

1. Check the process is still running: `ps aux | grep eq`.
2. Verify IPC channel is open — look for `IpcDisconnected` events in the log.
3. Call `mark_session_exited()` then re-register to reset state.

### Offset browser shows wrong values after an EQ patch

EQ patches (typically Wednesdays) may relocate structures. After a patch:

1. Run `data/` offset rescan scripts.
2. Rebuild the workspace: `cargo build`.
3. The EQ Internals panel reads `OffsetDatabase::from_compiled_offsets()` at
   startup, so a fresh binary picks up the updated offsets automatically.

### Hex-dump panel is blank

- Confirm a client is connected and the IPC channel is healthy.
- Use `addr 0x<address>` to manually set a known-good address.
- On macOS the panel always renders empty (no live EQ process).

### Tracing log is not growing

- Confirm the log directory is writable: `ls -la logs/`.
- Check that the `RUST_LOG` filter is not set to `error` (which would suppress
  debug/info events).
- Look for the `tracing initialized` info event at process startup.

### Memory samples show `None`

`sample_process_memory_bytes` returns `None` on macOS. On Linux or Windows, a
`None` return usually means the target PID no longer exists. Re-check that the
client is still running before recording additional samples.
