//! VEH-based general-purpose hooking framework.
//!
//! Provides three complementary hook installation strategies that can replace
//! `retour` for any hook point:
//!
//! 1. **PAGE_GUARD** — marks the target function's page as `PAGE_GUARD`. The
//!    first access triggers `STATUS_GUARD_PAGE_VIOLATION`, the VEH fires,
//!    redirects to the hook, then restores the guard. Not limited to 4 slots
//!    like HWBP.
//!
//! 2. **INT3** — writes a single `0xCC` byte at the hook point (1 byte vs 5+
//!    for a JMP). The VEH catches `EXCEPTION_BREAKPOINT` and dispatches.
//!
//! 3. **PAGE_NOACCESS** — removes execute permission from the target page, the
//!    VEH catches the access violation, executes the hook, single-steps the
//!    original instruction, then re-guards the page.
//!
//! # Hook dispatch
//!
//! All strategies share a single `DISPATCH_TABLE` that maps a target address
//! to a hook function pointer. The VEH handler resolves the faulting address
//! and looks it up in that table.
//!
//! # Performance
//!
//! Exception dispatch adds ~1-5 µs of overhead per call vs retour's inline
//! trampoline. PAGE_GUARD and PAGE_NOACCESS hooks have additional TLB/page
//! fault costs. INT3 hooks have the lowest overhead of the three VEH
//! approaches and are closest to retour in cost.
//!
//! # Thread safety
//!
//! The dispatch table is protected by a `Mutex`. The VEH handler itself uses
//! only `Acquire`/`Release` atomics for fast paths.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::too_many_lines
)]

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

/// Callback signature for VEH hook handlers.
///
/// Receives the raw `EXCEPTION_POINTERS` pointer on Windows (cast to `*mut ()`
/// for cross-platform compatibility). Return `true` to consume the exception
/// (continue execution at the hook's target), `false` to continue searching.
pub type VehHookCallback = fn(*mut ()) -> bool;

/// Hook installation strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehHookKind {
    /// PAGE_GUARD on the target function's page.
    PageGuard,
    /// INT3 (`0xCC`) patched at the target address.
    Int3,
    /// PAGE_NOACCESS on the target function's page.
    PageNoAccess,
}

/// An entry in the dispatch table.
#[derive(Clone)]
struct DispatchEntry {
    kind: VehHookKind,
    callback: VehHookCallback,
    /// The original byte at the target address (INT3 strategy only).
    original_byte: u8,
    /// Whether the hook is currently active.
    active: bool,
}

/// Global hook dispatch table: target address → entry.
static DISPATCH_TABLE: OnceLock<Mutex<HashMap<usize, DispatchEntry>>> = OnceLock::new();

fn dispatch_table() -> &'static Mutex<HashMap<usize, DispatchEntry>> {
    DISPATCH_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[allow(unused_imports)]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static VEH_INSTALLED: AtomicBool = AtomicBool::new(false);
static VEH_HANDLE: AtomicUsize = AtomicUsize::new(0);

// ──────────────────────────────────────────────────────────────────────────────
// Platform: Windows
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(windows)]
mod platform {
    use super::*;

    use windows::Win32::System::{
        Diagnostics::Debug::{
            AddVectoredExceptionHandler, CONTEXT_FLAGS, EXCEPTION_POINTERS,
            RemoveVectoredExceptionHandler,
        },
        Memory::{
            MEMORY_BASIC_INFORMATION, PAGE_EXECUTE_READ, PAGE_GUARD, PAGE_NOACCESS,
            PAGE_PROTECTION_FLAGS, VirtualProtect, VirtualQuery,
        },
    };

    // Exception codes
    const EXCEPTION_BREAKPOINT: u32 = 0x80000003;
    const EXCEPTION_SINGLE_STEP: u32 = 0x80000004;
    const STATUS_GUARD_PAGE_VIOLATION: u32 = 0x80000001;
    const STATUS_ACCESS_VIOLATION: u32 = 0xC0000005;

    const EXCEPTION_CONTINUE_EXECUTION: i32 = -1;
    const EXCEPTION_CONTINUE_SEARCH: i32 = 0;
    const PAGE_MASK: usize = !(0x1000 - 1);

    #[inline]
    fn page_base(addr: usize) -> usize {
        addr & PAGE_MASK
    }

    /// Install the VEH handler (idempotent).
    pub fn install_veh() -> Result<(), String> {
        if VEH_INSTALLED.load(Ordering::Acquire) {
            return Ok(());
        }
        // Priority 1 — run before the HWBP handler so VEH hook exceptions are
        // dispatched first.
        let handle = unsafe { AddVectoredExceptionHandler(1, Some(veh_handler)) };
        if handle.is_null() {
            return Err("AddVectoredExceptionHandler returned null".into());
        }
        VEH_HANDLE.store(handle as usize, Ordering::Release);
        VEH_INSTALLED.store(true, Ordering::Release);
        tracing::info!("VEH general-purpose hook handler installed");
        Ok(())
    }

    /// Remove the VEH handler (idempotent).
    pub fn remove_veh() {
        if !VEH_INSTALLED.load(Ordering::Acquire) {
            return;
        }
        let handle = VEH_HANDLE.swap(0, Ordering::AcqRel);
        if handle != 0 {
            unsafe {
                let h = handle as *mut core::ffi::c_void;
                let _ = RemoveVectoredExceptionHandler(h);
            }
        }
        VEH_INSTALLED.store(false, Ordering::Release);
        tracing::info!("VEH general-purpose hook handler removed");
    }

    /// Write a single byte to `addr` (INT3 install/restore).
    ///
    /// # Safety
    /// Caller must ensure `addr` is valid, mapped, and writable.
    pub unsafe fn write_byte(addr: usize, byte: u8) -> Result<(), String> {
        let ptr = addr as *mut u8;
        // Temporarily make the page writable.
        let mut old = PAGE_PROTECTION_FLAGS(0);
        unsafe {
            VirtualProtect(ptr as *const _, 1, PAGE_EXECUTE_READ, &mut old)
                .map_err(|e| format!("VirtualProtect(RX query) failed: {e}"))?;
            // Restore to RW for the write.
            VirtualProtect(ptr as *const _, 1, PAGE_PROTECTION_FLAGS(0x40), &mut old)
                .map_err(|e| format!("VirtualProtect(RW) failed: {e}"))?;
            ptr.write_volatile(byte);
            // Re-lock to the original protection.
            VirtualProtect(ptr as *const _, 1, old, &mut PAGE_PROTECTION_FLAGS(0))
                .map_err(|e| format!("VirtualProtect(restore) failed: {e}"))?;
        }
        Ok(())
    }

    /// Apply PAGE_GUARD to the page containing `addr`.
    ///
    /// Returns the previous protection flags so they can be restored.
    pub fn apply_page_guard(addr: usize) -> Result<u32, String> {
        let mut mbi = MEMORY_BASIC_INFORMATION::default();
        let result = unsafe {
            VirtualQuery(
                Some(addr as *const _),
                &mut mbi,
                core::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if result == 0 {
            return Err(format!("VirtualQuery failed for {addr:#x}"));
        }
        let old_protect = mbi.Protect;
        let guard_flags = PAGE_PROTECTION_FLAGS(old_protect.0 | PAGE_GUARD.0);
        let mut old = PAGE_PROTECTION_FLAGS(0);
        unsafe {
            VirtualProtect(mbi.BaseAddress, mbi.RegionSize, guard_flags, &mut old)
                .map_err(|e| format!("VirtualProtect(PAGE_GUARD) failed: {e}"))?;
        }
        tracing::debug!(
            addr = format!("{:#x}", addr),
            base = format!("{:#x}", mbi.BaseAddress as usize),
            size = mbi.RegionSize,
            "PAGE_GUARD applied"
        );
        Ok(old_protect.0)
    }

    /// Apply PAGE_NOACCESS to the page containing `addr`.
    pub fn apply_page_noaccess(addr: usize) -> Result<u32, String> {
        let mut mbi = MEMORY_BASIC_INFORMATION::default();
        let result = unsafe {
            VirtualQuery(
                Some(addr as *const _),
                &mut mbi,
                core::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if result == 0 {
            return Err(format!("VirtualQuery failed for {addr:#x}"));
        }
        let old_protect = mbi.Protect;
        let mut old = PAGE_PROTECTION_FLAGS(0);
        unsafe {
            VirtualProtect(mbi.BaseAddress, mbi.RegionSize, PAGE_NOACCESS, &mut old)
                .map_err(|e| format!("VirtualProtect(PAGE_NOACCESS) failed: {e}"))?;
        }
        tracing::debug!(addr = format!("{:#x}", addr), "PAGE_NOACCESS applied");
        Ok(old_protect.0)
    }

    /// Restore a page's protection flags (undo PAGE_GUARD / PAGE_NOACCESS).
    pub fn restore_page_protect(addr: usize, protect: u32) -> Result<(), String> {
        let mut mbi = MEMORY_BASIC_INFORMATION::default();
        let result = unsafe {
            VirtualQuery(
                Some(addr as *const _),
                &mut mbi,
                core::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if result == 0 {
            return Err(format!("VirtualQuery failed for {addr:#x}"));
        }
        let mut old = PAGE_PROTECTION_FLAGS(0);
        unsafe {
            VirtualProtect(
                mbi.BaseAddress,
                mbi.RegionSize,
                PAGE_PROTECTION_FLAGS(protect),
                &mut old,
            )
            .map_err(|e| format!("VirtualProtect(restore) failed: {e}"))?;
        }
        Ok(())
    }

    /// Set the single-step (trap) flag in the exception context so execution
    /// will generate `EXCEPTION_SINGLE_STEP` after the next instruction.
    unsafe fn set_single_step(ctx: *mut windows::Win32::System::Diagnostics::Debug::CONTEXT) {
        // EFlags bit 8 = TF (Trap Flag).
        unsafe { (*ctx).EFlags |= 1 << 8 };
    }

    /// The VEH handler for all three hook strategies.
    ///
    /// Placed in `.tq` so it remains executable during sleep obfuscation.
    #[unsafe(link_section = ".tq")]
    unsafe extern "system" fn veh_handler(exception_info: *mut EXCEPTION_POINTERS) -> i32 {
        let info = unsafe { &*exception_info };
        let record = unsafe { &*info.ExceptionRecord };
        let ctx = unsafe { &mut *info.ContextRecord };
        let code = record.ExceptionCode.0 as u32;

        match code {
            // ── INT3 breakpoint ────────────────────────────────────────────
            EXCEPTION_BREAKPOINT => {
                let fault_addr = ctx.Rip as usize;
                let table = dispatch_table().lock().unwrap_or_else(|p| p.into_inner());
                if let Some(entry) = table.get(&fault_addr) {
                    if entry.active && entry.kind == VehHookKind::Int3 {
                        let cb = entry.callback;
                        drop(table);
                        crate::stealth::wake();
                        let handled = cb(exception_info as *mut ());
                        crate::stealth::sleep();
                        if handled {
                            // Advance RIP past the INT3 byte.
                            ctx.Rip += 1;
                            return EXCEPTION_CONTINUE_EXECUTION;
                        }
                    }
                }
                EXCEPTION_CONTINUE_SEARCH
            }

            // ── PAGE_GUARD violation ───────────────────────────────────────
            STATUS_GUARD_PAGE_VIOLATION => {
                // ExceptionInformation[1] = faulting address.
                if record.NumberParameters < 2 {
                    return EXCEPTION_CONTINUE_SEARCH;
                }
                let fault_addr = record.ExceptionInformation[1];
                let table = dispatch_table().lock().unwrap_or_else(|p| p.into_inner());
                if let Some(entry) = table.get(&fault_addr) {
                    if entry.active && entry.kind == VehHookKind::PageGuard {
                        let cb = entry.callback;
                        drop(table);
                        crate::stealth::wake();
                        let handled = cb(exception_info as *mut ());
                        crate::stealth::sleep();
                        if handled {
                            // Single-step so we can re-apply the guard after
                            // the original instruction executes.
                            unsafe { set_single_step(info.ContextRecord) };
                            return EXCEPTION_CONTINUE_EXECUTION;
                        }
                    }
                }
                let fault_page = page_base(fault_addr);
                let same_guard_page = table.iter().any(|(addr, e)| {
                    e.active && e.kind == VehHookKind::PageGuard && page_base(*addr) == fault_page
                });
                drop(table);
                if same_guard_page {
                    // Consume unrelated faults on guarded pages to avoid
                    // Windows clearing PAGE_GUARD and disabling the hook.
                    unsafe { set_single_step(info.ContextRecord) };
                    return EXCEPTION_CONTINUE_EXECUTION;
                }
                EXCEPTION_CONTINUE_SEARCH
            }

            // ── Single-step (re-arm page traps after they fired) ────────────
            EXCEPTION_SINGLE_STEP => {
                let table = dispatch_table().lock().unwrap_or_else(|p| p.into_inner());
                let guard_targets: Vec<usize> = table
                    .iter()
                    .filter(|(_, e)| e.active && e.kind == VehHookKind::PageGuard)
                    .map(|(addr, _)| *addr)
                    .collect();
                let noaccess_targets: Vec<usize> = table
                    .iter()
                    .filter(|(_, e)| e.active && e.kind == VehHookKind::PageNoAccess)
                    .map(|(addr, _)| *addr)
                    .collect();
                drop(table);
                for addr in guard_targets {
                    let _ = apply_page_guard(addr);
                }
                for addr in noaccess_targets {
                    let _ = apply_page_noaccess(addr);
                }
                EXCEPTION_CONTINUE_EXECUTION
            }

            // ── PAGE_NOACCESS access violation ─────────────────────────────
            STATUS_ACCESS_VIOLATION => {
                if record.NumberParameters < 2 {
                    return EXCEPTION_CONTINUE_SEARCH;
                }
                let fault_addr = record.ExceptionInformation[1];
                let table = dispatch_table().lock().unwrap_or_else(|p| p.into_inner());
                if let Some(entry) = table.get(&fault_addr) {
                    if entry.active && entry.kind == VehHookKind::PageNoAccess {
                        let cb = entry.callback;
                        drop(table);
                        crate::stealth::wake();
                        let handled = cb(exception_info as *mut ());
                        crate::stealth::sleep();
                        if handled {
                            // Temporarily restore RX so the original
                            // instruction can execute, then re-guard on the
                            // following single-step.
                            let _ = restore_page_protect(fault_addr, PAGE_EXECUTE_READ.0);
                            unsafe { set_single_step(info.ContextRecord) };
                            return EXCEPTION_CONTINUE_EXECUTION;
                        }
                    }
                }
                let fault_page = page_base(fault_addr);
                let same_noaccess_page = table.iter().any(|(addr, e)| {
                    e.active
                        && e.kind == VehHookKind::PageNoAccess
                        && page_base(*addr) == fault_page
                });
                drop(table);
                if same_noaccess_page {
                    // This access hit a PAGE_NOACCESS-trapped page but not the
                    // exact target address. Temporarily restore execute/read
                    // and single-step so the trap can be re-armed.
                    let _ = restore_page_protect(fault_addr, PAGE_EXECUTE_READ.0);
                    unsafe { set_single_step(info.ContextRecord) };
                    return EXCEPTION_CONTINUE_EXECUTION;
                }
                EXCEPTION_CONTINUE_SEARCH
            }

            _ => EXCEPTION_CONTINUE_SEARCH,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Platform: non-Windows stubs
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(not(windows))]
mod platform {
    pub fn install_veh() -> Result<(), String> {
        tracing::warn!("VEH general-purpose hook: stub (non-Windows)");
        Ok(())
    }

    pub fn remove_veh() {
        tracing::warn!("VEH general-purpose hook remove: stub (non-Windows)");
    }

    pub unsafe fn write_byte(_addr: usize, _byte: u8) -> Result<(), String> {
        tracing::warn!("write_byte: stub (non-Windows)");
        Ok(())
    }

    pub fn apply_page_guard(_addr: usize) -> Result<u32, String> {
        tracing::warn!("apply_page_guard: stub (non-Windows)");
        Ok(0)
    }

    pub fn apply_page_noaccess(_addr: usize) -> Result<u32, String> {
        tracing::warn!("apply_page_noaccess: stub (non-Windows)");
        Ok(0)
    }

    pub fn restore_page_protect(_addr: usize, _protect: u32) -> Result<(), String> {
        tracing::warn!("restore_page_protect: stub (non-Windows)");
        Ok(())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Public API
// ──────────────────────────────────────────────────────────────────────────────

/// Install a hook at `target_addr` using the specified strategy.
///
/// The `callback` receives a raw `*mut ()` pointing to the Windows
/// `EXCEPTION_POINTERS` structure on exception. Return `true` to claim
/// the exception (hook handled), `false` to pass it on.
///
/// # Errors
///
/// Returns an error string if the VEH handler could not be registered or if
/// the page protection operation failed.
pub fn install(
    target_addr: usize,
    kind: VehHookKind,
    callback: VehHookCallback,
) -> Result<(), String> {
    platform::install_veh()?;

    let original_byte = if kind == VehHookKind::Int3 {
        // Read the byte that will be overwritten.
        let byte = unsafe { (target_addr as *const u8).read_volatile() };
        // Write the INT3.
        unsafe { platform::write_byte(target_addr, 0xCC)? };
        byte
    } else {
        0
    };

    let protect_result = match kind {
        VehHookKind::PageGuard => platform::apply_page_guard(target_addr),
        VehHookKind::PageNoAccess => platform::apply_page_noaccess(target_addr),
        VehHookKind::Int3 => Ok(0),
    };

    if let Err(e) = protect_result {
        // If INT3 was already written, restore it before bailing.
        if kind == VehHookKind::Int3 {
            let _ = unsafe { platform::write_byte(target_addr, original_byte) };
        }
        return Err(e);
    }

    let mut table = dispatch_table().lock().unwrap_or_else(|p| p.into_inner());
    table.insert(
        target_addr,
        DispatchEntry {
            kind,
            callback,
            original_byte,
            active: true,
        },
    );
    tracing::info!(
        addr = format!("{:#x}", target_addr),
        strategy = format!("{:?}", kind),
        "VEH hook installed"
    );
    Ok(())
}

/// Remove a hook previously installed with [`install`].
///
/// For INT3 hooks, restores the original byte. For PAGE_GUARD / PAGE_NOACCESS
/// hooks, restores `PAGE_EXECUTE_READ`.
pub fn remove(target_addr: usize) -> Result<(), String> {
    let mut table = dispatch_table().lock().unwrap_or_else(|p| p.into_inner());
    let Some(entry) = table.remove(&target_addr) else {
        return Ok(());
    };

    match entry.kind {
        VehHookKind::Int3 => {
            unsafe { platform::write_byte(target_addr, entry.original_byte)? };
        }
        VehHookKind::PageGuard | VehHookKind::PageNoAccess => {
            // Restore to standard PAGE_EXECUTE_READ (0x20).
            platform::restore_page_protect(target_addr, 0x20)?;
        }
    }

    tracing::info!(
        addr = format!("{:#x}", target_addr),
        strategy = format!("{:?}", entry.kind),
        "VEH hook removed"
    );

    // If no hooks remain, remove the VEH handler.
    if table.is_empty() {
        drop(table);
        platform::remove_veh();
    }
    Ok(())
}

/// Remove all installed VEH hooks.
pub fn remove_all() {
    let addresses: Vec<usize> = {
        let table = dispatch_table().lock().unwrap_or_else(|p| p.into_inner());
        table.keys().copied().collect()
    };
    for addr in addresses {
        if let Err(e) = remove(addr) {
            tracing::warn!(addr = format!("{:#x}", addr), error = %e, "VEH hook remove failed");
        }
    }
    platform::remove_veh();
}

/// Returns `true` if a hook is currently installed at `target_addr`.
pub fn is_installed(target_addr: usize) -> bool {
    dispatch_table()
        .lock()
        .map(|t| t.contains_key(&target_addr))
        .unwrap_or(false)
}

/// Returns the number of currently active VEH hooks.
pub fn hook_count() -> usize {
    dispatch_table().lock().map(|t| t.len()).unwrap_or(0)
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::*;

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn noop_callback(_: *mut ()) -> bool {
        true
    }

    #[test]
    fn install_and_is_installed() {
        let _g = test_guard();
        remove_all();

        // On non-Windows, the page-guard path is a stub so install() succeeds.
        assert!(install(0xDEAD_BEEF, VehHookKind::PageGuard, noop_callback).is_ok());
        assert!(is_installed(0xDEAD_BEEF));
        assert_eq!(hook_count(), 1);
        remove_all();
    }

    #[test]
    fn remove_clears_entry() {
        let _g = test_guard();
        remove_all();

        install(0x1000, VehHookKind::PageGuard, noop_callback).unwrap();
        assert!(is_installed(0x1000));
        remove(0x1000).unwrap();
        assert!(!is_installed(0x1000));
        assert_eq!(hook_count(), 0);
    }

    #[test]
    fn remove_unknown_addr_is_noop() {
        let _g = test_guard();
        remove_all();
        // Removing a non-existent address must not error.
        assert!(remove(0xFFFF_0000).is_ok());
    }

    #[test]
    fn remove_all_is_idempotent() {
        let _g = test_guard();
        remove_all();
        remove_all();
        remove_all();
        assert_eq!(hook_count(), 0);
    }

    #[test]
    fn multiple_hooks_tracked_independently() {
        let _g = test_guard();
        remove_all();

        install(0x2000, VehHookKind::PageGuard, noop_callback).unwrap();
        install(0x3000, VehHookKind::PageNoAccess, noop_callback).unwrap();

        // INT3 strategy reads a byte from the target address. On non-Windows
        // that read is from our own stack/heap — fine for test purposes.
        let mut dummy_target: u8 = 0x55;
        let addr = &mut dummy_target as *mut u8 as usize;
        install(addr, VehHookKind::Int3, noop_callback).unwrap();

        assert_eq!(hook_count(), 3);
        assert!(is_installed(0x2000));
        assert!(is_installed(0x3000));
        assert!(is_installed(addr));

        remove_all();
        assert_eq!(hook_count(), 0);
    }

    #[test]
    fn hook_count_reflects_table_size() {
        let _g = test_guard();
        remove_all();
        assert_eq!(hook_count(), 0);

        install(0x5000, VehHookKind::PageGuard, noop_callback).unwrap();
        assert_eq!(hook_count(), 1);

        install(0x6000, VehHookKind::PageGuard, noop_callback).unwrap();
        assert_eq!(hook_count(), 2);

        remove(0x5000).unwrap();
        assert_eq!(hook_count(), 1);

        remove_all();
        assert_eq!(hook_count(), 0);
    }

    #[test]
    fn veh_hook_kind_debug_formatting() {
        assert_eq!(format!("{:?}", VehHookKind::PageGuard), "PageGuard");
        assert_eq!(format!("{:?}", VehHookKind::Int3), "Int3");
        assert_eq!(format!("{:?}", VehHookKind::PageNoAccess), "PageNoAccess");
    }
}
