//! Section remapping and VirtualQuery audit for the DLL image.

use windows::Win32::System::Memory::{
    MEM_COMMIT, MEM_IMAGE, MEM_PRIVATE, MEM_RELEASE, MEM_RESERVE, MEMORY_BASIC_INFORMATION,
    PAGE_EXECUTE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY,
    PAGE_PROTECTION_FLAGS, PAGE_READONLY, PAGE_READWRITE, VirtualAlloc, VirtualFree,
    VirtualProtect, VirtualQuery,
};

const PAGE_PROTECTION_BASE_MASK: u32 = 0xff;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SectionRemapAudit {
    pub image_regions: u32,
    pub private_regions: u32,
    pub rwx_regions: u32,
}

pub fn remap_sections(dll_base: *mut u8) -> Result<(), String> {
    if dll_base.is_null() {
        return Err("DLL base is null".into());
    }

    let mut remapped = 0u32;
    unsafe {
        let mut address = dll_base as usize;
        for _ in 0..1024 {
            let mut mbi = MEMORY_BASIC_INFORMATION::default();
            if VirtualQuery(
                Some(address as *const _),
                &mut mbi,
                size_of::<MEMORY_BASIC_INFORMATION>(),
            ) == 0
                || mbi.RegionSize == 0
            {
                break;
            }
            if mbi.AllocationBase != dll_base as *mut _ {
                break;
            }
            if mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE && mbi.RegionSize > 0 {
                if remap_region(address as *mut u8, mbi.RegionSize, mbi.Protect).is_ok() {
                    remapped += 1;
                }
            }
            address = address.saturating_add(mbi.RegionSize);
        }
    }

    let audit = audit_regions(dll_base)?;
    if audit.rwx_regions > 0 {
        return Err(format!(
            "section remap audit found {} execute-write region(s)",
            audit.rwx_regions
        ));
    }
    tracing::debug!(
        sections = audit.image_regions + audit.private_regions,
        image_regions = audit.image_regions,
        private_regions = audit.private_regions,
        remapped,
        "Section remapping VirtualQuery audit complete"
    );
    Ok(())
}

pub fn audit_regions(dll_base: *mut u8) -> Result<SectionRemapAudit, String> {
    if dll_base.is_null() {
        return Err("DLL base is null".into());
    }

    let mut address = dll_base as usize;
    let mut audit = SectionRemapAudit::default();
    for _ in 0..1024 {
        let mut mbi = MEMORY_BASIC_INFORMATION::default();
        // SAFETY: VirtualQuery accepts arbitrary process addresses and writes to
        // the provided MEMORY_BASIC_INFORMATION buffer on success.
        let queried = unsafe {
            VirtualQuery(
                Some(address as *const _),
                &mut mbi,
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if queried == 0 || mbi.RegionSize == 0 {
            break;
        }
        if mbi.AllocationBase != dll_base as *mut _ {
            break;
        }
        if mbi.State == MEM_COMMIT {
            if mbi.Type == MEM_IMAGE {
                audit.image_regions += 1;
            } else if mbi.Type == MEM_PRIVATE {
                audit.private_regions += 1;
            }
            if has_execute_write_protection(mbi.Protect) {
                audit.rwx_regions += 1;
            }
        }
        address = address.saturating_add(mbi.RegionSize);
    }

    Ok(audit)
}

unsafe fn remap_region(
    addr: *mut u8,
    size: usize,
    orig: PAGE_PROTECTION_FLAGS,
) -> Result<(), String> {
    // SAFETY: Allocates temporary scratch storage in the current process.
    let temp = unsafe { VirtualAlloc(None, size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) };
    if temp.is_null() {
        return Err("alloc failed".into());
    }

    let read_protect = readable_protection(orig);
    let mut op = PAGE_PROTECTION_FLAGS::default();
    // SAFETY: Temporarily ensures the DLL region can be copied without leaving it
    // execute-write.
    if unsafe { VirtualProtect(addr as *const _, size, read_protect, &mut op) }.is_err() {
        // SAFETY: Releases the scratch allocation from VirtualAlloc above.
        let _ = unsafe { VirtualFree(temp, 0, MEM_RELEASE) };
        return Err("VP failed".into());
    }
    // SAFETY: Both regions are at least `size` bytes and non-overlapping.
    unsafe { core::ptr::copy_nonoverlapping(addr, temp as *mut u8, size) };

    let mut op2 = PAGE_PROTECTION_FLAGS::default();
    // SAFETY: Enables the copy-back that triggers COW/private backing where the
    // loader mapping supports it.
    if unsafe { VirtualProtect(addr as *const _, size, PAGE_READWRITE, &mut op2) }.is_err() {
        let _ = unsafe { VirtualFree(temp, 0, MEM_RELEASE) };
        return Err("VP RW failed".into());
    }
    // SAFETY: Both regions are at least `size` bytes and non-overlapping.
    unsafe { core::ptr::copy_nonoverlapping(temp as *const u8, addr, size) };

    let mut _t = PAGE_PROTECTION_FLAGS::default();
    let final_protect = sanitized_final_protection(orig);
    // SAFETY: Restores the final section protection, dropping execute-write.
    let _ = unsafe { VirtualProtect(addr as *const _, size, final_protect, &mut _t) };
    let _ = unsafe { VirtualFree(temp, 0, MEM_RELEASE) };
    Ok(())
}

fn readable_protection(protection: PAGE_PROTECTION_FLAGS) -> PAGE_PROTECTION_FLAGS {
    if is_executable(protection) {
        with_base_protection(protection, PAGE_EXECUTE_READ)
    } else {
        with_base_protection(protection, PAGE_READONLY)
    }
}

fn sanitized_final_protection(protection: PAGE_PROTECTION_FLAGS) -> PAGE_PROTECTION_FLAGS {
    if has_execute_write_protection(protection) {
        with_base_protection(protection, PAGE_EXECUTE_READ)
    } else {
        protection
    }
}

fn is_executable(protection: PAGE_PROTECTION_FLAGS) -> bool {
    matches!(
        protection.0 & PAGE_PROTECTION_BASE_MASK,
        x if x == PAGE_EXECUTE.0
            || x == PAGE_EXECUTE_READ.0
            || x == PAGE_EXECUTE_READWRITE.0
            || x == PAGE_EXECUTE_WRITECOPY.0
    )
}

fn has_execute_write_protection(protection: PAGE_PROTECTION_FLAGS) -> bool {
    matches!(
        protection.0 & PAGE_PROTECTION_BASE_MASK,
        x if x == PAGE_EXECUTE_READWRITE.0 || x == PAGE_EXECUTE_WRITECOPY.0
    )
}

fn with_base_protection(
    protection: PAGE_PROTECTION_FLAGS,
    base: PAGE_PROTECTION_FLAGS,
) -> PAGE_PROTECTION_FLAGS {
    PAGE_PROTECTION_FLAGS((protection.0 & !PAGE_PROTECTION_BASE_MASK) | base.0)
}

#[cfg(test)]
mod tests {
    use windows::Win32::System::Memory::{
        PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_GUARD,
        PAGE_PROTECTION_FLAGS, PAGE_READWRITE,
    };

    #[test]
    fn null_base_returns_error() {
        assert!(super::remap_sections(std::ptr::null_mut()).is_err());
    }

    #[test]
    fn final_protection_removes_execute_write() {
        assert_eq!(
            super::sanitized_final_protection(PAGE_EXECUTE_READWRITE),
            PAGE_EXECUTE_READ
        );
        assert_eq!(
            super::sanitized_final_protection(PAGE_EXECUTE_WRITECOPY),
            PAGE_EXECUTE_READ
        );
        assert_eq!(
            super::sanitized_final_protection(PAGE_READWRITE),
            PAGE_READWRITE
        );
    }

    #[test]
    fn final_protection_preserves_modifier_bits() {
        assert_eq!(
            super::sanitized_final_protection(PAGE_PROTECTION_FLAGS(
                PAGE_EXECUTE_READWRITE.0 | PAGE_GUARD.0
            )),
            PAGE_PROTECTION_FLAGS(PAGE_EXECUTE_READ.0 | PAGE_GUARD.0)
        );
    }
}
