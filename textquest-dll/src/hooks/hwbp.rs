//! Hardware breakpoint hooking engine.
//!
//! Uses x86_64 debug registers (DR0-DR3) to set execution breakpoints on target
//! functions. A Vectored Exception Handler (VEH) catches the resulting
//! `EXCEPTION_SINGLE_STEP` and dispatches to the appropriate hook callback.
//!
//! Zero code byte modifications -- invisible to memory integrity scans.
//! No trampolines -- original function bytes are untouched.
//! Only 4 debug registers available -- prioritize the most critical hooks.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub const MAX_SLOTS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum HwbpSlot {
    Dr0 = 0,
    Dr1 = 1,
    Dr2 = 2,
    Dr3 = 3,
}

impl HwbpSlot {
    pub fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Self::Dr0),
            1 => Some(Self::Dr1),
            2 => Some(Self::Dr2),
            3 => Some(Self::Dr3),
            _ => None,
        }
    }
}

pub type HwbpCallback = fn(*mut ()) -> bool;

struct SlotEntry {
    address: AtomicUsize,
    active: AtomicBool,
}

impl SlotEntry {
    const fn new() -> Self {
        Self {
            address: AtomicUsize::new(0),
            active: AtomicBool::new(false),
        }
    }
}

static SLOTS: [SlotEntry; MAX_SLOTS] = [
    SlotEntry::new(),
    SlotEntry::new(),
    SlotEntry::new(),
    SlotEntry::new(),
];

static CALLBACKS: [AtomicUsize; MAX_SLOTS] = [
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
];

static VEH_INSTALLED: AtomicBool = AtomicBool::new(false);
static VEH_HANDLE: AtomicUsize = AtomicUsize::new(0);

#[cfg(windows)]
mod platform {
    use super::*;
    use std::sync::atomic::AtomicU32;
    use windows::Win32::System::Diagnostics::Debug::{
        AddVectoredExceptionHandler, CONTEXT, CONTEXT_FLAGS, EXCEPTION_POINTERS, GetThreadContext,
        RemoveVectoredExceptionHandler, SetThreadContext,
    };
    use windows::Win32::System::Threading::GetCurrentThread;

    const DR7_LOCAL_ENABLE: [u64; 4] = [1 << 0, 1 << 2, 1 << 4, 1 << 6];
    const DR7_COND_LEN_CLEAR: [u64; 4] = [0xF << 16, 0xF << 20, 0xF << 24, 0xF << 28];
    const STATUS_SINGLE_STEP: u32 = 0x80000004;
    const EXCEPTION_CONTINUE_EXECUTION: i32 = -1;
    const EXCEPTION_CONTINUE_SEARCH: i32 = 0;

    /// Thread ID of EQ's main thread (the window message pump).
    /// Set once by `find_main_thread_id`, read by `set_breakpoint_on_main_thread`.
    static MAIN_THREAD_ID: AtomicU32 = AtomicU32::new(0);

    /// Find EQ's main thread by locating the thread that owns the "EverQuest" window.
    pub fn find_main_thread_id() -> Result<u32, String> {
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowA, GetWindowThreadProcessId};
        use windows::core::s;

        let cached = MAIN_THREAD_ID.load(Ordering::Acquire);
        if cached != 0 {
            return Ok(cached);
        }

        let hwnd = unsafe { FindWindowA(s!("_EverQuestwndclass"), None) };
        if hwnd.0 == 0 {
            return Err("EverQuest window not found — character may not be in-world".into());
        }
        let tid = unsafe { GetWindowThreadProcessId(hwnd, None) };
        if tid == 0 {
            return Err("GetWindowThreadProcessId returned 0".into());
        }
        MAIN_THREAD_ID.store(tid, Ordering::Release);
        tracing::info!(tid, "Resolved EQ main thread ID from window handle");
        Ok(tid)
    }

    /// Set a hardware breakpoint on the current thread (caller's context).
    pub fn set_breakpoint(slot: HwbpSlot, address: usize) -> Result<(), String> {
        let idx = slot as usize;
        unsafe {
            let thread = GetCurrentThread();
            let mut ctx: CONTEXT = std::mem::zeroed();
            ctx.ContextFlags = CONTEXT_FLAGS(0x00100010);
            GetThreadContext(thread, &mut ctx)
                .map_err(|e| format!("GetThreadContext failed: {e}"))?;
            match idx {
                0 => ctx.Dr0 = address as u64,
                1 => ctx.Dr1 = address as u64,
                2 => ctx.Dr2 = address as u64,
                3 => ctx.Dr3 = address as u64,
                _ => unreachable!(),
            }
            ctx.Dr7 &= !DR7_COND_LEN_CLEAR[idx];
            ctx.Dr7 |= DR7_LOCAL_ENABLE[idx];
            SetThreadContext(thread, &ctx).map_err(|e| format!("SetThreadContext failed: {e}"))?;
        }
        Ok(())
    }

    /// Set a hardware breakpoint on EQ's main thread via cross-thread
    /// `SetThreadContext`.
    ///
    /// EQ's main loop uses `PeekMessage`/`GetMessage` — not alertable waits —
    /// so `QueueUserAPC` never fires. We fall back to opening the main thread
    /// handle and writing debug registers directly. This is a one-time call
    /// during init, not continuous.
    pub fn set_breakpoint_on_main_thread(slot: HwbpSlot, address: usize) -> Result<(), String> {
        use windows::Win32::System::Threading::{
            OpenThread, ResumeThread, SuspendThread, THREAD_GET_CONTEXT, THREAD_SET_CONTEXT,
            THREAD_SUSPEND_RESUME,
        };

        let tid = find_main_thread_id()?;
        let idx = slot as usize;

        let thread = unsafe {
            OpenThread(
                THREAD_GET_CONTEXT | THREAD_SET_CONTEXT | THREAD_SUSPEND_RESUME,
                false,
                tid,
            )
            .map_err(|e| format!("OpenThread({tid}) failed: {e}"))?
        };

        // Suspend → modify context → resume for a consistent snapshot.
        unsafe {
            SuspendThread(thread);
        }

        let result = unsafe {
            let mut ctx: CONTEXT = std::mem::zeroed();
            ctx.ContextFlags = CONTEXT_FLAGS(0x00100010); // CONTEXT_DEBUG_REGISTERS
            GetThreadContext(thread, &mut ctx)
                .map_err(|e| format!("GetThreadContext(TID {tid}) failed: {e}"))?;

            match idx {
                0 => ctx.Dr0 = address as u64,
                1 => ctx.Dr1 = address as u64,
                2 => ctx.Dr2 = address as u64,
                3 => ctx.Dr3 = address as u64,
                _ => unreachable!(),
            }
            ctx.Dr7 &= !DR7_COND_LEN_CLEAR[idx];
            ctx.Dr7 |= DR7_LOCAL_ENABLE[idx];

            SetThreadContext(thread, &ctx)
                .map_err(|e| format!("SetThreadContext(TID {tid}) failed: {e}"))
        };

        unsafe {
            ResumeThread(thread);
        }

        let _ = thread; // HANDLE is Copy in this crate version

        result?;

        tracing::info!(
            tid,
            slot = idx,
            addr = format!("{:#x}", address),
            "HWBP set on main thread via cross-thread SetThreadContext"
        );
        Ok(())
    }

    pub fn clear_breakpoint(slot: HwbpSlot) -> Result<(), String> {
        let idx = slot as usize;
        unsafe {
            let thread = GetCurrentThread();
            let mut ctx: CONTEXT = std::mem::zeroed();
            ctx.ContextFlags = CONTEXT_FLAGS(0x00100010);
            GetThreadContext(thread, &mut ctx)
                .map_err(|e| format!("GetThreadContext failed: {e}"))?;
            match idx {
                0 => ctx.Dr0 = 0,
                1 => ctx.Dr1 = 0,
                2 => ctx.Dr2 = 0,
                3 => ctx.Dr3 = 0,
                _ => unreachable!(),
            }
            ctx.Dr7 &= !DR7_LOCAL_ENABLE[idx];
            ctx.Dr6 &= !(1u64 << idx);
            SetThreadContext(thread, &ctx).map_err(|e| format!("SetThreadContext failed: {e}"))?;
        }
        Ok(())
    }

    /// VEH handler for HWBP dispatch.
    ///
    /// Lives in `.tq` so it remains executable when sleep obfuscation has
    /// encrypted `.text`. Calls `wake()` before dispatching to callbacks
    /// (which may touch `.text` code) and `sleep()` after.
    #[unsafe(link_section = ".tq")]
    unsafe extern "system" fn veh_handler(exception_info: *mut EXCEPTION_POINTERS) -> i32 {
        let info = unsafe { &*exception_info };
        let record = unsafe { &*info.ExceptionRecord };
        let context = unsafe { &mut *info.ContextRecord };
        if record.ExceptionCode.0 as u32 != STATUS_SINGLE_STEP {
            return EXCEPTION_CONTINUE_SEARCH;
        }
        let rip = context.Rip as usize;
        for i in 0..MAX_SLOTS {
            if !SLOTS[i].active.load(Ordering::Acquire) {
                continue;
            }
            let target = SLOTS[i].address.load(Ordering::Acquire);
            if target == 0 || target != rip {
                continue;
            }
            let cb_ptr = CALLBACKS[i].load(Ordering::Acquire);
            if cb_ptr != 0 {
                let callback: HwbpCallback = unsafe { std::mem::transmute(cb_ptr) };
                context.Dr6 &= !(1u64 << i);
                context.EFlags |= 1 << 16;

                // Decrypt .text before callback can touch any .text code.
                crate::stealth::wake();
                let handled = callback(exception_info as *mut ());
                // Re-encrypt .text after all .text code is done.
                crate::stealth::sleep();

                if handled {
                    return EXCEPTION_CONTINUE_EXECUTION;
                }
            }
        }
        EXCEPTION_CONTINUE_SEARCH
    }

    pub fn install_veh() -> Result<(), String> {
        if VEH_INSTALLED.load(Ordering::Acquire) {
            return Ok(());
        }
        let handle = unsafe { AddVectoredExceptionHandler(1, Some(veh_handler)) };
        if handle.is_null() {
            return Err("AddVectoredExceptionHandler returned null".into());
        }
        VEH_HANDLE.store(handle as usize, Ordering::Release);
        VEH_INSTALLED.store(true, Ordering::Release);
        tracing::info!("VEH handler installed for HWBP dispatch");
        Ok(())
    }

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
        tracing::info!("VEH handler removed");
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;
    pub fn set_breakpoint(_slot: HwbpSlot, _address: usize) -> Result<(), String> {
        tracing::warn!("HWBP set_breakpoint stub (non-Windows)");
        Ok(())
    }
    pub fn clear_breakpoint(_slot: HwbpSlot) -> Result<(), String> {
        tracing::warn!("HWBP clear_breakpoint stub (non-Windows)");
        Ok(())
    }
    pub fn install_veh() -> Result<(), String> {
        tracing::warn!("VEH stub (non-Windows)");
        Ok(())
    }
    pub fn remove_veh() {
        tracing::warn!("VEH removal stub (non-Windows)");
    }
    pub fn set_breakpoint_on_main_thread(_slot: HwbpSlot, _address: usize) -> Result<(), String> {
        tracing::warn!("HWBP set_breakpoint_on_main_thread stub (non-Windows)");
        Ok(())
    }
}

pub fn register(
    slot: HwbpSlot,
    address: usize,
    callback: HwbpCallback,
) -> Result<(), Box<dyn std::error::Error>> {
    let idx = slot as usize;
    platform::install_veh().map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    CALLBACKS[idx].store(callback as usize, Ordering::Release);
    SLOTS[idx].address.store(address, Ordering::Release);

    // Set the HWBP on EQ's main thread via cross-thread SetThreadContext.
    // The DLL init runs on a thread pool worker (PoolParty), so
    // GetCurrentThread() would target the wrong thread. We suspend EQ's
    // main thread, write the debug register, and resume.
    if let Err(error) = platform::set_breakpoint_on_main_thread(slot, address) {
        clear_slot_state(slot);
        if active_count() == 0 {
            platform::remove_veh();
        }
        return Err(error.into());
    }

    SLOTS[idx].active.store(true, Ordering::Release);
    tracing::info!(
        slot = idx,
        addr = format!("{:#x}", address),
        "HWBP registered on main thread"
    );
    Ok(())
}

pub fn unregister(slot: HwbpSlot) -> Result<(), Box<dyn std::error::Error>> {
    let idx = slot as usize;
    SLOTS[idx].active.store(false, Ordering::Release);
    let clear_result = platform::clear_breakpoint(slot);
    clear_slot_state(slot);
    clear_result.map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    tracing::info!(slot = idx, "HWBP unregistered");
    Ok(())
}

pub fn remove_all() {
    for (i, slot_state) in SLOTS.iter().enumerate().take(MAX_SLOTS) {
        let Some(slot) = HwbpSlot::from_index(i) else {
            continue;
        };

        if !slot_state.active.load(Ordering::Acquire) {
            clear_slot_state(slot);
            continue;
        }

        if let Err(e) = unregister(slot) {
            tracing::warn!(slot = i, error = %e, "Failed to unregister HWBP");
        }
    }
    platform::remove_veh();
}

pub fn is_active(slot: HwbpSlot) -> bool {
    SLOTS[slot as usize].active.load(Ordering::Acquire)
}

pub fn get_address(slot: HwbpSlot) -> usize {
    SLOTS[slot as usize].address.load(Ordering::Acquire)
}

pub fn active_count() -> usize {
    (0..MAX_SLOTS)
        .filter(|i| SLOTS[*i].active.load(Ordering::Acquire))
        .count()
}

fn clear_slot_state(slot: HwbpSlot) {
    let idx = slot as usize;
    SLOTS[idx].active.store(false, Ordering::Release);
    SLOTS[idx].address.store(0, Ordering::Release);
    CALLBACKS[idx].store(0, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_from_index_valid() {
        assert_eq!(HwbpSlot::from_index(0), Some(HwbpSlot::Dr0));
        assert_eq!(HwbpSlot::from_index(3), Some(HwbpSlot::Dr3));
        assert_eq!(HwbpSlot::from_index(4), None);
    }

    #[test]
    fn slot_roundtrip() {
        for idx in 0..MAX_SLOTS {
            let slot = HwbpSlot::from_index(idx).unwrap();
            assert_eq!(slot as usize, idx);
        }
    }

    #[test]
    fn max_slots_is_four() {
        assert_eq!(MAX_SLOTS, 4);
    }

    #[test]
    fn register_unregister_stub() {
        #[cfg(not(windows))]
        {
            fn dummy_callback(_: *mut ()) -> bool {
                true
            }
            assert!(register(HwbpSlot::Dr0, 0x12345, dummy_callback).is_ok());
            assert!(is_active(HwbpSlot::Dr0));
            assert_eq!(get_address(HwbpSlot::Dr0), 0x12345);
            assert!(unregister(HwbpSlot::Dr0).is_ok());
            assert!(!is_active(HwbpSlot::Dr0));
            assert_eq!(get_address(HwbpSlot::Dr0), 0);
        }
    }

    #[test]
    fn remove_all_is_safe_when_empty() {
        remove_all();
    }

    #[test]
    fn remove_all_clears_inactive_slot_metadata() {
        fn dummy_callback(_: *mut ()) -> bool {
            true
        }

        SLOTS[HwbpSlot::Dr0 as usize]
            .address
            .store(0x12345, Ordering::Release);
        CALLBACKS[HwbpSlot::Dr0 as usize]
            .store(dummy_callback as *const () as usize, Ordering::Release);
        SLOTS[HwbpSlot::Dr0 as usize]
            .active
            .store(false, Ordering::Release);

        remove_all();

        assert_eq!(get_address(HwbpSlot::Dr0), 0);
        assert_eq!(CALLBACKS[HwbpSlot::Dr0 as usize].load(Ordering::Acquire), 0);
        assert!(!is_active(HwbpSlot::Dr0));
    }
}
