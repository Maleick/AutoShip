//! PEB module unlinking — removes our DLL from the three module lists in the
//! Process Environment Block (PEB).

use std::ffi::c_void;

#[repr(C)]
struct ListEntry { flink: *mut ListEntry, blink: *mut ListEntry }

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
    _length: u32, _initialized: u32, _ss_handle: *mut c_void,
    in_load_order_module_list: ListEntry,
    in_memory_order_module_list: ListEntry,
    in_initialization_order_module_list: ListEntry,
}

#[repr(C)]
struct Peb { _reserved1: [u8; 0x18], ldr: *mut PebLdrData }

pub fn unlink_module(dll_base: *mut u8) -> Result<(), String> {
    unsafe {
        let peb = read_peb();
        if peb.is_null() { return Err("Failed to read PEB address from TEB".into()); }
        let ldr = (*peb).ldr;
        if ldr.is_null() { return Err("PEB.Ldr is null".into()); }
        let head = &mut (*ldr).in_load_order_module_list as *mut ListEntry;
        let mut current = (*head).flink;
        let mut count = 0u32;
        while current != head && count < 4096 {
            let entry = current as *mut LdrDataTableEntry;
            if (*entry).dll_base == dll_base as *mut c_void {
                unlink_entry(&mut (*entry).in_load_order_links);
                unlink_entry(&mut (*entry).in_memory_order_links);
                unlink_entry(&mut (*entry).in_initialization_order_links);
                return Ok(());
            }
            current = (*current).flink;
            count += 1;
        }
        Err(format!("Module at base {:#x} not found in PEB module lists", dll_base as usize))
    }
}

unsafe fn unlink_entry(entry: *mut ListEntry) {
    let flink = (*entry).flink;
    let blink = (*entry).blink;
    if flink == entry && blink == entry { return; }
    (*blink).flink = flink;
    (*flink).blink = blink;
    (*entry).flink = entry;
    (*entry).blink = entry;
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
        let mut e = ListEntry { flink: std::ptr::null_mut(), blink: std::ptr::null_mut() };
        e.flink = &mut e; e.blink = &mut e;
        unsafe { unlink_entry(&mut e); }
        assert_eq!(e.flink, &mut e as *mut ListEntry);
    }
    #[test]
    fn unlink_entry_removes_from_chain() {
        let mut a = ListEntry { flink: std::ptr::null_mut(), blink: std::ptr::null_mut() };
        let mut b = ListEntry { flink: std::ptr::null_mut(), blink: std::ptr::null_mut() };
        let mut c = ListEntry { flink: std::ptr::null_mut(), blink: std::ptr::null_mut() };
        a.flink = &mut b; a.blink = &mut c;
        b.flink = &mut c; b.blink = &mut a;
        c.flink = &mut a; c.blink = &mut b;
        unsafe { unlink_entry(&mut b); }
        assert_eq!(a.flink, &mut c as *mut ListEntry);
        assert_eq!(c.blink, &mut a as *mut ListEntry);
    }
}
