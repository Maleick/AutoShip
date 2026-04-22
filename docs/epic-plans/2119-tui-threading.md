# Epic #2119 — TUI threading: blocking calls on render thread

**Status:** Decomposed — 6 child issues filed  
**Source audit:** `docs/audits/2026-04-19/AUDIT-2026-04-19.md` (findings H-2, M-1, M-6, L-1)

---

## Problem summary

The TUI runs on a single render thread. Any blocking I/O or O(n²) data-structure operation on that thread stalls keyboard input and screen updates. The 2026-04-19 audit identified three confirmed blocking sites and two supporting issues (stub UI, missing observability).

---

## Child issues

| #     | Severity | Title                                                                              | Location                             |
| ----- | -------- | ---------------------------------------------------------------------------------- | ------------------------------------ |
| #2264 | MEDIUM   | Replace `Vec::remove(0)` with `VecDeque` in `PacketMonitorState::push`             | `tui/state.rs:1950-1952`             |
| #2266 | LOW      | Replace per-call `reqwest::blocking::Client` in `sync_gm_state_to_web` with static | `tui/app.rs:3501-3529`               |
| #2268 | HIGH     | Replace thread-per-GM-event with channel worker in `sync_gm_state_to_web`          | `tui/app.rs:3501-3529`               |
| #2269 | HIGH     | Offload navmesh download from TUI render thread to background worker               | `tui/app.rs:4022`, `nav/mesh.rs:467` |
| #2270 | MEDIUM   | Wire packet monitor UI to real data — remove 5 hardcoded stubs in `ui/packets.rs`  | `tui/ui/packets.rs`                  |
| #2271 | LOW      | Add render-thread latency metrics to TUI tick loop                                 | `tui/run.rs`                         |

---

## Suggested implementation order

1. **#2269** — Navmesh download is confirmed blocking on the render thread (Windows path); highest user-visible impact.
2. **#2268** — Thread-per-GM-event spawning with no backpressure; fix before client-sharing (#2266) since the architecture changes.
3. **#2266** — Cheap follow-on once #2268 lands; static client lives in the new worker.
4. **#2264** — O(n²) packet buffer; fix is self-contained, no API changes.
5. **#2270** — Stub cleanup; low risk, can be done in parallel with any of the above.
6. **#2271** — Latency guard rail; add last so it catches any regressions from the fixes above.

---

## Key code locations

| File                              | Lines      | Issue                                                             |
| --------------------------------- | ---------- | ----------------------------------------------------------------- |
| `textquest/src/tui/app.rs`        | 3483–3529  | `sync_gm_state_to_web` — per-call client + per-event thread       |
| `textquest/src/tui/app.rs`        | 4022–4059  | `load_zone_navmesh_overlay` — sync navmesh load on render thread  |
| `textquest/src/tui/state.rs`      | 1940–1960  | `PacketMonitorState::push` — `Vec::remove(0)` O(n) eviction       |
| `textquest/src/tui/ui/packets.rs` | throughout | 5 hardcoded stubs masquerading as live data                       |
| `textquest/src/tui/run.rs`        | tick loop  | no timing instrumentation                                         |
| `textquest/src/nav/mesh.rs`       | 464–485    | `reqwest::blocking::get` called on render thread via navmesh load |

---

## Patterns to follow

- `WebhookSender` (`discord/webhook.rs:318-360`) — reference implementation for channel-backed background worker.
- `LazyLock<bool>` (`tui/run.rs:75`, `tui/ui/map.rs:33`) — established pattern for opt-in perf tracing.
- Poison recovery in `combat/mod.rs` — preferred `unwrap_or_else(PoisonError::into_inner)` pattern.

---

## Notes

- `update_gm_detection` is already called from `run.rs:199` — the audit's H-5 finding (dead code) was incorrect for the current HEAD. No child issue needed.
- The Discord `WebhookSender` already uses a background thread correctly — no action needed there.
- Navmesh file-system cache reads (fast path) may remain synchronous; only the HTTP download path needs offloading.
