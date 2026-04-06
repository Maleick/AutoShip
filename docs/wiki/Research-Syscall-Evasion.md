# Syscall Evasion and NTDLL Unhooking Research

Defensive research into techniques for bypassing userland hooks installed by anti-cheat (AC) or EDR systems. Focuses on three approaches: RecycledGate indirect syscalls, fresh NTDLL mapping, and NTDLL .text section unhooking.

Confidence: Medium (community-sourced techniques, not yet validated against Daybreak AC).

## Background: How Userland Hooks Work

When an AC loads into a process, it overwrites the first bytes of ntdll syscall stubs with a `JMP` instruction that redirects execution into AC monitoring code. The AC inspects arguments, logs the call, and optionally blocks it before forwarding to the real syscall.

### Clean Syscall Stub (x64)

Every `Nt*` function in ntdll follows this pattern:

```
4C 8B D1          mov r10, rcx        ; save first arg (Windows syscall convention)
B8 XX XX 00 00    mov eax, <SSN>      ; System Service Number
0F 05             syscall             ; transition to kernel
C3                ret
```

Total: 11-12 bytes. The SSN at offset +4 is a 16-bit value unique per function per Windows build.

### Hooked Syscall Stub

AC replaces the first 5+ bytes with a near JMP:

```
E9 XX XX XX XX    jmp <ac_handler>    ; 5-byte relative jump
00 00             (remnant padding)
0F 05             syscall             ; may or may not be preserved
C3                ret
```

The `4C 8B D1 B8` prologue is destroyed. HellsGate-family techniques detect this by checking whether byte[0..4] matches the clean prologue.

### Why This Matters for TextQuest

Our DLL calls `VirtualProtect`, `NtSetContextThread` (hardware breakpoint installation), and `NtGetContextThread` from inside eqgame.exe. If the AC hooks these in ntdll, every call we make is intercepted. We need a way to invoke these functions without going through hooked stubs.

---

## 1. RecycledGate

### Mechanism

RecycledGate combines HellsGate SSN resolution with indirect syscall execution through existing ntdll stubs. The key insight: even if the AC hooks the specific function you want to call, other ntdll functions likely remain unhooked. RecycledGate finds a clean `syscall; ret` gadget in any unhooked stub and jumps to it.

**Flow:**

1. Walk the PEB to find ntdll base address (`PEB -> Ldr -> InMemoryOrderModuleList`)
2. Parse ntdll's Export Address Table (EAT) to enumerate all `Nt*` exports
3. For each export, check if the prologue matches `4C 8B D1 B8` (clean) or starts with `E9`/`FF 25` (hooked)
4. **SSN resolution** (HellsGate + HalosGate fallback):
   - If the target stub is clean: read SSN directly from offset +4
   - If hooked: scan neighboring stubs (sorted by SSN). If neighbor at distance N is clean, its SSN +/- N gives the target SSN (HalosGate/TartarusGate)
5. **Find a recycled gate**: scan all clean stubs to find any `0F 05 C3` sequence (the `syscall; ret` gadget). Record its address
6. **Build the trampoline**: instead of placing `syscall` in your own code, emit a `JMP` to the recycled gate address

**Execution sequence:**

```asm
; PrepareSyscall sets these up:
mov r10, rcx              ; first arg
mov eax, <resolved_SSN>   ; from step 4
; Instead of 'syscall' here, we do:
jmp [recycled_gate_addr]  ; jumps to 0F 05 C3 inside ntdll
; syscall executes from within ntdll address range
; ret returns to our caller
```

### How It Differs from SysWhispers3

| Aspect            | SysWhispers3                                        | RecycledGate                             |
| ----------------- | --------------------------------------------------- | ---------------------------------------- |
| SSN source        | Compile-time sorted table or runtime sort           | HellsGate + HalosGate runtime resolution |
| Syscall location  | Finds random `syscall; ret` in ntdll (same concept) | Finds any clean stub's `syscall; ret`    |
| Code generation   | Generates .asm stubs at build time                  | Fully runtime, no static stubs           |
| Static signatures | Generated stubs can be signatured                   | No static assembly artifacts             |
| Flexibility       | Fixed set of functions at compile time              | Any Nt function resolvable at runtime    |

Both ensure the `syscall` instruction executes from within ntdll's address range. The main advantage of RecycledGate is that it is entirely runtime-resolved with no compile-time artifacts.

### Advantages over HellsGate/HalosGate

- **HellsGate** executes `syscall` from your own code section (direct syscall). EDR/AC can detect this by checking that the return address of a syscall is outside ntdll (`0F 05` scanning outside ntdll range)
- **RecycledGate** ensures the `syscall` instruction pointer is inside ntdll when the transition happens, so stack-based return-address checks see a legitimate ntdll address
- The HalosGate fallback handles cases where the target function AND its immediate neighbors are all hooked (scan further out)

### Rust Implementation Guidance

```rust
use core::arch::asm;
use windows_sys::Win32::System::Threading::PEB;

/// Resolved syscall: SSN + address of a clean syscall;ret gadget in ntdll
struct SyscallGate {
    ssn: u32,
    syscall_ret_addr: *const u8, // points to 0F 05 C3 inside ntdll
}

/// Walk PEB -> Ldr -> InMemoryOrderModuleList to find ntdll base
unsafe fn get_ntdll_base() -> *const u8 {
    let peb: *const PEB;
    asm!("mov {}, gs:[0x60]", out(reg) peb, options(nostack, nomem));

    let ldr = (*peb).Ldr;
    let list = &(*ldr).InMemoryOrderModuleList;
    let mut entry = list.Flink; // first = exe, second = ntdll
    entry = (*entry).Flink;     // skip exe

    // LDR_DATA_TABLE_ENTRY.DllBase is at offset 0x30 from InMemoryOrderLinks
    let dll_base = *((entry as *const u8).add(0x30) as *const *const u8);
    dll_base
}

/// Parse EAT, find clean stubs, extract SSN, locate syscall;ret gadget
unsafe fn resolve_syscall(ntdll_base: *const u8, fn_hash: u32) -> Option<SyscallGate> {
    // Parse DOS header -> NT headers -> Optional header -> DataDirectory[0] (Export)
    let dos = ntdll_base as *const IMAGE_DOS_HEADER;
    let nt = ntdll_base.add((*dos).e_lfanew as usize) as *const IMAGE_NT_HEADERS64;
    let export_dir_rva = (*nt).OptionalHeader.DataDirectory[0].VirtualAddress;
    let export_dir = ntdll_base.add(export_dir_rva as usize) as *const IMAGE_EXPORT_DIRECTORY;

    let names = ntdll_base.add((*export_dir).AddressOfNames as usize) as *const u32;
    let functions = ntdll_base.add((*export_dir).AddressOfFunctions as usize) as *const u32;
    let ordinals = ntdll_base.add((*export_dir).AddressOfNameOrdinals as usize) as *const u16;

    let mut gate_addr: *const u8 = core::ptr::null();
    let mut target_ssn: Option<u32> = None;

    for i in 0..(*export_dir).NumberOfNames {
        let name_rva = *names.add(i as usize);
        let name_ptr = ntdll_base.add(name_rva as usize);
        let ordinal = *ordinals.add(i as usize);
        let fn_rva = *functions.add(ordinal as usize);
        let fn_addr = ntdll_base.add(fn_rva as usize);

        // Check if this stub is clean (starts with 4C 8B D1 B8)
        if *fn_addr == 0x4C
            && *fn_addr.add(1) == 0x8B
            && *fn_addr.add(2) == 0xD1
            && *fn_addr.add(3) == 0xB8
        {
            // Extract SSN from bytes 4-5
            let ssn = *fn_addr.add(4) as u32 | ((*fn_addr.add(5) as u32) << 8);

            // If this is our target function (by hash), save the SSN
            if djb2_hash(name_ptr) == fn_hash {
                target_ssn = Some(ssn);
            }

            // Find the syscall;ret gadget (0F 05 C3) in this clean stub
            // Typically at offset +18 from function start, but scan to be safe
            if gate_addr.is_null() {
                for offset in 0..24 {
                    if *fn_addr.add(offset) == 0x0F
                        && *fn_addr.add(offset + 1) == 0x05
                        && *fn_addr.add(offset + 2) == 0xC3
                    {
                        gate_addr = fn_addr.add(offset);
                        break;
                    }
                }
            }
        }
    }

    // HalosGate fallback: if target was hooked, infer SSN from neighbors
    // (implementation: sort clean stubs by SSN, find neighbors, interpolate)

    target_ssn.map(|ssn| SyscallGate {
        ssn,
        syscall_ret_addr: gate_addr,
    })
}

/// Execute the syscall via the recycled gate (indirect syscall)
/// Example: NtProtectVirtualMemory(process, &base, &size, new_prot, &old_prot)
unsafe fn do_syscall(gate: &SyscallGate, args: [usize; 5]) -> i32 {
    let status: i32;
    asm!(
        "mov r10, rcx",
        "mov eax, {ssn:e}",
        // Push 5th arg onto stack (Windows x64: first 4 in rcx,rdx,r8,r9)
        "mov [rsp+0x28], {arg5}",
        "jmp {gate}",
        ssn = in(reg) gate.ssn,
        gate = in(reg) gate.syscall_ret_addr,
        arg5 = in(reg) args[4],
        in("rcx") args[0],
        in("rdx") args[1],
        in("r8") args[2],
        in("r9") args[3],
        lateout("rax") status,
        options(nostack),
    );
    status
}
```

**Key Rust considerations:**

- Use `core::arch::asm!` (stable since Rust 1.59) for inline assembly
- `windows-sys` crate provides raw FFI bindings but does NOT provide PE struct types -- use `winapi` or define them manually
- The `nostack` option is critical to prevent the compiler from inserting stack adjustments that break the syscall ABI
- For the DLL (`cdylib`), this runs inside eqgame.exe so PEB walking gives you eqgame's module list

### Detection Vectors

| Vector                          | Risk       | Notes                                                                                                                                        |
| ------------------------------- | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `syscall` origin check          | **Low**    | Syscall executes from within ntdll -- passes return-address validation                                                                       |
| Call stack analysis             | **Medium** | The `JMP` into ntdll creates an unusual call stack: your DLL -> ntdll (no `call` frame). Sophisticated AC may flag missing ntdll call frames |
| EAT enumeration telemetry       | **Low**    | Reading PE headers in-process is normal; no API calls to intercept                                                                           |
| Byte scanning for `JMP` gadgets | **Low**    | The JMP target is a normal ntdll address, not suspicious                                                                                     |
| Kernel-mode ETW                 | **High**   | Kernel callbacks see the syscall regardless of userland tricks                                                                               |

---

## 2. Fresh NTDLL Mapping

### Mechanism

Load a second, clean copy of ntdll.dll into the process. This copy was never hooked because the AC only hooks the originally loaded copy. You can then either call functions directly from the fresh copy or use it as a reference to extract clean bytes.

**Two sources for the clean copy:**

#### Method A: From KnownDlls (preferred)

Windows caches frequently-used DLLs as section objects in the `\KnownDlls\` object namespace. This is a kernel-maintained cache, so the DLL is the same clean version the OS loaded originally.

```
NtOpenSection(&handle, SECTION_MAP_READ | SECTION_MAP_EXECUTE,
    &OBJECT_ATTRIBUTES { ObjectName: "\KnownDlls\ntdll.dll" })
NtMapViewOfSection(handle, NtCurrentProcess(), &base, ...)
```

**Advantage:** No disk I/O, no file open -- harder to detect via file-access monitoring.

#### Method B: From Disk

```
CreateFileA("C:\Windows\System32\ntdll.dll", GENERIC_READ, FILE_SHARE_READ, ...)
CreateFileMapping(file, NULL, PAGE_READONLY | SEC_IMAGE, ...)
MapViewOfFile(mapping, FILE_MAP_READ, ...)
```

Or using native APIs to avoid Win32 hooks:

```
NtOpenFile(&handle, ..., "C:\Windows\System32\ntdll.dll")
NtCreateSection(&section, ..., handle, SEC_IMAGE)
NtMapViewOfSection(section, NtCurrentProcess(), &base, ...)
```

### Resolving Functions from the Fresh Copy

The fresh copy is mapped at a different base address and is NOT registered in the PEB's InLoadOrderModuleList. You cannot use `GetProcAddress` on it. Instead, parse its EAT manually:

```rust
unsafe fn resolve_from_fresh(fresh_base: *const u8, fn_name: &[u8]) -> *const u8 {
    let dos = fresh_base as *const IMAGE_DOS_HEADER;
    let nt = fresh_base.add((*dos).e_lfanew as usize) as *const IMAGE_NT_HEADERS64;
    let export_rva = (*nt).OptionalHeader.DataDirectory[0].VirtualAddress;
    let export_dir = fresh_base.add(export_rva as usize) as *const IMAGE_EXPORT_DIRECTORY;

    let names = fresh_base.add((*export_dir).AddressOfNames as usize) as *const u32;
    let funcs = fresh_base.add((*export_dir).AddressOfFunctions as usize) as *const u32;
    let ords = fresh_base.add((*export_dir).AddressOfNameOrdinals as usize) as *const u16;

    for i in 0..(*export_dir).NumberOfNames {
        let name_rva = *names.add(i as usize);
        let name_ptr = fresh_base.add(name_rva as usize);
        if compare_bytes(name_ptr, fn_name) {
            let ordinal = *ords.add(i as usize);
            let fn_rva = *funcs.add(ordinal as usize);
            return fresh_base.add(fn_rva as usize);
        }
    }
    core::ptr::null()
}
```

You can then `transmute` the pointer to the appropriate function signature and call it directly. The function in the fresh copy is unhooked, so it executes the real `mov r10,rcx / mov eax,SSN / syscall / ret` sequence.

### Rust Implementation (KnownDlls approach)

```rust
use windows_sys::Win32::Foundation::{HANDLE, UNICODE_STRING, OBJECT_ATTRIBUTES};
use windows_sys::Win32::System::Memory::*;
use core::arch::asm;

// These are Nt* functions we need to bootstrap -- resolve from the
// hooked ntdll first (they're less commonly hooked), or use direct syscalls
type FnNtOpenSection = unsafe extern "system" fn(
    SectionHandle: *mut HANDLE,
    DesiredAccess: u32,
    ObjectAttributes: *const OBJECT_ATTRIBUTES,
) -> i32;

type FnNtMapViewOfSection = unsafe extern "system" fn(
    SectionHandle: HANDLE,
    ProcessHandle: HANDLE,
    BaseAddress: *mut *mut core::ffi::c_void,
    ZeroBits: usize,
    CommitSize: usize,
    SectionOffset: *mut i64,
    ViewSize: *mut usize,
    InheritDisposition: u32,
    AllocationType: u32,
    Win32Protect: u32,
) -> i32;

unsafe fn map_fresh_ntdll() -> *const u8 {
    // Set up UNICODE_STRING for L"\KnownDlls\ntdll.dll"
    let dll_name: [u16; 22] = [
        '\\' as u16, 'K' as u16, 'n' as u16, 'o' as u16, 'w' as u16,
        'n' as u16, 'D' as u16, 'l' as u16, 'l' as u16, 's' as u16,
        '\\' as u16, 'n' as u16, 't' as u16, 'd' as u16, 'l' as u16,
        'l' as u16, '.' as u16, 'd' as u16, 'l' as u16, 'l' as u16,
        0, 0,
    ];

    let us = UNICODE_STRING {
        Length: 40,        // 20 chars * 2 bytes
        MaximumLength: 42,
        Buffer: dll_name.as_ptr() as *mut u16,
    };

    let oa = OBJECT_ATTRIBUTES {
        Length: core::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: 0,
        ObjectName: &us as *const UNICODE_STRING as *mut UNICODE_STRING,
        Attributes: 0x40, // OBJ_CASE_INSENSITIVE
        SecurityDescriptor: core::ptr::null_mut(),
        SecurityQualityOfService: core::ptr::null_mut(),
    };

    let mut section_handle: HANDLE = 0;
    let mut base_address: *mut core::ffi::c_void = core::ptr::null_mut();
    let mut view_size: usize = 0;

    // Use direct/indirect syscall for NtOpenSection to avoid hooks
    // SSN for NtOpenSection varies by Windows build -- resolve at runtime
    let nt_open_section: FnNtOpenSection = /* resolved via RecycledGate or fresh lookup */;

    let status = nt_open_section(
        &mut section_handle,
        SECTION_MAP_READ | SECTION_MAP_EXECUTE,
        &oa,
    );
    if status != 0 { return core::ptr::null(); }

    let nt_map_view: FnNtMapViewOfSection = /* resolved similarly */;

    let status = nt_map_view(
        section_handle,
        -1isize as HANDLE, // NtCurrentProcess()
        &mut base_address,
        0,
        0,
        core::ptr::null_mut(),
        &mut view_size,
        1, // ViewShare
        0,
        PAGE_READONLY,
    );
    if status != 0 { return core::ptr::null(); }

    base_address as *const u8
}
```

### Detection Vectors

| Vector                               | Risk       | Notes                                                                                                                                                  |
| ------------------------------------ | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| VAD enumeration                      | **High**   | The second ntdll mapping appears in the process's Virtual Address Descriptor tree. Kernel-mode AC can enumerate VADs and flag duplicate ntdll mappings |
| NtOpenSection monitoring             | **Medium** | Kernel callback on section object access targeting `\KnownDlls\` is unusual for game processes                                                         |
| NtCreateFile/NtOpenFile on ntdll.dll | **Medium** | Disk-based approach triggers file-access telemetry                                                                                                     |
| Module list discrepancy              | **Low**    | Fresh mapping is NOT in PEB module lists -- but VAD walk reveals it                                                                                    |
| Memory region attributes             | **Medium** | A SEC_IMAGE mapping that doesn't correspond to any loaded module is suspicious                                                                         |

### Mitigation: Unmap After Use

Map the fresh copy, extract what you need (SSNs, clean bytes, or function pointers), then immediately `NtUnmapViewOfSection` to remove the evidence. The VAD entry disappears after unmapping. This is the approach used by the NTDLL-Unhook project.

---

## 3. NTDLL .text Section Unhooking

### Mechanism

Rather than loading a separate copy, overwrite the hooked .text section of the already-loaded ntdll with clean bytes from disk or KnownDlls. After this, the original ntdll functions are restored and all subsequent calls go through clean stubs.

**Step-by-step:**

1. **Obtain clean ntdll bytes** (disk or KnownDlls, as in technique #2)
2. **Find the loaded ntdll base** via `GetModuleHandleA("ntdll.dll")` or PEB walk
3. **Parse PE headers** of both copies to locate the `.text` section:
   - `IMAGE_DOS_HEADER.e_lfanew` -> `IMAGE_NT_HEADERS`
   - Iterate `IMAGE_SECTION_HEADER[]` until `Name == ".text"`
   - Record `VirtualAddress` (RVA) and `VirtualSize`
4. **Change memory protection** on the loaded ntdll's .text to `PAGE_EXECUTE_READWRITE`:
   ```
   VirtualProtect(ntdll_base + text_rva, text_size, PAGE_EXECUTE_READWRITE, &old_prot)
   ```
5. **Copy clean bytes over hooked bytes:**
   ```
   memcpy(ntdll_base + text_rva, fresh_base + text_rva, text_size)
   ```
6. **Restore original protection:**
   ```
   VirtualProtect(ntdll_base + text_rva, text_size, old_prot, &old_prot)
   ```
7. **Unmap the fresh copy** to remove evidence

### Rust Implementation

```rust
use windows_sys::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
use windows_sys::Win32::System::ProcessStatus::{GetModuleInformation, MODULEINFO};

#[repr(C)]
struct ImageDosHeader {
    e_magic: u16,
    _pad: [u8; 58],
    e_lfanew: i32,
}

#[repr(C)]
struct ImageFileHeader {
    machine: u16,
    number_of_sections: u16,
    // ... other fields
}

#[repr(C)]
struct ImageSectionHeader {
    name: [u8; 8],
    virtual_size: u32,
    virtual_address: u32,
    size_of_raw_data: u32,
    pointer_to_raw_data: u32,
    // ... other fields
}

unsafe fn unhook_ntdll() -> bool {
    // 1. Get loaded (hooked) ntdll base
    let hooked_base = GetModuleHandleA(b"ntdll.dll\0".as_ptr()) as *const u8;
    if hooked_base.is_null() { return false; }

    // 2. Map fresh copy from KnownDlls (or disk)
    let fresh_base = map_fresh_ntdll(); // from technique #2
    if fresh_base.is_null() { return false; }

    // 3. Parse hooked ntdll PE to find .text section
    let dos = hooked_base as *const ImageDosHeader;
    let nt_offset = (*dos).e_lfanew as usize;
    // NT headers start at e_lfanew; FileHeader is at +4 (after signature)
    let file_header = hooked_base.add(nt_offset + 4) as *const ImageFileHeader;
    let num_sections = (*file_header).number_of_sections;

    // First section header follows OptionalHeader
    // OptionalHeader offset = nt_offset + 4 + sizeof(FileHeader) = nt_offset + 24
    // OptionalHeader size for x64 = 240 bytes
    let first_section = hooked_base.add(nt_offset + 24 + 240) as *const ImageSectionHeader;

    for i in 0..num_sections as usize {
        let section = &*first_section.add(i);
        if &section.name[..5] == b".text" {
            let text_rva = section.virtual_address as usize;
            let text_size = section.virtual_size as usize;

            let hooked_text = hooked_base.add(text_rva) as *mut u8;
            let clean_text = fresh_base.add(text_rva);

            // 4. Make writable
            let mut old_prot: u32 = 0;
            VirtualProtect(
                hooked_text as *const core::ffi::c_void,
                text_size,
                PAGE_EXECUTE_READWRITE,
                &mut old_prot,
            );

            // 5. Overwrite with clean bytes
            core::ptr::copy_nonoverlapping(clean_text, hooked_text, text_size);

            // 6. Restore protection
            VirtualProtect(
                hooked_text as *const core::ffi::c_void,
                text_size,
                old_prot,
                &mut old_prot,
            );

            break;
        }
    }

    // 7. Unmap fresh copy
    // NtUnmapViewOfSection(NtCurrentProcess(), fresh_base)

    true
}
```

### The Bootstrap Problem

There is a circular dependency: `VirtualProtect` itself may be hooked. Solutions:

1. **Use NtProtectVirtualMemory via direct/indirect syscall** (RecycledGate) to change protections without going through the hooked path
2. **Use native API from the fresh mapping**: resolve `NtProtectVirtualMemory` from the fresh ntdll copy and call it directly -- it is unhooked since it lives in the fresh mapping
3. **Signal Labs approach**: use in-memory disassembly to find where the AC relocated the original function body, then call the relocated version

Option 2 is the most practical for TextQuest: map fresh ntdll, resolve `NtProtectVirtualMemory` from it, use that to make the hooked .text writable, overwrite, restore, unmap.

### Detection Vectors

| Vector                          | Risk       | Notes                                                                                                                                            |
| ------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| VirtualProtect on ntdll .text   | **High**   | Memory protection change on ntdll code section is a strong signal. AC may hook NtProtectVirtualMemory specifically to catch this                 |
| Momentary RWX state             | **High**   | PAGE_EXECUTE_READWRITE on a code section is never normal. Periodic memory attribute scans will flag this                                         |
| AC re-hooking                   | **High**   | After unhooking, the AC may periodically re-install hooks. The unhooking only works until the next re-hook cycle                                 |
| .text hash mismatch             | **Medium** | If the AC baselines .text at load time and periodically re-hashes, restoring clean bytes changes the hash (though to the "expected" clean value) |
| ETW memory write callbacks      | **Medium** | Kernel-mode ETW can observe writes to ntdll .text regardless of userland tricks                                                                  |
| File access on ntdll.dll        | **Medium** | Disk-read approach triggers NtOpenFile telemetry (KnownDlls avoids this)                                                                         |
| VAD during fresh mapping window | **Medium** | Brief dual-ntdll mapping is visible in VAD. Mitigated by unmapping immediately after copy                                                        |

### Daybreak-Specific Considerations

Per the SME research in `anti-detection.md`:

- **Memcheck 1-4**: Server sends an address range, client hashes the region and returns the result. If the server requests a hash of ntdll .text AFTER we unhook, the hash will match the clean on-disk version -- this actually passes. But if the server requests it DURING our brief RWX window, the protection bits are anomalous
- **Inline byte count checks**: These monitor the main game loop body, not ntdll. Unhooking ntdll should not trigger these
- **Process enumeration**: Not relevant to in-process ntdll manipulation

---

## Comparison Matrix

| Criterion               | RecycledGate                                  | Fresh Mapping                     | .text Unhooking             |
| ----------------------- | --------------------------------------------- | --------------------------------- | --------------------------- |
| Complexity              | Medium                                        | Low-Medium                        | Medium                      |
| Leaves artifacts        | None (no extra mappings)                      | VAD entry (unless unmapped)       | Momentary RWX, clean .text  |
| Survives re-hooking     | Yes (resolves at call time)                   | Yes (call from fresh copy)        | No (must re-unhook)         |
| Covers all Nt functions | Yes                                           | Yes                               | Yes (entire .text replaced) |
| Kernel-visible          | Only the syscall itself                       | Section mapping event             | Protection change event     |
| Static signatures       | None (fully runtime)                          | API call pattern                  | API call pattern            |
| Best use case           | Targeted calls (VirtualProtect, NtSetContext) | Reference copy for SSN extraction | One-shot "clean everything" |

## Recommended Approach for TextQuest

**Layer the techniques:**

1. **At DLL load (Early Bird APC)**: map fresh ntdll from KnownDlls, extract SSNs and build a RecycledGate syscall table for critical functions (`NtProtectVirtualMemory`, `NtSetContextThread`, `NtGetContextThread`, `NtAllocateVirtualMemory`). Unmap the fresh copy immediately
2. **For hardware breakpoint installation**: use the RecycledGate table to call `NtSetContextThread` / `NtGetContextThread` via indirect syscalls. These calls never touch hooked stubs
3. **For VirtualProtect needs**: use `NtProtectVirtualMemory` through RecycledGate rather than Win32 `VirtualProtect`. Avoids both the Win32 API hook and the ntdll hook
4. **Do NOT do full .text unhooking**: it is the noisiest approach and the AC may detect the protection change or re-hook. Instead, use per-function indirect syscalls for the specific functions we need

This layered approach minimizes artifacts (no persistent second mapping, no RWX transitions on ntdll) while ensuring our critical calls bypass any userland hooks.

## Open Questions

1. Does Daybreak's AC install ntdll hooks at all, or does it rely purely on kernel callbacks + inline game-loop checks? (Needs Ghidra verification via #343)
2. If the AC does hook ntdll, which specific functions are hooked? (Determines whether we even need these techniques)
3. Does the AC enumerate VADs or just rely on PEB module lists? (Determines risk of fresh mapping)
4. Does the AC monitor `NtSetContextThread` / `NtGetContextThread` specifically? (Critical for hardware breakpoint approach)

## Sources

- [RecycledGate - thefLink/RecycledGate](https://github.com/thefLink/RecycledGate)
- [RefleXXion - hlldz/RefleXXion](https://github.com/hlldz/RefleXXion)
- [NTDLL-Unhook - hwbp/NTDLL-Unhook](https://github.com/hwbp/NTDLL-Unhook)
- [Signal Labs IAT Unhook Sample](https://github.com/Signal-Labs/iat_unhook_sample)
- [SharpNtdllOverwrite - ricardojoserf](https://github.com/ricardojoserf/SharpNtdllOverwrite)
- [Full DLL Unhooking with C++ - ired.team](https://www.ired.team/offensive-security/defense-evasion/how-to-unhook-a-dll-using-c++)
- [Detecting Hooked Syscall Functions - ired.team](https://www.ired.team/offensive-security/defense-evasion/detecting-hooked-syscall-functions)
- [Direct Syscalls vs Indirect Syscalls - RedOps](https://redops.at/en/blog/direct-syscalls-vs-indirect-syscalls)
- [Indirect Syscalls and Hooked SSNs - RedOps](https://redops.at/en/blog/indirect-syscalls-and-hooked-ssns)
- [EDR Evasion Techniques Using Syscalls - HADESS](https://hadess.io/edr-evasion-techniques-using-syscalls/)
- [Bypassing User-Mode Hooks - MDSec](https://www.mdsec.co.uk/2020/12/bypassing-user-mode-hooks-and-direct-invocation-of-system-calls-for-red-teams/)
- [An Introduction to Bypassing User Mode EDR Hooks - MalwareTech](https://malwaretech.com/2023/12/an-introduction-to-bypassing-user-mode-edr-hooks.html)
- [Loader Dev 3: Evading Userspace Hooks - cirosec](https://cirosec.de/en/news/loader-dev-3-evading-userspace-hooks/)
- [Unhooking ntdll.dll in Rust - Maverick](https://medium.com/@maverickcx64/unhooking-ntdll-dll-in-rust-a-beginner-friendly-guide-to-bypassing-edr-hooks-ca113c22ef01)
- [Hells Gate in Rust - fluxsec](https://fluxsec.red/rust-edr-evasion-hells-gate)
- [DInvoke_rs - Kudaes](https://github.com/Kudaes/DInvoke_rs)
- [BouncyGate (RecycledGate in Nim) - eversinc33](https://github.com/eversinc33/BouncyGate)
