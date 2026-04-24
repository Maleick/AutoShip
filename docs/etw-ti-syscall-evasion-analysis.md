# ETW-TI Syscall Evasion Analysis

Date: 2026-04-24

Issue: #918

## Executive conclusion

Indirect syscalls do not suppress ETW-TI.

ETW-TI is emitted from kernel execution paths in `ntoskrnl.exe`, after user-mode
code has already entered the kernel. RecycledGate, HellsGate, or a direct
`syscall` instruction can change what user-mode hooks and return-address checks
see, but they still converge on the same kernel implementations for
`NtAllocateVirtualMemory`, `NtProtectVirtualMemory`, `NtSetContextThread`, and
related native APIs. If the operation matches an enabled ETW-TI descriptor, the
event is still eligible to fire.

For TextQuest, the evasion layer should treat indirect syscalls as user-mode
hook and call-stack mitigation only. It should not treat them as an ETW-TI
blind. Any documentation or risk model that says `NtTraceEvent` blinding
suppresses ETW-TI should be read as applying to user-mode ETW providers, not to
the `Microsoft-Windows-Threat-Intelligence` kernel provider.

## Evidence reviewed

- FluxSec's ETW-TI Rust consumer article states that the Threat Intelligence
  provider is emitted from the kernel and cannot be patched out from user mode.
  It also lists ETW-TI tasks for allocation, protection, mapping, APC, context,
  read, write, suspend, and resume operations.
- Undev's reverse-engineering notes identify `EtwThreatIntProvRegHandle`
  references in kernel functions including `EtwTiLogAllocExecVm`,
  `EtwTiLogProtectExecVm`, `EtwTiLogReadWriteVm`, and
  `EtwTiLogSetContextThread`. The same article notes that allocation/protection
  logging is gated by protection masks and, in observed builds, by enabled event
  descriptor keywords.
- Praetorian's kernel-debugger write-up lists `nt!EtwTiLogAllocExecVm`,
  `nt!EtwTiLogProtectExecVm`, `nt!EtwTiLogReadWriteVm`,
  `nt!EtwTiLogSetContextThread`, and related `nt!EtwTiLog*` symbols directly in
  a WinDbg kernel session.
- Rotta's ETW notes summarize the same kernel placement: `EtwTiLogAllocExecVm`
  is called from `MiAllocateVirtualMemory`, and `EtwTiLogProtectExecVm` is
  called from the kernel-side `NtProtectVirtualMemory` path.
- The local TextQuest docs already model the LoadLibrary injection path as
  producing ETW-TI memory events plus a separate kernel image-load event:
  `docs/etw-ti-loadlibrary.md` and `docs/etw-ti-virtualprotect.md`.

References are listed at the end of this document.

## Research questions

### 1. ETW-TI instrumentation is in ntoskrnl.exe. Does it fire even for direct syscall entry?

Yes.

The direct, indirect, and normal ntdll-stub forms are different user-mode entry
styles for reaching the same kernel service. Once execution reaches the kernel,
the memory-manager and thread paths that call `EtwTiLog*` do not depend on
whether user mode arrived through `kernel32`, a clean ntdll stub, a copied stub,
or a recycled `syscall; ret` gadget.

That means direct syscall entry can still produce ETW-TI. The thing that changes
is attribution: stack capture, return address, and "syscall instruction address"
heuristics may distinguish normal ntdll calls, direct syscalls from an injected
module, and indirect syscalls through ntdll. The kernel event itself is not
removed by the indirect path.

### 2. Does NtAllocateVirtualMemory via indirect syscall still emit ALLOCVM_REMOTE?

Yes, when the operation qualifies as a remote allocation event and the ETW-TI
descriptor is enabled.

The important caveat is that `ALLOCVM_REMOTE` is about the target process
relationship and event filters, not about the syscall route:

| Scenario | Expected ETW-TI result | Reason |
| --- | --- | --- |
| Orchestrator allocates memory in `eqgame.exe` for a LoadLibrary path buffer | `ALLOCVM_REMOTE` is expected | Caller and target are different processes. |
| DLL allocates memory inside its own process with `NtAllocateVirtualMemory` | Not `ALLOCVM_REMOTE`; possibly `ALLOCVM_LOCAL` or no ETW-TI event | Caller and target are the same process. Some builds/filter sets only log executable allocation or selected local descriptors. |
| Indirect syscall allocates executable memory in another process | `ALLOCVM_REMOTE` is still expected | Indirect syscall changes user-mode call shape, not the kernel memory-manager event. |

So if a TextQuest capture lacks `ALLOCVM_REMOTE` for the injected DLL's
in-process allocation, that is not evidence that RecycledGate suppressed ETW-TI.
It is evidence that the operation was local, non-executable, not enabled by the
active keyword set, or filtered by the provider's event conditions.

### 3. Does NtProtectVirtualMemory via syscall stub bypass PROTECTVM_REMOTE?

No.

`NtProtectVirtualMemory` reaches the same kernel implementation regardless of
whether it was invoked through `VirtualProtect`, an ntdll stub, a direct syscall,
or an indirect syscall. Public reverse-engineering notes place
`EtwTiLogProtectExecVm` in that kernel path and describe logging when either the
old or new protection includes execute permissions.

As with allocation, local-vs-remote classification matters. A same-process
protect operation should not be described as `PROTECTVM_REMOTE` unless the event
schema/provider configuration reports it that way. For risk modeling, the
defensible conclusion is:

- Remote executable protection changes remain ETW-TI visible.
- Local executable protection changes may be logged or filtered depending on
  descriptor enablement and Windows build/provider configuration.
- The indirect syscall path is not the bypass condition.

## JSONL comparison

This macOS worktree cannot run a protected Windows ETW-TI consumer, so no live
capture was performed here. The comparison below records the expected JSONL
presence/absence based on the local ETW-TI docs and the public kernel evidence
above. It is suitable as the baseline for a Frostreaver capture run.

### Direct LoadLibrary injection path

The project path is effectively `CreateRemoteThread + LoadLibraryW`, though the
same kernel conclusions apply to a `LoadLibraryA` variant.

Expected JSONL:

```jsonl
{"Provider":"Microsoft-Windows-Threat-Intelligence","EventName":"ALLOCVM_REMOTE","CallingProcessName":"textquest.exe","TargetProcessName":"eqgame.exe","Protect":"PAGE_READWRITE"}
{"Provider":"Microsoft-Windows-Threat-Intelligence","EventName":"WRITEVM_REMOTE","CallingProcessName":"textquest.exe","TargetProcessName":"eqgame.exe"}
{"Provider":"Microsoft-Windows-Kernel-Process","EventName":"ImageLoad","ProcessName":"eqgame.exe","ImageName":"textquest_dll.dll"}
```

Interpretation:

- `ALLOCVM_REMOTE` and `WRITEVM_REMOTE` come from the remote staging buffer.
- `ImageLoad` comes from the target process loader mapping the DLL.
- A bare same-process `LoadLibraryA` call is not equivalent to remote
  LoadLibrary injection; it should be evaluated as loader/image mapping
  telemetry rather than as `ALLOCVM_REMOTE`.

### Indirect NtAllocateVirtualMemory path

Expected JSONL for same-process non-executable allocation:

```jsonl
{"Provider":"Microsoft-Windows-Threat-Intelligence","EventName":"ALLOCVM_REMOTE","Presence":"absent","Reason":"same-process allocation is not remote; non-executable allocations may be filtered"}
```

Expected JSONL for remote or executable allocation through an indirect syscall:

```jsonl
{"Provider":"Microsoft-Windows-Threat-Intelligence","EventName":"ALLOCVM_REMOTE","CallingProcessName":"textquest.exe","TargetProcessName":"eqgame.exe","Invocation":"indirect syscall"}
```

Interpretation:

- Absence in the same-process case does not prove syscall evasion.
- Presence in the remote/executable case proves the user-mode route did not
  suppress ETW-TI.

### NtProtectVirtualMemory via syscall stub

Expected JSONL for remote executable protection change:

```jsonl
{"Provider":"Microsoft-Windows-Threat-Intelligence","EventName":"PROTECTVM_REMOTE","CallingProcessName":"textquest.exe","TargetProcessName":"eqgame.exe","ProtectionMask":"PAGE_EXECUTE_READ"}
```

Expected JSONL for same-process protection change:

```jsonl
{"Provider":"Microsoft-Windows-Threat-Intelligence","EventName":"PROTECTVM_LOCAL","Presence":"build/provider dependent","Reason":"local descriptors may be disabled or filtered; indirect syscall is not the deciding factor"}
```

Interpretation:

- A remote protection event should remain visible.
- A missing local protection event must be attributed to provider filtering,
  keyword configuration, or protection-mask conditions before attributing it to
  an evasion technique.

## Recommendation for TextQuest

1. Keep RecycledGate/indirect syscall language scoped to user-mode hook and
   call-stack evasion. Do not claim ETW-TI suppression.
2. Model ETW-TI risk by operation semantics: remote target, executable
   allocation, executable protection transition, remote write, context change,
   APC, and section mapping.
3. Treat the LoadLibrary injection path as high-signal because it combines
   remote allocation, remote write, remote thread creation, and image-load
   telemetry.
4. For future validation on Frostreaver, compare direct and indirect variants by
   event presence plus stack/callsite fields. The expected result is same event
   presence with different attribution, not suppressed ETW-TI.
5. Update any evasion-layer design notes that imply user-mode `NtTraceEvent`
   blinding suppresses ETW-TI. That claim is only safe for user-mode ETW
   emission from the current process.

## Final finding

Indirect syscalls do not suppress ETW-TI telemetry. They can make a syscall look
less suspicious to user-mode hooks and shallow syscall-origin checks, but
kernel-originated ETW-TI events still fire when the kernel operation satisfies
the provider's event criteria.

## References

- FluxSec, "Reading Event Tracing for Windows Threat Intelligence":
  https://fluxsec.red/event-tracing-for-windows-threat-intelligence-rust-consumer
- Undev, "Introduction to Threat Intelligence ETW":
  https://undev.ninja/introduction-to-threat-intelligence-etw/
- Praetorian, "ETW Threat Intelligence and Hardware Breakpoints":
  https://www.praetorian.com/blog/etw-threat-intelligence-and-hardware-breakpoints/
- Rotta, "Interacting with ETW":
  https://www.rotta.rocks/offensive-tool-development/windows-internals/event-tracing-for-windows-etw/interacting-with-etw
- IBM X-Force, "Direct Kernel Object Manipulation (DKOM) Attacks on ETW Providers":
  https://www.ibm.com/think/x-force/direct-kernel-object-manipulation-attacks-etw-providers
- thefLink, "Hunt-Weird-Syscalls":
  https://github.com/thefLink/Hunt-Weird-Syscalls
- Local TextQuest reference:
  `docs/etw-ti-loadlibrary.md`
- Local TextQuest reference:
  `docs/etw-ti-virtualprotect.md`
