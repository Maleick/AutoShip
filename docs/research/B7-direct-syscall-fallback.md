# B7 — Direct Syscall Fallback Design

**Issue:** #2181  
**Date:** 2026-04-21  
**Status:** Implemented — stub module at `textquest-dll/src/stealth/direct_stub.rs`

---

## Problem Statement

`textquest-dll/src/syscall/` uses **RecycledGate** (indirect syscalls): SSNs extracted
from a fresh ntdll mapped from `\KnownDlls`, executed via a `syscall; ret` gadget found
inside the _real_ loaded ntdll. The gadget address makes the syscall's return address appear
ntdll-originated, defeating EDR/AC call-stack validation.

Two conditions can silently break gadget discovery:

1. **ntdll version skew / ASLR edge case** — `find_syscall_ret_gadget` scans the `.text`
   section for `0F 05 C3`; if ntdll is stripped, obfuscated, or the scan window overruns
   section bounds, no gadget is found.
2. **Usermode hook trampoline at the gadget site** — If an EDR replaces the exact byte
   sequence with a JMP trampoline, our gadget pointer now lands inside a hook prologue,
   producing undefined behavior or a crash on the first syscall.

Without a fallback the init path returns `SyscallError::GadgetNotFound` and the DLL
**does not inject**. This is a hard availability failure.

---

## Design

### Primary path (unchanged)

RecycledGate: SSN from fresh ntdll + gadget at `<ntdll_base + offset>`. No change.

### Fallback path (new)

A private RX page allocated at DLL init that contains per-SSN direct stubs.
Each stub is 16 bytes of machine code generated at runtime.

**Per-stub layout (x64, 16 bytes):**

```
offset  bytes  mnemonic
0       49 89 CA          mov r10, rcx
3       B8 XX XX 00 00    mov eax, <SSN>    (little-endian u16, zero-extended)
8       0F 05             syscall
A       C3                ret
B–F     90 90 90 90 90    nop (padding to 16-byte alignment)
```

The `0F 05` opcode executes directly from our private page — this is the "classic direct
syscall" approach (SysWhispers2/3, HellsGate). The return address will be **inside our
DLL section**, which is detectable by return-address scanners. This is intentionally
the _secondary_ path.

### Invocation policy

```
try RecycledGate(ssn, gadget)
  → if gadget == 0 or gadget lookup fails for this SSN:
      if ssn in BLOCKED_DIRECT_SET: return error (fail closed)
      emit WARN log
      mark ssn as DIRECT_FALLBACK
      try direct_stub(ssn)
```

**Blocked from direct fallback** (fail closed; these are most suspicious from non-ntdll):

| Function                  | Reason                                           |
| ------------------------- | ------------------------------------------------ |
| `NtProtectVirtualMemory`  | Primary target for CFG/shadow-stack validations  |
| `NtAllocateVirtualMemory` | Memory allocation from non-ntdll very suspicious |

Allowed via direct fallback (lower risk profile):

| Function             | Rationale                                    |
| -------------------- | -------------------------------------------- |
| `NtSetContextThread` | Thread context read/write, lower AC priority |
| `NtGetContextThread` | Same                                         |

### Syscall Number Reference (Windows 11 22H2 x64 — build 22621)

These are the SSNs for the four target NT functions. SSNs are **OS-version specific**
and resolved at runtime from ntdll — the values below are for reference/cross-check only.

| NT Function               | SSN (22H2) | SSN (Win10 19041) | Notes                        |
| ------------------------- | ---------- | ----------------- | ---------------------------- |
| `NtSetContextThread`      | 0x00BE     | 0x00BA            | Stable across Win10/11 21H2+ |
| `NtGetContextThread`      | 0x00F5     | 0x00EF            | Slight shift Win10→Win11     |
| `NtAllocateVirtualMemory` | 0x0018     | 0x0018            | Very stable (low ordinal)    |
| `NtProtectVirtualMemory`  | 0x004D     | 0x004D            | Very stable (low ordinal)    |

Source: j00ru/windows-syscalls, verified against live ntdll on Frostreaver (Win11).

Full table excerpt for context (Win11 22H2 build 22621, 64-bit):

```
NtAcceptConnectPort          0x0002
NtAccessCheck                0x0000
NtAllocateVirtualMemory      0x0018   ← target
NtClose                      0x000F
NtCreateSection              0x004A
NtGetContextThread           0x00F5   ← target
NtOpenProcess                0x0026
NtOpenSection                0x0037
NtProtectVirtualMemory       0x004D   ← target (blocked from direct fallback)
NtSetContextThread           0x00BE   ← target
NtWriteVirtualMemory         0x003A
```

---

## Threat Model Analysis

| Scenario                        | Primary (RecycledGate)    | Fallback (Direct)         |
| ------------------------------- | ------------------------- | ------------------------- |
| EDR hook on ntdll stub          | TartarusGate recovers SSN | Same SSN, direct exec     |
| Gadget site hooked/trampolined  | Gadget ptr invalid        | Fallback kicks in         |
| Return-address scanner (stack)  | Looks like ntdll call     | Exposed — our DLL addr    |
| ntdll entirely remapped         | Gadget scan fails         | Fallback kicks in         |
| `NtProtectVirtualMemory` hooked | Gadget still valid        | **Blocked — fail closed** |

**Key invariant:** the fallback is a _resilience_ mechanism, not an evasion upgrade.
If the fallback triggers, it is logged. If it triggers for blocked SSNs, the call fails.
We monitor for fallback activations in production to detect hook pressure.

---

## RX Page Allocation Strategy

The stub page is co-located with `.tq` section intent: always executable, outside
the main `.text` that gets encrypted during sleep cycles. Implementation options:

**Option A (implemented):** `VirtualAlloc(PAGE_EXECUTE_READ)` at DLL init, write stubs
with `VirtualProtect(PAGE_EXECUTE_READWRITE)` → generate stubs → `VirtualProtect(PAGE_EXECUTE_READ)`.
The page is private, not backed by a file — visible as an anonymous RX allocation.

**Option B (future hardening):** Allocate via `NtCreateSection` and map into the
process as a proper named section in `.tq`, making it look like a legitimate DLL section.
This would require touching the PE header manipulation code in `stealth/section_remap.rs`.

Option A is implemented for B7. Option B is tracked as a future hardening step.

---

## Warning Log Format

When the fallback fires, the log entry reads:

```
WARN textquest_dll::syscall::fallback: Direct syscall fallback invoked
  hash=0x<hash_hex> ssn=<ssn_decimal>
  reason="RecycledGate gadget unavailable"
```

In production, recurring fallback warnings on the same SSN indicate sustained EDR hook
pressure on ntdll and should trigger operator review.

---

## References

- matro7sh, _BypassAV §3.3 Direct Syscall Execution_ (SysWhispers2/3, HellsGate)
- j00ru/windows-syscalls: https://j00ru.vexillium.org/syscalls/nt/64/
- SysWhispers2: https://github.com/jthuraisamy/SysWhispers2
- HellsGate: https://github.com/am0nsec/HellsGate
- RecycledGate: https://github.com/thefLink/RecycledGate
- `textquest-dll/src/syscall/{gate,table,hash}.rs` — existing RecycledGate impl
- `docs/wiki/Research-Anti-Detection.md` §M5 evasion architecture table
