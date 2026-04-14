//! PEB module unlinking — removes our DLL from the three module lists in the
//! Process Environment Block (PEB).
//!
//! This hides the module from loader-backed enumeration paths (for example,
//! "list modules" workflows that first resolve candidates via `PEB_LDR_DATA`)
//! before any export-table inspection occurs.
//!
//! Defensive features:
//! - Pointer validation before dereference (range check)
//! - Idempotency guard (don't double-unlink)
//! - Iteration limit to detect PEB corruption
//! - Graceful error propagation (no panics)

use std::ffi::c_void;

// Valid kernel-mode pointer range for 64-bit Windows
const MIN_VALID_POINTER: usize = 0x1000;
const MAX_VALID_POINTER: usize = 0x7FFFFFFF000;

fn is_valid_pointer(ptr: usize) -> bool {
    ptr >= MIN_VALID_POINTER && ptr <= MAX_VALID_POINTER
}

#[repr(C)]
struct ListEntry {
    flink: *mut ListEntry,
    blink: *mut ListEntry,
}

#[repr(C)]
struct LdrDataTableEntry {
    in_load_order_links: ListEntry,
    in_memory_order_links: ListEntry,
    in_initialization_order_links: ListEntry,
    dll_base: *mut c_void,
    _entry_point: *mut c_void,
    _size_of_image: u32,
}

#[repr(C)]
struct PebLdrData {
    _length: u32,
    _initialized: u32,
    _ss_handle: *mut c_void,
    in_load_order_module_list: ListEntry,
    in_memory_order_module_list: ListEntry,
    in_initialization_order_module_list: ListEntry,
}

#[repr(C)]
struct Peb {
    _reserved1: [u8; 0x18],
    ldr: *mut PebLdrData,
}

pub fn unlink_module(dll_base: *mut u8) -> Result<(), String> {
    unsafe {
        let peb = read_peb();
        if peb.is_null() {
            return Err("Failed to read PEB address from TEB".into());
        }
        let ldr = (*peb).ldr;
        if ldr.is_null() {
            return Err("PEB.Ldr is null".into());
        }

        // Validate PEB_LDR_DATA pointer before dereferencing
        if !is_valid_pointer(ldr as usize) {
            return Err(format!("PEB.Ldr pointer invalid: {:#x}", ldr as usize));
        }

        let head = &mut (*ldr).in_load_order_module_list as *mut ListEntry;
        let mut current = (*head).flink;
        let mut count = 0u32;
        const MAX_ITERATIONS: u32 = 4096;

        while current != head && count < MAX_ITERATIONS {
            // Validate flink/blink pointers before dereferencing
            if !is_valid_pointer(current as usize) {
                return Err(format!(
                    "Corrupted list entry at iteration {}: {:#x}",
                    count, current as usize
                ));
            }

            let entry = current as *mut LdrDataTableEntry;
            if (*entry).dll_base == dll_base as *mut c_void {
                // Found the entry. Check for double-unlink via idempotency guard.
                // An already-unlinked entry points to itself in its primary list link.
                let entry_ptr = entry as *mut ListEntry;
                if (*entry_ptr).flink == entry_ptr && (*entry_ptr).blink == entry_ptr {
                    return Err("Module already unlinked (idempotency guard)".into());
                }

                // Unlink from all three lists
                unlink_entry(&mut (*entry).in_load_order_links)?;
                unlink_entry(&mut (*entry).in_memory_order_links)?;
                unlink_entry(&mut (*entry).in_initialization_order_links)?;

                return Ok(());
            }
            current = (*current).flink;
            count += 1;
        }
        if count >= MAX_ITERATIONS {
            return Err(format!(
                "PEB module list walk exceeded {} iterations; possible corruption",
                MAX_ITERATIONS
            ));
        }
        Err(format!(
            "Module at base {:#x} not found in PEB module lists",
            dll_base as usize
        ))
    }
}

unsafe fn unlink_entry(entry: *mut ListEntry) -> Result<(), String> {
    // Validate entry pointer before dereferencing
    if !is_valid_pointer(entry as usize) {
        return Err(format!("Invalid entry pointer: {:#x}", entry as usize));
    }

    let flink = (*entry).flink;
    let blink = (*entry).blink;

    // Validate flink and blink pointers before dereferencing them
    if !is_valid_pointer(flink as usize) {
        return Err(format!("Invalid flink pointer: {:#x}", flink as usize));
    }
    if !is_valid_pointer(blink as usize) {
        return Err(format!("Invalid blink pointer: {:#x}", blink as usize));
    }

    // Self-referential entry = already unlinked, noop
    if flink == entry && blink == entry {
        return Ok(());
    }

    // Perform the unlink operation
    (*blink).flink = flink;
    (*flink).blink = blink;
    (*entry).flink = entry;
    (*entry).blink = entry;

    Ok(())
}

unsafe fn read_peb() -> *mut Peb {
    let peb: *mut Peb;
    core::arch::asm!("mov {}, gs:[0x60]", out(reg) peb, options(nostack, nomem, preserves_flags));
    peb
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unlink_entry_self_referential_is_noop() {
        let mut e = ListEntry {
            flink: std::ptr::null_mut(),
            blink: std::ptr::null_mut(),
        };
        e.flink = &mut e;
        e.blink = &mut e;
        unsafe {
            unlink_entry(&mut e).expect("self-referential unlink should not fail");
        }
        assert_eq!(e.flink, &mut e as *mut ListEntry);
        assert_eq!(e.blink, &mut e as *mut ListEntry);
    }
    #[test]
    fn unlink_entry_removes_from_chain() {
        let mut a = ListEntry {
            flink: std::ptr::null_mut(),
            blink: std::ptr::null_mut(),
        };
        let mut b = ListEntry {
            flink: std::ptr::null_mut(),
            blink: std::ptr::null_mut(),
        };
        let mut c = ListEntry {
            flink: std::ptr::null_mut(),
            blink: std::ptr::null_mut(),
        };
        a.flink = &mut b;
        a.blink = &mut c;
        b.flink = &mut c;
        b.blink = &mut a;
        c.flink = &mut a;
        c.blink = &mut b;
        unsafe {
            unlink_entry(&mut b).expect("unlink from chain should not fail");
        }
        let a_ptr = &mut a as *mut ListEntry;
        let c_ptr = &mut c as *mut ListEntry;
        assert_eq!(a.flink, c_ptr);
        assert_eq!(c.blink, a_ptr);
    }
}
