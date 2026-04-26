# Live Test Spec — Encrypt-Owner Mutex (Issue #3408)

## Purpose

Validate that the `ENCRYPT_OWNER` `AtomicU8` correctly prevents the per-frame
and timer-queue encrypt/decrypt paths from overlapping under real EQ client
load.

## Background

Issue #3408 adds a shared owner-flag (`ENCRYPT_OWNER`) to `timer_queue_sleep`
that both the per-frame path (`stealth::wake` / `stealth::sleep`) and the
timer-queue callback must win before touching `.text`.  The existing unit tests
cover the CAS logic and early-exit paths.  This spec covers live-client
validation.

---

## Test 1 — Login Screen Idle (Encrypted Fraction)

**Scenario:** Client sits at the EQ login screen for 5 minutes with no user
input and no zone loaded.

**Expected outcome:** The timer-queue path fires every 1 500 ms; the per-frame
path is inactive (EQ is not ticking).  The code section should be encrypted for
≥ 95 % of the window.

**Measurement:**

```
# In the DLL trace log (tracing::trace! at INFO level or above), count:
# - "timer_queue_sleep: skipping — ENCRYPT_OWNER held by frame path"  → should be 0
# - timer_callback encrypt cycles fired   → expect ~200 in 5 min
# - timer_callback decrypt cycles fired   → expect ~200 in 5 min
```

**Pass criteria:**

- `encrypted_fraction = encrypted_ticks / total_elapsed_ticks ≥ 0.95`
- Zero `ENCRYPT_OWNER` collision log lines from either path
- Client remains stable (no crash, no freeze, no disconnect)

---

## Test 2 — Concurrent Path Stress (No Deadlock)

**Scenario:** Force both paths to fire simultaneously by lowering the timer
interval to 50 ms and injecting rapid per-frame ticks via the test harness.

**Platform:** Windows only (Frostreaver); macOS stubs are no-ops.

**Steps:**

1. Build with `RUST_LOG=trace` and `timer_interval_ms = 50`.
2. Run the EQ client to the character select screen.
3. Let the stress run for 60 seconds.
4. Capture the DLL trace log.

**Pass criteria:**

- No deadlock (process remains responsive throughout).
- No panic or `unwrap()` failure in the DLL log.
- `ENCRYPT_OWNER` collision lines (`skipping — ENCRYPT_OWNER held by …`) appear
  but are ≤ 10 % of total cycle attempts, confirming contention is handled
  gracefully.
- `CODE_ENCRYPTED` never reaches an inconsistent state (no double-encrypt or
  double-decrypt errors in the log).

---

## Test 3 — Shutdown Safety

**Scenario:** Call `timer_queue_sleep::shutdown()` while a timer callback is
mid-flight.

**Steps:**

1. Set timer interval to 100 ms.
2. While a callback is executing (observe via trace log), call `shutdown()`.
3. Verify `ENCRYPT_OWNER` is `None` (0) after shutdown completes.
4. Verify `.text` is decrypted (page permissions restored to RX).

**Pass criteria:**

- `shutdown()` blocks until the in-flight callback finishes.
- `ENCRYPT_OWNER.load()` returns `0` (None) after shutdown.
- No access-violation or segfault.

---

## Automated Unit Test Coverage

All acceptance-criteria items have automated unit tests that run on every CI
push (macOS host, `cargo test -p textquest-dll`):

| Test | File | Covers |
|------|------|--------|
| `per_frame_skips_when_timer_queue_holds_encrypt_owner` | `stealth/mod.rs` | Frame path bails when timer holds owner |
| `timer_queue_skips_when_frame_holds_encrypt_owner` | `stealth/mod.rs` | Timer CAS fails when frame holds owner |
| `no_deadlock_rapid_frame_acquire_release` | `stealth/mod.rs` | 1 000-iteration stress, no panic |
| `per_frame_and_timer_queue_cannot_overlap` | `stealth/timer_queue_sleep.rs` | Mutual exclusion invariant |
| `try_acquire_frame_owner_fails_when_held` | `stealth/timer_queue_sleep.rs` | Second acquire fails |
| `timer_queue_cas_fails_while_frame_holds_encrypt_owner` | `stealth/timer_queue_sleep.rs` | CAS semantics |

---

## References

- Issue #3408 (parent: #2180)
- `textquest-dll/src/stealth/mod.rs`
- `textquest-dll/src/stealth/timer_queue_sleep.rs`
- `docs/research/B6-timer-queue-sleep-obfuscation.md`
