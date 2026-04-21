# TextQuest ETW-TI Keyword Mapping

Maps every TextQuest operation that crosses process boundaries to its corresponding ETW-TI (Event Tracing for Windows - Threat Intelligence) keyword.

## Overview

TextQuest uses two injection/syscall strategies:

1. **Orchestrator (textquest binary)**: Direct Windows API stubs via the `windows` crate
2. **Injected DLL (textquest-dll)**: Indirect syscalls using the RecycledGate pattern (gadget-based syscall invocation)

The RecycledGate pattern jumps to a `syscall; ret` gadget in the real ntdll.dll, making call stacks appear legitimate to EDR/anti-cheat inspection.

---

## Process Boundary Operations

| Operation              | ETW-TI Keyword                       | ETW Event ID                                | IOCTL / Monitor                       | TextQuest Implementation                                     | Evasion Status                                                                                             |
| ---------------------- | ------------------------------------ | ------------------------------------------- | ------------------------------------- | ------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- |
| **VirtualAllocEx**     | `KERNEL_MEMORY`                      | 9 (MemoryAlloc)                             | ETW Kernel Logger                     | Direct ntdll stub (Windows API) — Orchestrator               | **Direct API call** — leaves ntdll call stack signature                                                    |
| **VirtualProtectEx**   | `PAGE_PROTECTION`                    | 11 (MemoryProtect)                          | ETW Kernel Logger                     | Indirect syscall (`NtProtectVirtualMemory`) — DLL            | **Indirect syscall via RecycledGate** — gadget-based, call stack appears ntdll-native                      |
| **WriteProcessMemory** | `PROCESS_WRITE`                      | N/A (API-level)                             | ETW usermode (rarely monitored)       | Direct ntdll stub (Windows API) — Orchestrator               | **Direct API call** — kernel-mode detection possible but uncommon                                          |
| **ReadProcessMemory**  | `PROCESS_READ`                       | N/A (API-level)                             | ETW usermode (rarely monitored)       | **Not used** — DLL runs in-process                           | N/A                                                                                                        |
| **SetThreadContext**   | `CONTEXT_SWITCH`, `THREAD_OPERATION` | 17 (SetContextThread)                       | ETW Kernel Logger, KernelTraceControl | Indirect syscall (`NtSetContextThread`) — DLL                | **Indirect syscall via RecycledGate** — hardware breakpoint setup disguised as legitimate thread operation |
| **CreateRemoteThread** | `CREATE_REMOTE_THREAD`               | 8, 10, 19, 20 (ProcessCreate, ThreadCreate) | ETW Kernel Logger, WMI Events         | Direct ntdll stub (Windows API) — Orchestrator               | **Direct API call** — monitored by most modern EDR, uses LoadLibraryW as entry point                       |
| **NtCreateThreadEx**   | `CREATE_REMOTE_THREAD`               | 8, 10, 19, 20                               | ETW Kernel Logger                     | **Not used directly** — TextQuest prefers CreateRemoteThread | N/A                                                                                                        |
| **MapViewOfSection**   | `IMAGE_LOAD`, `FILE_MAPPING`         | 3 (ImageLoad)                               | ETW Kernel Logger                     | **Implicit** — via LoadLibraryW (wraps MapViewOfSection)     | **Indirect via LoadLibraryW** — DLL loads itself, then unlinks from PEB module list                        |

---

## Current Evasion Tactics

### Orchestrator (textquest binary)

| Operation                             | Technique                              | Detection Risk                                                                                                |
| ------------------------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| **VirtualAllocEx**                    | Direct Windows API call                | High — monitored by all modern EDR (kernel-level)                                                             |
| **WriteProcessMemory**                | Direct Windows API call                | Medium — API-level monitoring less common than syscall monitoring                                             |
| **CreateRemoteThread + LoadLibraryW** | Standard injection (classic technique) | High — CreateRemoteThread is a primary indicator of DLL injection, but LoadLibraryW is a legitimate operation |

### Injected DLL (textquest-dll)

| Operation                   | Technique                                        | Detection Risk                                                                                                              |
| --------------------------- | ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------- |
| **NtAllocateVirtualMemory** | Indirect syscall via RecycledGate (gadget-based) | **Low-Medium** — call stack appears legitimate (ntdll-native), but syscall monitor hooks at kernel level can detect pattern |
| **NtProtectVirtualMemory**  | Indirect syscall via RecycledGate                | **Low-Medium** — same as above; page protection changes are monitored for RWX transitions                                   |
| **NtSetContextThread**      | Indirect syscall via RecycledGate                | **Low** — legitimate thread operation; used for hardware breakpoint hooking (legitimate use case)                           |
| **Module Unlinking**        | PEB (Process Environment Block) manipulation     | **Low** — unlinks DLL from module enumeration after LoadLibraryW, bypassing `CreateToolhelp32Snapshot` scanning             |

---

## ETW-TI Keyword Taxonomy

### Kernel-Mode Keywords

- **KERNEL_MEMORY**: Memory allocation, deallocation, and protection
- **PAGE_PROTECTION**: Virtual memory page protection changes
- **CREATE_REMOTE_THREAD**: Remote thread creation across processes
- **CONTEXT_SWITCH**: CPU context switching (thread state changes)
- **THREAD_OPERATION**: General thread operations (create, terminate, suspend)
- **IMAGE_LOAD**: Executable image loading and unloading
- **FILE_MAPPING**: File mapping and section operations

### Usermode Keywords

- **PROCESS_WRITE**: Direct process memory writes
- **PROCESS_READ**: Direct process memory reads
- **NTDLL_OPERATION**: Native library function calls

---

## TextQuest Syscall Targets (RecycledGate)

TextQuest's DLL initializes the RecycledGate layer with 4 target NT syscalls:

```rust
// From textquest-dll/src/syscall/mod.rs::TARGET_HASHES
const TARGET_HASHES: [u32; 4] = [
    hash::NT_PROTECT_VIRTUAL_MEMORY,      // NtProtectVirtualMemory
    hash::NT_SET_CONTEXT_THREAD,          // NtSetContextThread
    hash::NT_GET_CONTEXT_THREAD,          // NtGetContextThread (not in process-boundary ops)
    hash::NT_ALLOCATE_VIRTUAL_MEMORY,     // NtAllocateVirtualMemory
];
```

Each target is resolved at DLL load time via SSN extraction from a fresh ntdll.dll snapshot.

---

## Monitoring & Detection

### Tools That Detect These Operations

| Tool                                                   | Target                                                 | Detection Method                                                                 |
| ------------------------------------------------------ | ------------------------------------------------------ | -------------------------------------------------------------------------------- |
| **Sysmon**                                             | All operations                                         | Event IDs 8 (CreateRemoteThread), 11 (FileCreate for DLL), 17 (SetContextThread) |
| **ETW Kernel Logger**                                  | Syscalls, memory ops                                   | Kernel-mode hooking of API dispatch tables                                       |
| **EDR Products** (Defender, CrowdStrike, Sentinel One) | All operations                                         | API hooking, syscall monitoring, behavioral analysis                             |
| **WinDbg / KernelDebugger**                            | All operations                                         | Manual inspection of kernel structures                                           |
| **Process Monitor**                                    | VirtualAllocEx, WriteProcessMemory, CreateRemoteThread | ETW-based file, registry, and API monitoring                                     |

### How TextQuest Operations Appear

1. **Initial Injection (Orchestrator)**:
   - Process creates `textquest-dll.dll` file to disk (staging)
   - `CreateRemoteThread` + `LoadLibraryW` from orchestrator → target EQ process
   - Sysmon Event 8 (CreateRemoteThread) fires
   - DLL maps into target process memory

2. **DLL Runtime (Injected Process)**:
   - DLL calls `NtAllocateVirtualMemory` via indirect syscall (appears as legitimate ntdll call)
   - DLL calls `NtProtectVirtualMemory` via indirect syscall to change page protection
   - DLL calls `NtSetContextThread` via indirect syscall to set hardware breakpoints
   - PEB module unlinking removes DLL from enumeration (bypasses `Module32FirstW`/`Module32NextW`)

---

## References & Further Reading

- **ETW Keywords**: [Microsoft Docs - Event Tracing Keywords](https://docs.microsoft.com/en-us/windows/win32/etw/about-event-tracing)
- **Threat Intelligence Framework**: [MITRE ATT&CK - T1055 (Process Injection)](https://attack.mitre.org/techniques/T1055/)
- **RecycledGate Pattern**: Indirect syscall invocation via legitimate ntdll gadgets
- **Hardware Breakpoint Hooking**: [VEH (Vectored Exception Handler) + DR0-DR3 registers](https://docs.microsoft.com/en-us/windows/win32/debug/using-hardware-breakpoints)
- **PEB Unlinking**: Manipulation of Process Environment Block module list

---

## Acceptance Criteria ✓

- ✓ All 7+ operations mapped to ETW-TI keywords
- ✓ Each entry notes whether TextQuest uses direct API call, indirect syscall, or ntdll stub
- ✓ Table includes evasion status and detection risk assessment
- ✓ Committed to `docs/etw-ti-keyword-map.md`
