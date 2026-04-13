# ETW-TI Telemetry: VirtualProtect RWX Trampoline Marking

## Overview

Windows Threat Intelligence ETW (ETW-TI) is a kernel-mode provider that emits
telemetry for security-sensitive operations, including memory protection changes
made via `VirtualProtect` / `VirtualProtectEx`. Any time a region of memory
transitions to a protection mask that includes execute permissions — in
particular the `PAGE_EXECUTE_READWRITE` (RWX) pattern common to trampoline-style
hooks — ETW-TI emits a `PROTECTVM_REMOTE` event that EDR/AV sensors subscribe
to in real-time.

TextQuest's injected DLL (`textquest-dll`) uses two stealth techniques that
interact with this telemetry surface:

1. **Page encryption** (`stealth/page_encrypt.rs`) — rotates pages through
   `PAGE_READWRITE` → `PAGE_NOACCESS` → `PAGE_EXECUTE_READ` cycles via
   `VirtualProtect`, generating repeated protection-change events.
2. **ETW blinding** (`stealth/etw_blind.rs`) — suppresses all ETW emission from
   the process by intercepting `NtTraceEvent` via a hardware breakpoint (DR0)
   and a Vectored Exception Handler (VEH).

This document explains how to capture and interpret the ETW-TI events that would
appear _before_ blinding is active, how to correlate them with TextQuest's hook
installation sequence, and how to filter a live JSONL event stream.

---

## 1. EtwTiViewer Setup — PROTECTVM_REMOTE Keyword

**EtwTiViewer** (part of the ETWInspector / Microsoft SysinternalsAlpha
toolset) surfaces ETW-TI events in real-time. To capture only memory-protection
events:

### 1.1 Start an EtwTiViewer session targeting the EQ client PID

```powershell
# Replace <PID> with the eqgame.exe process ID.
EtwTiViewer.exe -pid <PID> -provider Microsoft-Windows-Threat-Intelligence ^
    -keyword PROTECTVM_REMOTE
```

If EtwTiViewer is not available, use a raw ETW session via `logman` or
`xperf`:

```powershell
# Start a kernel-mode ETW session
logman start TI-Capture `
    -p "Microsoft-Windows-Threat-Intelligence" 0x40 0x4 `
    -o C:\Temp\ti-capture.etl -ets

# ... reproduce the operation ...

logman stop TI-Capture -ets

# Convert to JSONL for offline analysis
wpr -stop C:\Temp\ti-capture.etl
python3 scripts/etl_to_jsonl.py C:\Temp\ti-capture.etl > ti-events.jsonl
```

### 1.2 Keyword bitmask reference

| Keyword name       | Bitmask      | Triggers on                                           |
| ------------------ | ------------ | ----------------------------------------------------- |
| `PROTECTVM_REMOTE` | `0x00000040` | `VirtualProtect` / `VirtualProtectEx` calls (any PID) |
| `ALLOCVM_REMOTE`   | `0x00000004` | `VirtualAlloc` / `VirtualAllocEx`                     |
| `MAPVIEW_REMOTE`   | `0x00000010` | `MapViewOfSection` (DLL injection path)               |
| `SETTHREADCONTEXT` | `0x00000100` | `SetThreadContext` (HWBP installation)                |

Use `0x40` alone for VirtualProtect-only capture. Combine with `0x100` to
also catch the HWBP-based hook installation used by `stealth/etw_blind.rs`.

---

## 2. Expected Field Values for RWX Page Operations

Each `PROTECTVM_REMOTE` event carries the following fields. The values below
are what you will see when `page_encrypt.rs` cycles a code page, or when a
classic trampoline hook is written.

### 2.1 Page encryption cycle (TextQuest stealth)

TextQuest does **not** use RWX trampolines. Instead it cycles pages through
three protection states in sequence:

| Transition           | `ProtectionMask` value | `OldProtection` value | Notes                       |
| -------------------- | ---------------------- | --------------------- | --------------------------- |
| Arm for write        | `0x04` (`RW`)          | `0x20` (`XR`)         | Makes page writable for XOR |
| XOR encrypt + lock   | `0x01` (`NOACCESS`)    | `0x04` (`RW`)         | Locks encrypted page        |
| Decrypt + re-execute | `0x20` (`XR`)          | `0x01` (`NOACCESS`)   | Page restored for execution |

The full `PAGE_EXECUTE_READWRITE` (`0x40`, RWX) flag is **never set** by
TextQuest's page encryption — it avoids the most fingerprintable pattern.

### 2.2 Classic trampoline hook (reference)

A conventional inline hook writer (e.g., MinHook, Detours) produces this
sequence at the target function's page:

| Step | `ProtectionMask` | `OldProtection` | Meaning                     |
| ---- | ---------------- | --------------- | --------------------------- |
| 1    | `0x40` (RWX)     | `0x20` (XR)     | Enable write to patch bytes |
| 2    | `0x20` (XR)      | `0x40` (RWX)    | Restore after patch         |

The `0x40` (RWX) value in step 1 is the primary EDR signal. TextQuest's
HWBP-based approach never produces this because it never modifies code bytes.

### 2.3 Key ETW-TI event fields

| Field name         | Type     | Description                                          |
| ------------------ | -------- | ---------------------------------------------------- |
| `CallingProcessId` | uint32   | PID of the process calling VirtualProtect            |
| `TargetProcessId`  | uint32   | PID of the process whose pages are changed           |
| `BaseAddress`      | uint64   | Start of the affected virtual address range          |
| `RegionSize`       | uint64   | Size of the region in bytes (usually 0x1000 = 4096)  |
| `ProtectionMask`   | uint32   | The _new_ protection flags being applied             |
| `OldProtection`    | uint32   | The protection flags before the call                 |
| `TimeStamp`        | filetime | Kernel event timestamp (100-ns ticks since 1601)     |
| `ProcessId`        | uint32   | PID of the process generating the event (kernel-set) |

---

## 3. Correlating with TextQuest's Hook Installation

### 3.1 Injection timeline

When TextQuest injects `textquest-dll.dll` into an EQ client:

```
t=0   CreateRemoteThread / APC → DLL_PROCESS_ATTACH fires
t+1   stealth::init() called
        ├── etw_blind::init()     ← suppresses ALL further ETW-TI after this point
        ├── page_encrypt::init()  ← begins page cycling (XR↔RW↔NOACCESS)
        └── peb_unlink::init()    ← removes DLL from PEB loader lists
t+2   hooks::install_all() called
        └── hwbp::install()       ← SetThreadContext (DR0-DR3) → SETTHREADCONTEXT event
```

**Critical window**: ETW-TI events from `VirtualProtect` and `SetThreadContext`
can only appear between `t=0` and `t+1` (while `etw_blind` is not yet active).
After `etw_blind::init()` completes, `NtTraceEvent` is intercepted by the DR0
HWBP and all ETW emission from the process is silenced.

### 3.2 Matching BaseAddress to the DLL code section

To correlate a captured event's `BaseAddress` with TextQuest's code pages:

1. Find the DLL load base from a prior `MAPVIEW_REMOTE` event (same
   `TargetProcessId`, `BaseAddress` aligned to 0x10000).
2. The `.text` section starts at `DLL_BASE + 0x1000` on a standard PE layout.
3. Each subsequent `PROTECTVM_REMOTE` with `RegionSize = 0x1000` and
   `BaseAddress` in range `[DLL_BASE + 0x1000, DLL_BASE + .text_size]`
   corresponds to one page encryption cycle.

### 3.3 SETTHREADCONTEXT correlation

The `HWBP` hook and `etw_blind` both call `SetThreadContext` to write debug
registers. A `SETTHREADCONTEXT` ETW-TI event with:

- `CallingProcessId == TargetProcessId` (same process)
- `ContextFlags` containing `0x00100010` (CONTEXT_DEBUG_REGISTERS | CONTEXT_AMD64)

indicates that DR0 is being armed — either for ETW blinding (`NtTraceEvent`
address) or for a game function hook.

---

## 4. Sample JSONL Event Format

Below is a representative JSONL line for each relevant event type. Fields not
shown are omitted for brevity.

### 4.1 VirtualProtect: page encryption arm (RW)

```jsonl
{
  "EventName": "PROTECTVM_REMOTE",
  "TimeStamp": "2026-04-13T14:22:01.123456Z",
  "CallingProcessId": 9812,
  "TargetProcessId": 9812,
  "BaseAddress": "0x7FF6A1231000",
  "RegionSize": 4096,
  "OldProtection": 32,
  "ProtectionMask": 4,
  "ProviderGuid": "{F4E1897C-BB5D-5668-F1D8-040F4D8DD344}"
}
```

- `OldProtection: 32` = `PAGE_EXECUTE_READ` (0x20)
- `ProtectionMask: 4` = `PAGE_READWRITE` (0x04)

### 4.2 VirtualProtect: page lock (NOACCESS)

```jsonl
{
  "EventName": "PROTECTVM_REMOTE",
  "TimeStamp": "2026-04-13T14:22:01.124001Z",
  "CallingProcessId": 9812,
  "TargetProcessId": 9812,
  "BaseAddress": "0x7FF6A1231000",
  "RegionSize": 4096,
  "OldProtection": 4,
  "ProtectionMask": 1,
  "ProviderGuid": "{F4E1897C-BB5D-5668-F1D8-040F4D8DD344}"
}
```

- `OldProtection: 4` = `PAGE_READWRITE`
- `ProtectionMask: 1` = `PAGE_NOACCESS`

### 4.3 SetThreadContext: DR0 arm (ETW blind or HWBP hook)

```jsonl
{
  "EventName": "SETTHREADCONTEXT",
  "TimeStamp": "2026-04-13T14:22:00.998123Z",
  "CallingProcessId": 9812,
  "TargetProcessId": 9812,
  "TargetThreadId": 11044,
  "ContextFlags": 1048592,
  "ProviderGuid": "{F4E1897C-BB5D-5668-F1D8-040F4D8DD344}"
}
```

- `ContextFlags: 1048592` = `0x00100010` (CONTEXT_DEBUG_REGISTERS | CONTEXT_AMD64)

### 4.4 Classic RWX trampoline (reference — NOT produced by TextQuest)

```jsonl
{
  "EventName": "PROTECTVM_REMOTE",
  "TimeStamp": "2026-04-13T14:22:01.000000Z",
  "CallingProcessId": 9812,
  "TargetProcessId": 9812,
  "BaseAddress": "0x7FF6A0001000",
  "RegionSize": 4096,
  "OldProtection": 32,
  "ProtectionMask": 64,
  "ProviderGuid": "{F4E1897C-BB5D-5668-F1D8-040F4D8DD344}"
}
```

- `ProtectionMask: 64` = `PAGE_EXECUTE_READWRITE` (0x40) — the classic EDR trigger

---

## 5. Event Filter Snippets

### 5.1 Rust filter (for integration into a future ETW consumer)

```rust
/// Returns true if this event represents an RWX trampoline write —
/// the canonical EDR trigger not produced by TextQuest's page encryption.
pub fn is_rwx_trampoline(protection_mask: u32) -> bool {
    const PAGE_EXECUTE_READWRITE: u32 = 0x40;
    const PAGE_EXECUTE_WRITECOPY: u32 = 0x80;
    (protection_mask & PAGE_EXECUTE_READWRITE) != 0
        || (protection_mask & PAGE_EXECUTE_WRITECOPY) != 0
}

/// Returns true if this is a TextQuest-style page encryption cycle —
/// never RWX, alternates between RW/NOACCESS/XR on the same base address.
pub fn is_page_encrypt_cycle(old_protection: u32, protection_mask: u32) -> bool {
    const PAGE_NOACCESS: u32 = 0x01;
    const PAGE_READWRITE: u32 = 0x04;
    const PAGE_EXECUTE_READ: u32 = 0x20;

    matches!(
        (old_protection, protection_mask),
        (PAGE_EXECUTE_READ, PAGE_READWRITE)
            | (PAGE_READWRITE, PAGE_NOACCESS)
            | (PAGE_NOACCESS, PAGE_EXECUTE_READ)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rwx_trampoline_detected() {
        assert!(is_rwx_trampoline(0x40));
        assert!(is_rwx_trampoline(0x80));
        assert!(!is_rwx_trampoline(0x04));
        assert!(!is_rwx_trampoline(0x20));
    }

    #[test]
    fn page_encrypt_cycle_detected() {
        // XR -> RW (arm for write)
        assert!(is_page_encrypt_cycle(0x20, 0x04));
        // RW -> NOACCESS (lock)
        assert!(is_page_encrypt_cycle(0x04, 0x01));
        // NOACCESS -> XR (restore)
        assert!(is_page_encrypt_cycle(0x01, 0x20));
        // RWX is not a page-encrypt cycle
        assert!(!is_page_encrypt_cycle(0x20, 0x40));
    }
}
```

### 5.2 Python filter (offline JSONL analysis)

```python
#!/usr/bin/env python3
"""
Filter ETW-TI JSONL events for memory protection patterns.

Usage:
    python3 scripts/etw_filter.py ti-events.jsonl --pid 9812
"""

import argparse
import json
import sys

PAGE_EXECUTE_READWRITE = 0x40
PAGE_EXECUTE_WRITECOPY = 0x80
PAGE_NOACCESS          = 0x01
PAGE_READWRITE         = 0x04
PAGE_EXECUTE_READ      = 0x20

PAGE_ENCRYPT_CYCLES = {
    (PAGE_EXECUTE_READ, PAGE_READWRITE),   # arm for write
    (PAGE_READWRITE, PAGE_NOACCESS),       # lock encrypted page
    (PAGE_NOACCESS, PAGE_EXECUTE_READ),    # restore for execution
}


def is_rwx_trampoline(mask: int) -> bool:
    return bool(mask & (PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY))


def is_page_encrypt_cycle(old: int, new: int) -> bool:
    return (old, new) in PAGE_ENCRYPT_CYCLES


def classify(event: dict) -> str:
    if event.get("EventName") != "PROTECTVM_REMOTE":
        return "other"
    old = event.get("OldProtection", 0)
    new = event.get("ProtectionMask", 0)
    if is_rwx_trampoline(new):
        return "RWX_TRAMPOLINE"
    if is_page_encrypt_cycle(old, new):
        return "PAGE_ENCRYPT_CYCLE"
    return "other"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("jsonl", help="Path to ETW-TI JSONL file")
    ap.add_argument("--pid", type=int, help="Filter to target process ID")
    ap.add_argument(
        "--show", choices=["RWX_TRAMPOLINE", "PAGE_ENCRYPT_CYCLE", "all"],
        default="all",
    )
    args = ap.parse_args()

    with open(args.jsonl) as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                ev = json.loads(line)
            except json.JSONDecodeError:
                continue

            if args.pid and ev.get("TargetProcessId") != args.pid:
                continue

            label = classify(ev)
            if args.show != "all" and label != args.show:
                continue

            print(f"[{label:20s}] {ev.get('TimeStamp','')}  "
                  f"base={hex(ev.get('BaseAddress', 0))}  "
                  f"old={hex(ev.get('OldProtection', 0))}  "
                  f"new={hex(ev.get('ProtectionMask', 0))}")


if __name__ == "__main__":
    main()
```

---

## 6. References

- `textquest-dll/src/stealth/etw_blind.rs` — NtTraceEvent HWBP suppression
- `textquest-dll/src/stealth/page_encrypt.rs` — Nighthawk-style page cycling
- `textquest-dll/src/hooks/hwbp.rs` — DR0-DR3 HWBP hook engine
- `textquest-dll/src/stealth/mod.rs` — Stealth init sequence
- Microsoft ETW-TI provider GUID: `{F4E1897C-BB5D-5668-F1D8-040F4D8DD344}`
- Protection flag constants: `winnt.h` `PAGE_*` defines
