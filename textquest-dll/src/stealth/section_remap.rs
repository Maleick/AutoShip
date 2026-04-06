//! Section remapping — triggers copy-on-write to convert MEM_IMAGE to MEM_PRIVATE.

use windows::Win32::System::Memory::{
    MEM_COMMIT, MEM_IMAGE, MEM_RELEASE, MEM_RESERVE, MEMORY_BASIC_INFORMATION,
    PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS, PAGE_READONLY,
    PAGE_READWRITE, VirtualAlloc, VirtualFree, VirtualProtect, VirtualQuery,
};

pub fn remap_sections(dll_base: *mut u8) -> Result<(), String> {
    if dll_base.is_null() { return Err("DLL base is null".into()); }
    unsafe {
        let mut address = dll_base as usize;
        let mut remapped = 0u32;
        for _ in 0..1024 {
            let mut mbi = MEMORY_BASIC_INFORMATION::default();
            if VirtualQuery(Some(address as *const _), &mut mbi, size_of::<MEMORY_BASIC_INFORMATION>()) == 0 { break; }
            if mbi.AllocationBase != dll_base as *mut _ { break; }
            if mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE && mbi.RegionSize > 0 {
                if remap_region(address as *mut u8, mbi.RegionSize, mbi.Protect).is_ok() { remapped += 1; }
            }
            address += mbi.RegionSize;
        }
        tracing::debug!(sections = remapped, "Section remapping complete");
        Ok(())
    }
}

unsafe fn remap_region(addr: *mut u8, size: usize, orig: PAGE_PROTECTION_FLAGS) -> Result<(), String> {
    let temp = VirtualAlloc(None, size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    if temp.is_null() { return Err("alloc failed".into()); }
    let rp = match orig { p if p == PAGE_EXECUTE_READ || p == PAGE_EXECUTE_READWRITE => p, _ => PAGE_READONLY };
    let mut op = PAGE_PROTECTION_FLAGS::default();
    if VirtualProtect(addr as *const _, size, rp, &mut op).is_err() { VirtualFree(temp, 0, MEM_RELEASE).ok(); return Err("VP failed".into()); }
    core::ptr::copy_nonoverlapping(addr, temp as *mut u8, size);
    let mut op2 = PAGE_PROTECTION_FLAGS::default();
    if VirtualProtect(addr as *const _, size, PAGE_READWRITE, &mut op2).is_err() { VirtualFree(temp, 0, MEM_RELEASE).ok(); return Err("VP RW failed".into()); }
    core::ptr::copy_nonoverlapping(temp as *const u8, addr, size);
    let mut _t = PAGE_PROTECTION_FLAGS::default();
    let _ = VirtualProtect(addr as *const _, size, orig, &mut _t);
    VirtualFree(temp, 0, MEM_RELEASE).ok();
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn null_base_returns_error() { assert!(super::remap_sections(std::ptr::null_mut()).is_err()); }
}
