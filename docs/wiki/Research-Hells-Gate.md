# Syscall Evasion: HellsGate, HalosGate, and TartarusGate

Defensive research for understanding how game anti-cheat might detect or miss direct syscall invocation patterns. All techniques described here are published open-source research.

## Background: The Windows Syscall Stub

Every `Nt*` / `Zw*` function exported by `ntdll.dll` is a thin stub that transitions from user mode to kernel mode. On x64 Windows 10/11, an **unhooked** stub looks like this:

```
Offset  Bytes              Instruction
------  -----------------  ------------------------------------
+0x00   4C 8B D1           mov r10, rcx
+0x03   B8 XX XX 00 00     mov eax, <SSN>          ; System Service Number
+0x08   F6 04 25 08 03     test byte ptr [7FFE0308h], 1
        FE 7F 01
+0x0F   75 03              jne +3                  ; jump to int 2Eh path
+0x11   0F 05              syscall
+0x13   C3                 ret
+0x14   CD 2E              int 2Eh                 ; legacy path
+0x16   C3                 ret
```

Total stub size: **32 bytes** (0x20). This fixed stride is critical -- stubs are laid out contiguously, meaning `stub[n+1]` starts exactly 32 bytes after `stub[n]`.

The **System Service Number (SSN)** at bytes +0x04 and +0x05 is the index into the System Service Descriptor Table (SSDT). SSNs are assigned incrementally: adjacent `Nt*` exports (sorted by name) have adjacent SSNs. This ordering is stable within a given Windows build but changes between builds.

### What EDR/AV Hooking Looks Like

When an EDR hooks a stub, it overwrites the first bytes with a detour jump:

**Standard inline hook (5-byte overwrite):**

```
+0x00   E9 XX XX XX XX     jmp <EDR_trampoline>    ; replaces mov r10,rcx + first byte of mov eax
+0x05   XX 00 00           (remainder of original mov eax, partially destroyed)
```

**Stealth hook (preserves mov r10,rcx):**

```
+0x00   4C 8B D1           mov r10, rcx            ; preserved
+0x03   E9 XX XX XX XX     jmp <EDR_trampoline>    ; replaces mov eax,SSN
```

The first variant is what HellsGate detects. The second is what TartarusGate was created to handle.

---

## 1. HellsGate

**Origin:** am0nsec/HellsGate (Paul Laing & smelly\_\_vx, 2020)

### Mechanism

HellsGate dynamically resolves SSNs at runtime by walking ntdll's Export Address Table (EAT) and pattern-matching the syscall stub prologue. No hardcoded SSNs are needed.

#### Step 1: Locate ntdll Base Address

Walk the PEB (Process Environment Block) to find ntdll's load address without calling any API:

```
TEB  ->  gs:[0x60]  ->  PEB
PEB  ->  +0x18      ->  PEB_LDR_DATA
LDR  ->  +0x10      ->  InMemoryOrderModuleList (doubly-linked LIST_ENTRY)
```

Each `LDR_DATA_TABLE_ENTRY` in the list has:

- `+0x30` : DllBase (load address)
- `+0x58` : BaseDllName.Length
- `+0x60` : BaseDllName.Buffer (wide string)

Iterate until the name matches `ntdll.dll`.

#### Step 2: Parse the Export Address Table

From the ntdll base, parse the PE headers:

```
DOS Header  ->  e_lfanew           ->  NT Headers
NT Headers  ->  OptionalHeader
            ->  DataDirectory[0]   ->  IMAGE_EXPORT_DIRECTORY (EAT)
```

The EAT provides three parallel arrays:

- `AddressOfNames` : array of RVAs to function name strings
- `AddressOfNameOrdinals` : array of ordinal indices
- `AddressOfFunctions` : array of RVAs to function addresses

To find a function: iterate `AddressOfNames`, compare each name (by hash or string), use the corresponding ordinal to index into `AddressOfFunctions`, and rebase the RVA to get the runtime address.

HellsGate uses **DJB2 hashing** to avoid plaintext function name strings in the binary:

```c
DWORD64 djb2(PBYTE str) {
    DWORD64 dwHash = 0x7734773477347734;
    INT c;
    while (c = *str++)
        dwHash = ((dwHash << 0x5) + dwHash) + c;
    return dwHash;
}
```

#### Step 3: Extract the SSN via Byte Pattern Matching

Once the function address is found, read its first bytes and match the canonical stub:

```c
// The exact pattern HellsGate checks:
if (*((PBYTE)pFunctionAddress + cw)     == 0x4c    // mov r10, rcx (byte 1)
 && *((PBYTE)pFunctionAddress + 1 + cw) == 0x8b    // mov r10, rcx (byte 2)
 && *((PBYTE)pFunctionAddress + 2 + cw) == 0xd1    // mov r10, rcx (byte 3)
 && *((PBYTE)pFunctionAddress + 3 + cw) == 0xb8    // mov eax, imm32 (opcode)
 && *((PBYTE)pFunctionAddress + 6 + cw) == 0x00    // SSN high word == 0
 && *((PBYTE)pFunctionAddress + 7 + cw) == 0x00)   // SSN high word == 0
{
    BYTE high = *((PBYTE)pFunctionAddress + 5 + cw);
    BYTE low  = *((PBYTE)pFunctionAddress + 4 + cw);
    pVxTableEntry->wSystemCall = (high << 8) | low;
    break;
}
```

The SSN is the 16-bit little-endian value at bytes +0x04 and +0x05 (the immediate operand of `mov eax, imm32`). The upper two bytes at +0x06 and +0x07 are always 0x00 0x00 for valid SSNs (they never exceed 16 bits).

#### Step 4: Hook Detection (Abort Conditions)

Before the pattern match, HellsGate checks for premature termination indicators:

```c
// If syscall opcode appears before the prologue -> hooked or corrupted
if (*((PBYTE)pFunctionAddress + cw) == 0x0f
 && *((PBYTE)pFunctionAddress + cw + 1) == 0x05)
    return FALSE;

// If ret appears before the prologue -> hooked or corrupted
if (*((PBYTE)pFunctionAddress + cw) == 0xc3)
    return FALSE;
```

**Limitation:** If the stub is hooked (first bytes replaced with `E9 jmp`), HellsGate simply fails for that function. It has no fallback.

#### Step 5: Execute the Syscall

With the SSN in hand, invoke the syscall directly from user code without ever calling into ntdll:

```asm
mov r10, rcx        ; first arg -> r10 (kernel expects this)
mov eax, <SSN>      ; system service number
syscall             ; transition to kernel
ret
```

### Rust Implementation

```rust
use std::arch::asm;

/// Locate ntdll.dll base address via PEB walk (x64 only)
unsafe fn get_ntdll_base() -> *const u8 {
    let peb: usize;
    let ldr: usize;
    let first_entry: usize;

    asm!(
        "mov {peb}, gs:[0x60]",      // TEB -> PEB
        "mov {ldr}, [{peb} + 0x18]", // PEB -> LDR
        "mov {flink}, [{ldr} + 0x10]", // InMemoryOrderModuleList.Flink
        peb = out(reg) peb,
        ldr = out(reg) ldr,
        flink = out(reg) first_entry,
        options(nostack, nomem),
    );

    // Walk the module list
    let mut entry = first_entry;
    loop {
        let dll_base = *(entry.wrapping_add(0x30) as *const usize);
        let name_len = *(entry.wrapping_add(0x58) as *const u16);
        let name_buf = *(entry.wrapping_add(0x60) as *const usize);

        if name_buf != 0 && name_len > 0 {
            let name_slice = core::slice::from_raw_parts(
                name_buf as *const u16,
                (name_len / 2) as usize,
            );
            // Compare against "ntdll.dll" (case-insensitive wide string)
            if wide_str_eq_ci(name_slice, &NTDLL_WIDE) {
                return dll_base as *const u8;
            }
        }
        entry = *(entry as *const usize); // Flink
        if entry == first_entry {
            break;
        }
    }
    core::ptr::null()
}

/// Check if a syscall stub is clean and extract the SSN
unsafe fn extract_ssn(stub_addr: *const u8) -> Option<u16> {
    let b = |off: usize| -> u8 { *stub_addr.add(off) };

    // Check for hook: first byte is JMP (0xE9) or JCC
    if b(0) == 0xE9 || b(0) == 0xEB {
        return None; // Standard inline hook
    }

    // Match canonical prologue: 4C 8B D1 B8 XX XX 00 00
    if b(0) == 0x4C && b(1) == 0x8B && b(2) == 0xD1 && b(3) == 0xB8
        && b(6) == 0x00 && b(7) == 0x00
    {
        let ssn = (b(4) as u16) | ((b(5) as u16) << 8);
        return Some(ssn);
    }

    // TartarusGate: mov r10,rcx preserved but byte 3 is JMP
    if b(0) == 0x4C && b(1) == 0x8B && b(2) == 0xD1 && b(3) == 0xE9 {
        return None; // Stealth hook detected
    }

    None
}

/// Invoke a syscall with up to 4 register arguments
#[cfg(target_arch = "x86_64")]
unsafe fn do_syscall(ssn: u32, arg1: u64, arg2: u64, arg3: u64, arg4: u64) -> i32 {
    let status: i32;
    asm!(
        "mov r10, rcx",
        "mov eax, {ssn:e}",
        "syscall",
        ssn = in(reg) ssn,
        in("rcx") arg1,
        in("rdx") arg2,
        in("r8") arg3,
        in("r9") arg4,
        lateout("rax") status,
        out("r10") _,
        out("r11") _,   // syscall clobbers r11
        options(nostack),
    );
    status
}
```

---

## 2. HalosGate

**Origin:** Reenz0h / SEKTOR7 (2021)

### Motivation

HellsGate fails when the target stub is hooked because the `4C 8B D1 B8` pattern is destroyed. HalosGate solves this by **scanning neighboring stubs** to find a clean one and inferring the hooked stub's SSN from the neighbor's SSN.

### Mechanism: Neighbor Scanning

The algorithm exploits two facts:

1. Each syscall stub is exactly **32 bytes** (0x20).
2. SSNs are assigned **sequentially** to `Nt*` functions sorted alphabetically within each Windows build.

If `stub[target]` is hooked but `stub[target + 1]` is clean with SSN = N, then `stub[target]` has SSN = N - 1.

#### The Algorithm

```
GetVxTableEntry(pFunctionAddress):

    // Step 1: Try HellsGate (check if stub is clean)
    if bytes at +0..+7 match [4C 8B D1 B8 XX XX 00 00]:
        extract SSN directly
        return SUCCESS

    // Step 2: Stub is hooked -- detect the hook
    if byte at +0 == 0xE9:           // standard JMP hook
        goto neighbor_scan
    if bytes +0..+2 == [4C 8B D1] && byte +3 == 0xE9:  // stealth hook (TartarusGate)
        goto neighbor_scan

    neighbor_scan:
    // Step 3: Scan neighbors in both directions
    for idx in 1..=500:

        // Check DOWN (next stub, higher address)
        neighbor_down = pFunctionAddress + (idx * 32)
        if neighbor_down matches [4C 8B D1 B8 XX XX 00 00]:
            ssn = extract_ssn(neighbor_down)
            target_ssn = ssn - idx    // neighbor is idx positions ahead
            return target_ssn

        // Check UP (previous stub, lower address)
        neighbor_up = pFunctionAddress - (idx * 32)
        if neighbor_up matches [4C 8B D1 B8 XX XX 00 00]:
            ssn = extract_ssn(neighbor_up)
            target_ssn = ssn + idx    // neighbor is idx positions behind
            return target_ssn

    return FAILURE  // all neighbors hooked within scan range
```

#### Key Constants

| Constant          | Value           | Meaning                                                |
| ----------------- | --------------- | ------------------------------------------------------ |
| Stub stride       | 32 bytes (0x20) | Distance between consecutive syscall stubs in ntdll    |
| DOWN              | +32             | Offset to next stub                                    |
| UP                | -32             | Offset to previous stub                                |
| Max scan distance | 500 iterations  | Scans up to 500 stubs in each direction (16,000 bytes) |

#### SSN Calculation

The math is straightforward:

- Neighbor found **below** (at `+idx * 32`): `target_ssn = neighbor_ssn - idx`
- Neighbor found **above** (at `-idx * 32`): `target_ssn = neighbor_ssn + idx`

This works because SSNs are sequential. If `NtAllocateVirtualMemory` has SSN 0x18 and is 3 stubs above our target, our target's SSN is `0x18 + 3 = 0x1B`.

### Why SSN Ordering Holds

Within a single Windows build, Microsoft assigns SSNs to `Nt*`/`Zw*` functions in **alphabetical order of the function name**. The ntdll export table is also sorted alphabetically. Therefore, the n-th exported `Nt*` function has the n-th SSN. This is not guaranteed across builds (SSNs shift when Microsoft adds or removes syscalls), but within a running process, the ordering is stable and deterministic.

### Rust Implementation (HalosGate Extension)

```rust
const STUB_SIZE: usize = 32;
const MAX_SCAN: usize = 500;

/// HalosGate: extract SSN even if the target stub is hooked
unsafe fn halos_gate_ssn(stub_addr: *const u8) -> Option<u16> {
    // Try direct extraction first (HellsGate)
    if let Some(ssn) = extract_ssn(stub_addr) {
        return Some(ssn);
    }

    // Stub is hooked -- verify it looks like a hook
    let b0 = *stub_addr;
    let is_hooked = b0 == 0xE9   // JMP rel32
        || b0 == 0xEB            // JMP rel8
        || (*stub_addr == 0x4C   // stealth hook: 4C 8B D1 E9
            && *stub_addr.add(1) == 0x8B
            && *stub_addr.add(2) == 0xD1
            && *stub_addr.add(3) == 0xE9);

    if !is_hooked {
        return None; // Unknown corruption, bail
    }

    // Neighbor scan: alternate down/up
    for idx in 1..=MAX_SCAN {
        // Scan DOWN (+idx stubs)
        let down = stub_addr.add(idx * STUB_SIZE);
        if let Some(neighbor_ssn) = extract_ssn(down) {
            return Some(neighbor_ssn.wrapping_sub(idx as u16));
        }

        // Scan UP (-idx stubs)
        let up = stub_addr.wrapping_sub(idx * STUB_SIZE);
        if let Some(neighbor_ssn) = extract_ssn(up) {
            return Some(neighbor_ssn.wrapping_add(idx as u16));
        }
    }

    None // All neighbors hooked within range
}
```

---

## 3. TartarusGate

**Origin:** trickster0 (2022), extending HalosGate

### The Problem It Solves

Some EDR products use a **stealth hook** that preserves the first three bytes (`mov r10, rcx` = `4C 8B D1`) and only overwrites byte +3 onward with a JMP:

```
+0x00   4C 8B D1           mov r10, rcx   (preserved)
+0x03   E9 XX XX XX XX     jmp <hook>     (replaces mov eax, SSN)
```

HalosGate only checks byte +0 for `0xE9`, so it misses this pattern and treats the stub as "not hooked but also not matching" -- a silent failure.

### The Fix

TartarusGate adds a **second hook detection check at byte +3**:

```c
// Original HalosGate check:
if (byte[0] == 0xE9) { /* hooked, scan neighbors */ }

// TartarusGate addition:
if (byte[0] == 0x4C && byte[1] == 0x8B && byte[2] == 0xD1 && byte[3] == 0xE9) {
    /* stealth hook detected, scan neighbors */
}
```

Everything else (neighbor scanning, SSN arithmetic, 32-byte stride) is identical to HalosGate.

---

## 4. Detection Vectors (Anti-Cheat Perspective)

These are the methods a game anti-cheat (e.g., EasyAntiCheat, BattlEye, or Daybreak's internal system) can use to detect direct/indirect syscall evasion.

### 4.1 Return Address Validation

The most effective detection: after a syscall returns to user mode, the kernel captures the return address (RIP) in the `KTRAP_FRAME`. Legitimate syscalls return to addresses **inside ntdll.dll** or **win32u.dll**.

```c
// Pseudocode for detection:
bool is_legitimate = (rip >= ntdll_base && rip < ntdll_base + ntdll_size)
                  || (rip >= win32u_base && rip < win32u_base + win32u_size);
if (!is_legitimate) {
    flag_direct_syscall(pid, tid, ssn, rip);
}
```

A direct syscall from HellsGate/HalosGate returns to the **caller's code** (the injected DLL or shellcode), not ntdll. This is trivially detectable.

**Countermeasure:** Indirect syscalls -- jump to the `syscall; ret` gadget inside ntdll rather than executing `syscall` inline. This makes the return address point back into ntdll.

### 4.2 Instrumentation Callbacks (Nirvana)

Windows provides `NtSetInformationProcess` with `ProcessInstrumentationCallback` (info class 40) to register a callback that fires on every kernel-to-user transition. The callback receives the full register context including RIP.

```c
// Anti-cheat sets this early in process init:
PROCESS_INSTRUMENTATION_CALLBACK_INFORMATION info = {
    .Version = 0,
    .Reserved = 0,
    .Callback = &my_syscall_monitor,
};
NtSetInformationProcess(GetCurrentProcess(), 40, &info, sizeof(info));
```

The monitor can check RIP, validate the call stack, and terminate the process if anomalies are found.

### 4.3 Thread Stack Analysis

Walk the thread stack at the point of a suspicious syscall. Legitimate syscalls have a clean stack:

```
kernel32!CreateFileW -> ntdll!NtCreateFile -> syscall
```

Direct syscalls have a short or unusual stack:

```
<unknown_module+0x1234> -> syscall
```

### 4.4 ETW (Event Tracing for Windows)

Kernel-mode ETW providers can log syscall events. The `Microsoft-Windows-Threat-Intelligence` provider specifically captures process injection primitives (`NtAllocateVirtualMemory`, `NtWriteVirtualMemory`, `NtProtectVirtualMemory`, `NtSetContextThread`, etc.) regardless of how they were invoked.

### 4.5 ntdll Integrity Checking

Anti-cheat can compare the in-memory ntdll against the on-disk copy. If stubs have been modified (which HellsGate/HalosGate do NOT do -- they only read), no detection. But techniques that **patch** ntdll (e.g., unhooking by restoring clean stubs from disk) are detectable this way.

### 4.6 Behavioral Profiling

Statistical analysis: which processes normally invoke `NtProtectVirtualMemory` with `PAGE_EXECUTE_READWRITE`? How often? A game client suddenly making direct syscalls to change memory protection stands out.

---

## 5. Application to TextQuest: Hiding VirtualProtect / NtSetContextThread / NtGetContextThread

### Current Exposure

TextQuest's injected DLL (`textquest-dll`) currently uses standard Win32 API calls which pass through ntdll's hooked stubs. If Daybreak ever deploys userland hooks (via their anti-cheat or a third-party AC), these calls become visible.

### What Direct Syscalls Would Protect

| API Call                  | SSN Target               | Why It Matters                                               |
| ------------------------- | ------------------------ | ------------------------------------------------------------ |
| `VirtualProtect`          | `NtProtectVirtualMemory` | Used when installing/removing detour hooks on game functions |
| `NtSetContextThread`      | Direct                   | Thread hijacking for code execution in target process        |
| `NtGetContextThread`      | Direct                   | Reading thread state for injection or debugging              |
| `NtAllocateVirtualMemory` | Direct                   | Allocating executable memory for hook trampolines            |

### Recommended Approach

1. **HalosGate for SSN resolution** -- handles both clean and hooked ntdll. The 32-byte neighbor scan is robust against partial hooking. TartarusGate's stealth-hook check should be included as a minimal addition.

2. **Indirect syscalls, not direct** -- to survive return-address validation, jump to the `syscall; ret` gadget at the appropriate offset within the real ntdll stub rather than executing `syscall` inline. The pattern:

```rust
// Find the syscall;ret gadget inside the real ntdll stub
let syscall_gadget = stub_addr.add(0x12); // offset to 0F 05 C3 in the stub

unsafe {
    asm!(
        "mov r10, rcx",
        "mov eax, {ssn:e}",
        "jmp {gadget}",        // jump INTO ntdll, so RIP points to ntdll on return
        ssn = in(reg) ssn,
        gadget = in(reg) syscall_gadget,
        in("rcx") arg1,
        in("rdx") arg2,
        in("r8") arg3,
        in("r9") arg4,
        options(nostack, noreturn),
    );
}
```

3. **Resolve SSNs once at DLL init** -- walk the EAT during `DllMain` or first game-loop tick, cache all needed SSNs, and reuse them. Do not re-scan on every call.

4. **Hash function names** -- use DJB2 or a compile-time hash to avoid plaintext `"NtProtectVirtualMemory"` strings in the DLL binary.

### What This Does NOT Protect Against

- **Kernel-mode syscall interception** (ETW Threat Intelligence, minifilter drivers, kernel callbacks) -- these see every syscall regardless of invocation method.
- **Behavioral detection** -- unusual patterns of `NtProtectVirtualMemory` with `PAGE_EXECUTE_READWRITE` are suspicious regardless of how the syscall is issued.
- **Module scanning** -- if the anti-cheat enumerates loaded modules and finds `textquest-dll`, no amount of syscall evasion matters.
- **Stack trace analysis** -- even indirect syscalls can be caught if the anti-cheat walks the full call stack and finds frames outside known modules.

---

## References

- [am0nsec/HellsGate](https://github.com/am0nsec/HellsGate) -- Original C implementation
- [0xflux/Rust-Hells-Gate](https://github.com/0xflux/Rust-Hells-Gate) -- Rust implementation with PEB walking and inline asm
- [boku7/AsmHalosGate](https://github.com/boku7/AsmHalosGate) -- x64 Assembly HalosGate implementation
- [trickster0/TartarusGate](https://github.com/trickster0/TartarusGate) -- Extended hook detection for stealth hooks
- [raulm0429/HalosGate-Cpl-C-](https://github.com/raulm0429/HalosGate-Cpl-C-) -- C++ HalosGate with neighbor scanning
- [Alice Climent-Pommeret: Direct Syscalls](https://alice.climent-pommeret.red/posts/direct-syscalls-hells-halos-syswhispers2/) -- Side-by-side comparison
- [Crow's Nest: Direct Syscalls](https://www.crow.rip/nest/mal/dev/inject/syscalls/direct-syscalls) -- Syscall stub layout and EDR hook patterns
- [Palo Alto: Direct Syscall Detection](https://www.paloaltonetworks.com/blog/security-operations/a-deep-dive-into-malicious-direct-syscall-detection/) -- KTRAP_FRAME RIP validation
- [winternl: Detecting Manual Syscalls](https://winternl.com/detecting-manual-syscalls-from-user-mode/) -- Instrumentation callbacks and return address checks
- [100 Days of Red Team: HalosGate](https://www.100daysofredteam.com/p/what-is-halos-gate-and-how-it-enables-red-team-tradecraft) -- HalosGate technique walkthrough
- [trickster0: Tartarus Gate Blog](https://trickster0.github.io/posts/Halo's-Gate-Evolves-to-Tartarus-Gate/) -- Stealth hook detection
- [0xflux: Rust Hell's Gate](https://fluxsec.red/rust-edr-evasion-hells-gate) -- Rust-specific implementation guide
