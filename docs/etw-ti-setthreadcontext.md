# ETW-TI Telemetry: SetThreadContext HWBP Installation

This document describes how to capture and interpret Windows ETW-TI (Event Tracing for
Windows - Threat Intelligence) telemetry for `SetThreadContext` calls, specifically in
the context of TextQuest's hardware breakpoint (HWBP) installation mechanism.

## Background

ETW-TI is a high-fidelity kernel telemetry provider (`Microsoft-Windows-Threat-Intelligence`,
GUID `f4e1897c-bb5d-5668-f1d8-040f4d8dd344`) available to processes with `SeDebugPrivilege`
or EDR kernel drivers via protected process handles. It emits events for security-sensitive
operations such as process injection, memory allocation with executable permissions, and
cross-thread debug register writes.

TextQuest's `textquest-dll` installs hardware breakpoints on EQ's main thread by calling
`SetThreadContext` from a worker thread (the PoolParty injection thread). This exact pattern
is what ETW-TI's `SETTHREADCONTEXT_REMOTE` keyword is designed to flag.

---

## EtwTiViewer Setup

[EtwTiViewer](https://github.com/pathtofile/etwtiviewer) (or any ETW consumer with
`Microsoft-Windows-Threat-Intelligence` support) can capture these events.

### Enabling the SETTHREADCONTEXT_REMOTE keyword

ETW-TI uses keyword bitmasks to gate which event types are emitted. The keyword for
cross-thread `SetThreadContext` (i.e., the caller thread differs from the target thread)
is documented in public ETW manifests as keyword bit `0x40` (`SETTHREADCONTEXT_REMOTE`).

**EtwTiViewer session config:**

```xml
<ETWProvider name="Microsoft-Windows-Threat-Intelligence"
             guid="{f4e1897c-bb5d-5668-f1d8-040f4d8dd344}"
             level="5"
             keywords="0x40" />
```

Or via `logman` on an elevated command prompt:

```cmd
logman create trace etw-ti-stc ^
  -p "Microsoft-Windows-Threat-Intelligence" 0x40 0x5 ^
  -o C:\traces\etw-ti-stc.etl ^
  -ets

REM  ... trigger the action (inject DLL, load EQ) ...

logman stop etw-ti-stc -ets
xperf -i C:\traces\etw-ti-stc.etl -o etw-ti-stc.txt -a dumper
```

To enable **all** ETW-TI keywords (for broad coverage during research):

```cmd
logman create trace etw-ti-all ^
  -p "Microsoft-Windows-Threat-Intelligence" 0xFFFFFFFFFFFFFFFF 0x5 ^
  -o C:\traces\etw-ti-all.etl -ets
```

---

## Expected Field Values for HWBP Installation

When TextQuest's `hwbp::register()` sets a hardware breakpoint on EQ's main thread,
the sequence is:

1. `OpenThread(THREAD_GET_CONTEXT | THREAD_SET_CONTEXT | THREAD_SUSPEND_RESUME, FALSE, eq_tid)`
2. `SuspendThread(eq_thread_handle)`
3. `GetThreadContext(eq_thread_handle, &ctx)` — with `ContextFlags = 0x00100010`
   (`CONTEXT_DEBUG_REGISTERS | CONTEXT_AMD64`)
4. Write target address into `ctx.Dr0` (or Dr1/Dr2/Dr3 for additional slots)
5. Set enable bit in `ctx.Dr7`
6. `SetThreadContext(eq_thread_handle, &ctx)`
7. `ResumeThread(eq_thread_handle)`

The ETW-TI `SETTHREADCONTEXT_REMOTE` event fires at step 6. Expected field values:

| Field             | Expected Value / Notes                                                |
| ----------------- | --------------------------------------------------------------------- |
| `CallerProcessId` | TextQuest orchestrator PID (or the EQ process itself, post-inject)    |
| `CallerThreadId`  | Worker thread TID — differs from `TargetThreadId`                     |
| `TargetProcessId` | EQ client PID                                                         |
| `TargetThreadId`  | EQ main thread TID (resolved via `FindWindowA("_EverQuestwndclass")`) |
| `ContextFlags`    | `0x00100010` — `CONTEXT_DEBUG_REGISTERS \| CONTEXT_AMD64`             |
| `Dr0`             | Runtime address of the hooked EQ function (e.g., `ProcessGameEvents`) |
| `Dr1`–`Dr3`       | `0x0` if unused, or addresses of additional hooks                     |
| `Dr6`             | `0x0` (status register — cleared before set)                          |
| `Dr7`             | Enable bits set for active slots; see bit layout below                |

### DR7 Bit Layout

TextQuest uses local enable bits only (bits 0, 2, 4, 6 for DR0–DR3 respectively).
The condition/length fields (bits 16–31) are cleared before writing.

| Slots active | Expected DR7 value |
| ------------ | ------------------ |
| DR0 only     | `0x00000001`       |
| DR0 + DR1    | `0x00000005`       |
| DR0–DR3 all  | `0x00000055`       |

The ETW blinding path (`stealth::etw_blind`) uses DR0 for `NtTraceEvent` and sets
`DR7 = (prev & !0x000F_0003) | 0x1`. An event for this path has:

| Field | Value                                              |
| ----- | -------------------------------------------------- |
| `Dr0` | Address of `NtTraceEvent` in ntdll.dll             |
| `Dr7` | `0x1` (local enable for DR0, all other bits clear) |

---

## Correlating with TextQuest's HWBP Installation

### Source files

| File                                     | Role                                                                  |
| ---------------------------------------- | --------------------------------------------------------------------- |
| `textquest-dll/src/hooks/hwbp.rs`        | Core HWBP engine — `register()`, `set_breakpoint_on_main_thread()`    |
| `textquest-dll/src/stealth/etw_blind.rs` | ETW blinding — sets DR0 on calling thread to intercept `NtTraceEvent` |

### Correlation approach

1. **Capture the ETW-TI trace** during DLL injection into EQ (see session setup above).
2. **Note the `CallerThreadId`** in `SETTHREADCONTEXT_REMOTE` events. Events from
   `hwbp.rs` originate on the PoolParty injection worker thread, not EQ's main thread.
   Events from `etw_blind.rs` have `CallerThreadId == TargetThreadId` (same-thread
   `SetThreadContext`).
3. **Cross-reference `Dr0`** with the EQ module base. TextQuest offsets are preferred-base
   `0x140000000`; at runtime use `offsets::rebase(preferred, actual_base)` to compute
   the effective address. The `Dr0` value in the ETW event should match this rebased address.
4. **DR7 value distinguishes hook count.** A single `ProcessGameEvents` hook produces
   `DR7 = 0x1`. Two hooks produce `DR7 = 0x5`. Full four-slot usage produces `DR7 = 0x55`.

### tracing log correlation

TextQuest logs HWBP installation at `INFO` level:

```
INFO textquest_dll::hooks::hwbp: HWBP set on main thread via cross-thread SetThreadContext
  tid=<eq_main_tid> slot=0 addr=0x140xxxxxx
INFO textquest_dll::hooks::hwbp: VEH handler installed for HWBP dispatch
INFO textquest_dll::stealth::etw_blind: resolved NtTraceEvent addr=0x7ff...
INFO textquest_dll::stealth::etw_blind: ETW blinding active (patchless via DR0)
```

Match the `addr=` field in the log against `Dr0` in the ETW-TI event. The `tid=` field
maps to `TargetThreadId`.

---

## Sample JSONL Event Format

Below is a representative JSONL record as produced by an ETW consumer (e.g., SilkETW,
EtwTiViewer with JSON output, or a custom `TraceEvent` session):

```jsonl
{
  "Provider": "Microsoft-Windows-Threat-Intelligence",
  "EventName": "SETTHREADCONTEXT_REMOTE",
  "Timestamp": "2026-04-13T04:00:00.123456Z",
  "ProcessId": 12345,
  "ThreadId": 67890,
  "Payload": {
    "CallerProcessId": 12345,
    "CallerThreadId": 67890,
    "TargetProcessId": 12345,
    "TargetThreadId": 11111,
    "ContextFlags": "0x00100010",
    "Dr0": "0x140123456",
    "Dr1": "0x0",
    "Dr2": "0x0",
    "Dr3": "0x0",
    "Dr6": "0x0",
    "Dr7": "0x00000001"
  }
}
```

**ETW blinding path** (same-thread, `etw_blind.rs`):

```jsonl
{
  "Provider": "Microsoft-Windows-Threat-Intelligence",
  "EventName": "SETTHREADCONTEXT_REMOTE",
  "Timestamp": "2026-04-13T04:00:00.120000Z",
  "ProcessId": 12345,
  "ThreadId": 22222,
  "Payload": {
    "CallerProcessId": 12345,
    "CallerThreadId": 22222,
    "TargetProcessId": 12345,
    "TargetThreadId": 22222,
    "ContextFlags": "0x00100010",
    "Dr0": "0x7ffb1234abcd",
    "Dr1": "0x0",
    "Dr2": "0x0",
    "Dr3": "0x0",
    "Dr6": "0x0",
    "Dr7": "0x00000001"
  }
}
```

Note: `CallerThreadId == TargetThreadId` here because `etw_blind.rs` calls
`GetCurrentThread()` (a pseudo-handle targeting the calling thread itself).

---

## Optional Event Filter Snippet

The following PowerShell snippet uses `Microsoft.Diagnostics.Tracing.TraceEvent`
(NuGet: `Microsoft.Diagnostics.Tracing.TraceEvent`) to filter for TextQuest-relevant
`SETTHREADCONTEXT_REMOTE` events in real time. Requires elevation and `SeDebugPrivilege`.

```powershell
# Requires: Install-Package Microsoft.Diagnostics.Tracing.TraceEvent
Add-Type -Path "C:\tools\TraceEvent\Microsoft.Diagnostics.Tracing.TraceEvent.dll"

$session = [Microsoft.Diagnostics.Tracing.Session.TraceEventSession]::new(
    "TextQuestHwbpCapture", $null
)

$session.EnableProvider(
    [System.Guid]::new("f4e1897c-bb5d-5668-f1d8-040f4d8dd344"),
    [Microsoft.Diagnostics.Tracing.TraceEventLevel]::Verbose,
    [uint64]0x40   # SETTHREADCONTEXT_REMOTE keyword only
)

$session.Source.Dynamic.All += {
    param($event)
    if ($event.EventName -notmatch "SETTHREADCONTEXT") { return }

    $dr7 = $event.PayloadByName("Dr7")
    $dr0 = $event.PayloadByName("Dr0")

    # Filter: only log events where debug registers are non-zero
    if ($dr0 -ne 0 -or $dr7 -ne 0) {
        Write-Host ("[{0}] PID={1} CallerTID={2} -> TargetTID={3} Dr0={4:X16} Dr7={5:X16}" -f `
            $event.TimeStamp, $event.ProcessID,
            $event.PayloadByName("CallerThreadId"),
            $event.PayloadByName("TargetThreadId"),
            $dr0, $dr7)
    }
}

$session.Source.Process()
```

Save output to a `.jsonl` file by replacing `Write-Host` with a `ConvertTo-Json | Add-Content` call.

---

## Notes

- ETW-TI events are only visible to consumers running as `NT AUTHORITY\SYSTEM` or a
  process with a kernel driver handle (`EtwTiOpenRegistrationHandle`). Standard admin
  processes cannot receive them directly.
- The `SETTHREADCONTEXT_REMOTE` event does **not** fire when `CallerThreadId ==
TargetThreadId` on some Windows versions. The ETW blinding `etw_blind.rs` path may
  therefore be invisible to this specific keyword on certain builds — use the broader
  `0xFFFFFFFFFFFFFFFF` keyword mask during research to confirm.
- TextQuest's VEH (`hwbp.rs::veh_handler`) lives in the `.tq` PE section (not `.text`)
  so it remains executable when the sleep obfuscation encrypts `.text`. This section name
  may appear in PE section enumeration events if those keywords are enabled.
