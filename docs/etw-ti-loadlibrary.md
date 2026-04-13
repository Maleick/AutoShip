# ETW-TI Telemetry: DLL Injection via LoadLibrary Path

## Overview

Windows Threat Intelligence (ETW-TI) is a high-fidelity kernel-mode ETW provider
(`Microsoft-Windows-Threat-Intelligence`, GUID `F4E1897C-BB5D-5668-F1D8-040F4D8DD344`)
that emits events for sensitive operations before they complete. Unlike user-mode
ETW providers, ETW-TI runs in the kernel and cannot be silenced from user space via
`NtTraceEvent` blinding — it fires even if the injected DLL's `etw_blind.rs` has
suppressed the process's own trace calls.

For the `CreateRemoteThread + LoadLibraryW` injection path used in
`textquest/src/inject/loader.rs`, ETW-TI emits several telemetry events that security
tools (CrowdStrike Falcon, Windows Defender ATP, custom SIEM collectors) can capture.

---

## Relevant ETW-TI Keywords and Event IDs

### Provider

| Field         | Value                                      |
| ------------- | ------------------------------------------ |
| Provider Name | `Microsoft-Windows-Threat-Intelligence`    |
| Provider GUID | `F4E1897C-BB5D-5668-F1D8-040F4D8DD344`     |
| Log Session   | NT Kernel Logger / Protected sessions only |

### Keyword Flags

ETW-TI uses keyword bitmasks to gate event categories. The following keywords are
relevant to the `LoadLibrary`-based injection path:

| Keyword Name                                   | Hex Value            | Description                                                              |
| ---------------------------------------------- | -------------------- | ------------------------------------------------------------------------ |
| `KERNEL_THREATINT_KEYWORD_ALLOCVM_REMOTE`      | `0x0000000000000002` | Remote virtual memory allocation (`VirtualAllocEx` into another process) |
| `KERNEL_THREATINT_KEYWORD_WRITEVM_REMOTE`      | `0x0000000000000008` | Remote memory write (`WriteProcessMemory`)                               |
| `KERNEL_THREATINT_KEYWORD_SETTHREADCONTEXT`    | `0x0000000000000080` | Remote thread context manipulation                                       |
| `KERNEL_THREATINT_KEYWORD_MAPVIEW_REMOTE`      | `0x0000000000000010` | Remote section map into another process                                  |
| `KERNEL_THREATINT_KEYWORD_QUEUEUSERAPC_REMOTE` | `0x0000000000000020` | Remote APC queuing                                                       |

For `CreateRemoteThread + LoadLibraryW`, the minimum keywords to enable are:

```
KERNEL_THREATINT_KEYWORD_ALLOCVM_REMOTE  (0x2)
KERNEL_THREATINT_KEYWORD_WRITEVM_REMOTE  (0x8)
```

A full-capture session enabling all DLL-injection-relevant keywords:

```
0x0000000000000002 | 0x0000000000000008 | 0x0000000000000080 = 0x8A
```

### Event IDs

| Event ID | Name                  | Trigger                                    |
| -------- | --------------------- | ------------------------------------------ |
| `1`      | `ALLOCVM_REMOTE`      | `VirtualAllocEx` into a remote process     |
| `2`      | `WRITEVM_REMOTE`      | `WriteProcessMemory` into a remote process |
| `5`      | `READVM_REMOTE`       | `ReadProcessMemory` from a remote process  |
| `10`     | `SETTHREADCONTEXT`    | `SetThreadContext` on a remote thread      |
| `12`     | `MAPVIEW_REMOTE`      | `NtMapViewOfSection` into remote process   |
| `14`     | `QUEUEUSERAPC_REMOTE` | `NtQueueApcThread` into remote thread      |

For the LoadLibrary injection sequence in `loader.rs`:

1. `VirtualAllocEx` → fires **Event ID 1** (`ALLOCVM_REMOTE`)
2. `WriteProcessMemory` (DLL path string) → fires **Event ID 2** (`WRITEVM_REMOTE`)
3. `CreateRemoteThread(LoadLibraryW, ...)` → the thread creation itself is not a
   direct ETW-TI event, but the loader inside the target process then calls
   `LdrLoadDll` which triggers `ImageLoad` kernel callbacks, visible via the
   separate `Microsoft-Windows-Kernel-Process` provider.

---

## Expected Field Values

### ALLOCVM_REMOTE (Event ID 1)

| Field                | Expected Value                            |
| -------------------- | ----------------------------------------- |
| `CallingProcessId`   | PID of the orchestrator (`textquest.exe`) |
| `CallingProcessName` | `textquest.exe`                           |
| `TargetProcessId`    | PID of the EQ client being injected into  |
| `TargetProcessName`  | `eqgame.exe`                              |
| `BaseAddress`        | Address returned by `VirtualAllocEx`      |
| `RegionSize`         | Length of the DLL path string in bytes    |
| `AllocationType`     | `0x3000` (MEM_COMMIT \| MEM_RESERVE)      |
| `Protect`            | `0x4` (PAGE_READWRITE)                    |

### WRITEVM_REMOTE (Event ID 2)

| Field              | Expected Value                                   |
| ------------------ | ------------------------------------------------ |
| `CallingProcessId` | PID of `textquest.exe`                           |
| `TargetProcessId`  | PID of `eqgame.exe`                              |
| `BaseAddress`      | Same address from the preceding `VirtualAllocEx` |
| `ByteCount`        | Byte length of the DLL path (wide string)        |

### ImageLoad (Microsoft-Windows-Kernel-Process, Event ID 5)

Emitted by the kernel's image-load notify routine when `LoadLibraryW` causes
the PE loader to map the DLL:

| Field           | Expected Value                    |
| --------------- | --------------------------------- |
| `ProcessID`     | PID of `eqgame.exe` (the target)  |
| `ImageName`     | Full path to `textquest_dll.dll`  |
| `ImageBase`     | Load address inside `eqgame.exe`  |
| `ImageSize`     | Size of the mapped image in bytes |
| `ImageCheckSum` | PE checksum from the DLL header   |

---

## Sample JSONL Output from EtwTiViewer

EtwTiViewer (or a custom ETW consumer like `logman` + `tracerpt`, or
`Microsoft-Windows-ETW-Trace-Session`) emits one JSON object per event.
Below are representative JSONL lines for the LoadLibrary injection sequence:

```jsonl
{"EventId":1,"Provider":"Microsoft-Windows-Threat-Intelligence","Keyword":"ALLOCVM_REMOTE","Timestamp":"2026-04-13T14:32:11.0012345Z","CallingProcessId":4812,"CallingProcessName":"textquest.exe","CallingThreadId":9120,"TargetProcessId":7744,"TargetProcessName":"eqgame.exe","BaseAddress":"0x000001F3A2C00000","RegionSize":516,"AllocationType":"0x3000","Protect":"0x4","ProtectName":"PAGE_READWRITE"}
{"EventId":2,"Provider":"Microsoft-Windows-Threat-Intelligence","Keyword":"WRITEVM_REMOTE","Timestamp":"2026-04-13T14:32:11.0013210Z","CallingProcessId":4812,"CallingProcessName":"textquest.exe","CallingThreadId":9120,"TargetProcessId":7744,"TargetProcessName":"eqgame.exe","BaseAddress":"0x000001F3A2C00000","ByteCount":516}
{"EventId":5,"Provider":"Microsoft-Windows-Kernel-Process","Keyword":"IMAGELOAD","Timestamp":"2026-04-13T14:32:11.0891002Z","ProcessID":7744,"ImageName":"C:\\ProgramData\\textquest\\textquest_dll.dll","ImageBase":"0x000001F3B4A00000","ImageSize":2097152,"ImageCheckSum":3294967295,"TimeDateStamp":1712345678}
```

> Note: `BaseAddress` and `ImageBase` values are representative; actual addresses
> depend on ASLR randomization in the target process.

---

## Capture Setup

### Logman (built-in Windows)

```cmd
rem Start capture session for ETW-TI injection events
logman create trace InjectionCapture -p "{F4E1897C-BB5D-5668-F1D8-040F4D8DD344}" 0x8A 0xFF -o C:\etw-ti-capture.etl -mode Circular -bs 64 -nb 20 40
logman start InjectionCapture

rem Also capture ImageLoad from kernel process provider
logman update trace InjectionCapture -p "{22FB2CD6-0E7B-422B-A0C7-2FAD1FD0E716}" 0x10 0xFF

rem After triggering injection:
logman stop InjectionCapture
tracerpt C:\etw-ti-capture.etl -o C:\etw-ti-capture.xml -of XML
```

### PowerShell (PerfView / SilkETW style)

```powershell
# Requires admin privileges and access to the protected ETW-TI session
# (Typically requires a kernel driver or PPL; standard user sessions cannot
#  subscribe to Microsoft-Windows-Threat-Intelligence directly)
$session = New-Object System.Diagnostics.Eventing.Reader.EventLogSession
```

> **Note:** ETW-TI (`Microsoft-Windows-Threat-Intelligence`) is a **protected provider**.
> Subscribing to it from user mode requires a kernel driver or a signed PPL process.
> Tools that can access it include:
>
> - CrowdStrike Falcon (kernel driver)
> - Windows Defender ATP (kernel driver)
> - [SilkETW](https://github.com/mandiant/SilkETW) (with driver component)
> - [EtwTiViewer](https://github.com/repnz/etw-ti-viewer) (PoC, requires admin + driver)
> - WPR / xperf in kernel mode

---

## Detection Logic

The canonical detection pattern for `CreateRemoteThread + LoadLibraryW` injection
based on ETW-TI events:

1. `ALLOCVM_REMOTE` where `CallingProcessId != TargetProcessId`
2. Followed within ~100ms by `WRITEVM_REMOTE` with same `BaseAddress` and
   `CallingProcessId`
3. Followed by `ImageLoad` in `TargetProcessId` where `ImageName` was not
   present at process start

This three-event sequence uniquely identifies the classic injection pattern.

---

## Rust Detection Stub

A minimal Rust module for parsing ETW-TI JSONL events and detecting the
LoadLibrary injection pattern is provided in
`textquest-common/src/etw_ti_detect.rs`.

---

## References

- [EtwTiViewer — repnz](https://github.com/repnz/etw-ti-viewer)
- [SilkETW — Mandiant](https://github.com/mandiant/SilkETW)
- [ETW-TI keyword values — jdu2600/etw-providers-docs](https://github.com/jdu2600/etw-providers-docs)
- [Windows Threat Intelligence ETW — modexp.wordpress.com](https://modexp.wordpress.com/2020/04/08/red-team-vs-etw/)
- [Kernel ETW Threat Intel — Posts by Yarden Shafir](https://windows-internals.com/etw-internals-in-user-mode-and-kernel-mode/)
- `textquest/src/inject/loader.rs` — classic LoadLibraryW injection in this project
- `textquest-dll/src/stealth/etw_blind.rs` — user-mode NtTraceEvent blinding (does NOT affect ETW-TI)
