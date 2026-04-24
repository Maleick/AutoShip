# ETW-TI Coverage Map

Final synthesis for GitHub issue #920. This map combines the operation taxonomy in
`docs/etw-ti-keyword-map.md` with the per-operation capture notes from the
LoadLibrary, VirtualProtect, and SetThreadContext ETW-TI writeups.

## Evidence Inputs

| Input | Status in this checkout | Notes |
| --- | --- | --- |
| `docs/etw-ti-keyword-map.md` | Present | Maps process-boundary operations to ETW-TI keywords and current TextQuest implementation paths. |
| `docs/etw-ti-loadlibrary.md` | Present | Documents expected LoadLibrary-path `ALLOCVM_REMOTE`, `WRITEVM_REMOTE`, and `ImageLoad` JSONL records. |
| `docs/etw-ti-virtualprotect.md` | Present | Documents `PROTECTVM_REMOTE` field values for TextQuest page-protection transitions. |
| `docs/etw-ti-setthreadcontext.md` | Present | Documents `SETTHREADCONTEXT_REMOTE` field values for HWBP installation. |
| `tests/etw-captures/loadlibrary-path.jsonl` | Missing | Expected capture artifact from #915. |
| `tests/etw-captures/virtualprotect-rwx.jsonl` | Missing | Expected capture artifact from #916. |
| `tests/etw-captures/setthreadcontext-hwbp.jsonl` | Missing | Expected capture artifact from #917. |
| `docs/etw-ti-syscall-evasion-analysis.md` | Missing | Expected analysis artifact from #918; #918 remains open, so indirect-syscall suppression is not proven. |

## Risk Scale

| Risk | Meaning |
| --- | --- |
| HIGH | The operation is directly visible to ETW-TI or adjacent kernel telemetry and is a strong injection or tampering signal. |
| MEDIUM | The operation emits telemetry but needs correlation, unusual flags, or repetition to become a strong signal. |
| LOW | The operation is not used by the current implementation or is not a primary detection surface by itself. |

## Per-Operation Coverage

| Operation | TextQuest path | Fires ETW-TI? | Suppressed by indirect syscall? | Risk | Evidence |
| --- | --- | --- | --- | --- | --- |
| `VirtualAllocEx` | Orchestrator DLL-path staging | Yes. `ALLOCVM_REMOTE` / Event ID 1 is expected for remote allocation into `eqgame.exe`. | No. Current path is a direct Windows API call, not an indirect syscall. | HIGH | `docs/etw-ti-keyword-map.md`, `docs/etw-ti-loadlibrary.md`, expected `tests/etw-captures/loadlibrary-path.jsonl` |
| `WriteProcessMemory` | Orchestrator writes the DLL path into the remote allocation | Yes. `WRITEVM_REMOTE` / Event ID 2 is expected after the allocation. | No. Current path is a direct Windows API call, not an indirect syscall. | HIGH | `docs/etw-ti-keyword-map.md`, `docs/etw-ti-loadlibrary.md`, expected `tests/etw-captures/loadlibrary-path.jsonl` |
| `CreateRemoteThread` + `LoadLibraryW` | Classic DLL load entry point from orchestrator into EQ | Partially. The remote thread itself is covered by thread/image-load telemetry rather than the ETW-TI records shown in the LoadLibrary writeup; the surrounding alloc/write sequence fires ETW-TI. | No. Current path is a direct Windows API call. | HIGH | `docs/etw-ti-keyword-map.md`, `docs/etw-ti-loadlibrary.md`, expected `tests/etw-captures/loadlibrary-path.jsonl` |
| `MapViewOfSection` / image load | Implicit loader behavior after `LoadLibraryW`; section-based injection alternative is not the current path | Current LoadLibrary path produces kernel `ImageLoad` telemetry. A direct remote section-map alternative is expected to hit `MAPVIEW_REMOTE`. | Not proven. No completed #918 comparison exists. | MEDIUM | `docs/etw-ti-keyword-map.md`, `docs/etw-ti-loadlibrary.md` |
| `VirtualProtect` / `NtProtectVirtualMemory` | Injected DLL page-protection cycling and hook setup | Yes. `PROTECTVM_REMOTE` is expected for page-protection changes. TextQuest avoids a persistent RWX state, but the transitions are still visible. | Not proven. The DLL uses RecycledGate-style indirect syscalls, but #918 is still open; assume ETW-TI is not suppressed until proven otherwise. | MEDIUM | `docs/etw-ti-keyword-map.md`, `docs/etw-ti-virtualprotect.md`, expected `tests/etw-captures/virtualprotect-rwx.jsonl` |
| `SetThreadContext` / `NtSetContextThread` | Injected DLL HWBP slot assignment and ETW-blind DR0 setup | Yes. `SETTHREADCONTEXT_REMOTE` is expected for cross-thread debug-register writes and can expose `ContextFlags`, `Dr0`, and `Dr7`. | Not proven. The DLL uses RecycledGate-style indirect syscalls, but #918 is still open; assume ETW-TI is not suppressed until proven otherwise. | HIGH | `docs/etw-ti-keyword-map.md`, `docs/etw-ti-setthreadcontext.md`, expected `tests/etw-captures/setthreadcontext-hwbp.jsonl` |
| `ReadProcessMemory` | Not used by the current DLL-in-process model | Not for current TextQuest behavior. ETW-TI `READVM_REMOTE` would be relevant if remote offset scanning is introduced. | N/A. | LOW | `docs/etw-ti-keyword-map.md` |
| `NtCreateThreadEx` | Not used directly; TextQuest currently uses `CreateRemoteThread` | Not for current TextQuest behavior. It would fall under the same remote-thread risk family if adopted. | N/A. | LOW | `docs/etw-ti-keyword-map.md` |

## High-Risk Mitigations

### Remote allocation and write sequence

The `VirtualAllocEx` plus `WriteProcessMemory` pair is the strongest ETW-TI signal
in the current LoadLibrary path because it produces a tight, cross-process
allocation/write sequence against the EQ process.

Recommended mitigations:

- Treat this path as high-noise and operator-visible in runbooks and release notes.
- Keep the operation gated behind explicit operator action rather than automatic startup behavior.
- Prefer designs that avoid remote process memory staging entirely when a supported integration point is available.
- Preserve local audit logs with target PID, allocation size, and DLL path so expected activity can be distinguished from unexpected injection attempts.

### Remote thread LoadLibrary entry point

`CreateRemoteThread(LoadLibraryW)` is a common DLL-injection pattern even when the
remote-thread creation is correlated through adjacent kernel providers instead of
only ETW-TI.

Recommended mitigations:

- Do not treat `LoadLibraryW` as stealthy; document it as a detectable bootstrap path.
- Keep the bootstrap short-lived and avoid repeated injection attempts that multiply telemetry.
- Surface failed or repeated injection attempts as explicit operator warnings.

### SetThreadContext HWBP installation

`SetThreadContext` is high risk because the capture schema can expose debug
register state. For TextQuest, `Dr0` and `Dr7` are enough to distinguish ETW
blinding and HWBP hook installation from ordinary thread management.

Recommended mitigations:

- Minimize the number of debug-register writes and avoid reinstall loops.
- Log the intended hook slot and target address locally so captured ETW-TI can be reconciled with expected activity.
- Keep the HWBP path disabled unless the operator selected a mode that requires it.
- Do not rely on indirect syscalls as an ETW-TI suppression boundary until #918 is completed with direct capture evidence.

## Conclusion

The current high-risk operations are `VirtualAllocEx`, `WriteProcessMemory`,
`CreateRemoteThread + LoadLibraryW`, and `SetThreadContext`. `VirtualProtect`
remains medium risk because TextQuest avoids RWX at rest, but ETW-TI still records
the protection transitions. Indirect syscall suppression is unresolved because
the #918 analysis artifact is absent; the safe operating assumption is that
kernel ETW-TI still fires.
