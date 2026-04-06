//! PE header erasure — zeroes DOS and NT headers of our loaded DLL.

use windows::Win32::System::Memory::{
    MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_PROTECTION_FLAGS, PAGE_READWRITE,
    VirtualProtect, VirtualQuery,
};

pub fn erase_pe_headers(dll_base: *mut u8) -> Result<(), String> {
    if dll_base.is_null() { return Err("DLL base is null".into()); }
    unsafe {
        let mut mbi = MEMORY_BASIC_INFORMATION::default();
        if VirtualQuery(Some(dll_base as *const _), &mut mbi, size_of::<MEMORY_BASIC_INFORMATION>()) == 0 {
            return Err("VirtualQuery failed".into());
        }
        if mbi.State != MEM_COMMIT { return Err("DLL base not committed".into()); }
        if *(dll_base as *const u16) != 0x5A4D { return Err("Invalid DOS signature".into()); }
        let e_lfanew = *(dll_base.add(0x3C) as *const i32);
        if e_lfanew <= 0 || e_lfanew as usize > mbi.RegionSize { return Err("Invalid e_lfanew".into()); }
        let pe_offset = e_lfanew as usize;
        if *(dll_base.add(pe_offset) as *const u32) != 0x0000_4550 { return Err("Invalid PE sig".into()); }
        let size_of_headers = *(dll_base.add(pe_offset + 84) as *const u32) as usize;
        if size_of_headers < 64 || size_of_headers > 8192 { return Err("Bad SizeOfHeaders".into()); }
        let erase_size = size_of_headers.min(mbi.RegionSize);
        let mut old_protect = PAGE_PROTECTION_FLAGS::default();
        VirtualProtect(dll_base as *const _, erase_size, PAGE_READWRITE, &mut old_protect)
            .map_err(|e| format!("VirtualProtect failed: {}", e))?;
        core::ptr::write_bytes(dll_base, 0, erase_size);
        let mut _tmp = PAGE_PROTECTION_FLAGS::default();
        let _ = VirtualProtect(dll_base as *const _, erase_size, old_protect, &mut _tmp);
        tracing::debug!(erased_bytes = erase_size, "PE headers zeroed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn null_base_returns_error() {
        assert!(super::erase_pe_headers(std::ptr::null_mut()).is_err());
    }
}
