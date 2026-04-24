# Hook Rotation Tuning

Issue: #925
Parent: #732 / PR #786

## Recommendation

Keep hook rotation disabled by default. When rotation is explicitly enabled for live
validation, use:

```toml
hook_rotation_interval_ms = 10000
```

Use 10,000 ms as the enabled tuning target, with 5,000-15,000 ms as the
acceptable validation band. Do not tune below 1,000 ms without changing the
rotation implementation, and treat intervals below 100 ms as unsupported.

The current shipped default from PR #786 is disabled with a 30,000 ms interval.
That remains the safest production posture while the EQ scanner cadence is not
fully measured. A 10,000 ms interval is the preferred live-validation value
because it stays inside the existing design band in `docs/specs/dll-hooks.md`
while avoiding the high scheduler churn and hook downtime caused by subsecond
rotation.

## Evidence

### EQ scan frequency

Issue #722 identifies the likely Ghidra target as
`MainGameLoop_AntiCheatValidation` at `0x140270d00`. The available issue notes
also record outside analysis that the current API detour check appears broken:
the check compares the function pointer value instead of dereferencing bytes at
that pointer. That makes current observed detour-detection risk low, but it is a
patch-sensitive bug and should not be treated as a durable control.

The available #722 evidence does not include a measured scan cadence. Until the
Ghidra call graph and live telemetry prove otherwise, assume the validation path
can be main-loop adjacent. At 60 FPS, a main-loop-bound scan could run about
every 16.7 ms. That number is a conservative planning assumption, not confirmed
Ghidra output.

### Hook-down window

`textquest-dll/src/hooks/rotation.rs` removes all registered hooks, sleeps for
50 ms, and then reinstalls the hooks. The effective hook-down window is
therefore at least 50 ms plus the time spent running the unhook and rehook
callbacks.

This makes 50 ms the current operational window, not a proven minimum. A smaller
window would require a code change to make the sleep configurable or adaptive,
then a Windows/live-client validation pass.

### Sub-100 ms intervals

Intervals below 100 ms are not viable with the current implementation. Because
each cycle includes a fixed 50 ms hook-down sleep, a 100 ms interval already
leaves hooks down for roughly half the cycle. Lower intervals approach
continuous unhook/rehook churn and provide little useful hook coverage.

Approximate hook-down duty cycle with the current 50 ms sleep:

| Interval | Hook-down duty cycle |
| --- | ---: |
| 100 ms | 50.0% |
| 1,000 ms | 5.0% |
| 5,000 ms | 1.0% |
| 10,000 ms | 0.5% |
| 30,000 ms | 0.17% |

Although rotation runs on a background thread, rapid intervals still create
avoidable scheduler churn and repeated hook-state transitions. They also create
large windows where TextQuest hooks are intentionally absent. For live tuning,
visible micro-stutter testing should start at 30,000 ms, then 15,000 ms, 10,000
ms, and 5,000 ms. Subsecond intervals should only be tested as a negative
control.

## Rationale

The recommended 10,000 ms interval balances three constraints:

1. It stays in the documented 5-15 second rotation band.
2. It limits the current 50 ms hook-down period to about 0.5% of wall-clock
   time.
3. It avoids the pathological behavior of sub-100 ms intervals, where the
   hard-coded hook-down sleep dominates the cycle.

The recommendation does not claim that periodic hook rotation can defeat a
main-loop-frequency scanner. If #722 later confirms the scanner runs every
frame and correctly inspects the relevant hook surface, interval tuning alone is
not sufficient. In that case, the safer path remains reducing detectable hook
surface area, especially by preferring HWBP paths over inline detours and vtable
swaps.

## Follow-up validation

Run this on the Windows live EQ client before enabling rotation by default:

1. Capture a baseline with rotation disabled.
2. Capture frame-time p50, p95, p99, and maximum frame time at 30,000 ms,
   15,000 ms, 10,000 ms, and 5,000 ms.
3. Record `rotation_count`, hook reinstall failures, and any missed hook events.
4. Stop the test if p99 frame time regresses by more than 5% or visible stutter
   appears.
5. Update this note with the confirmed #722 scanner cadence once the Ghidra
   call graph is complete.
