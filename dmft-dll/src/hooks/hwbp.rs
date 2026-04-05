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
        AddVectoredExceptionHandler, GetThreadContext, RemoveVectoredExceptionHandler,
        SetThreadContext, CONTEXT, CONTEXT_FLAGS, EXCEPTION_POINTERS,
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
        use windows::core::s;
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowA, GetWindowThreadProcessId};

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

    /// Schedule `set_breakpoint` to run on EQ's main thread via `QueueUserAPC`.
    ///
    /// The APC callback executes when the main thread enters an alertable wait
    /// (e.g., `SleepEx`, `WaitForSingleObjectEx`, or the window message loop's
    /// `MsgWaitForMultipleObjectsEx`). EQ's main loop enters alertable waits
    /// regularly, so the APC fires within a few frames.
    pub fn set_breakpoint_on_main_thread(slot: HwbpSlot, address: usize) -> Result<(), String> {
        use windows::Win32::System::Threading::{OpenThread, QueueUserAPC, THREAD_SET_CONTEXT};

        let tid = find_main_thread_id()?;

        // Pack slot + address into a static so the APC callback can read them.
        // We use a single AtomicU64: high 32 = slot index, low 32 = unused (address in separate static).
        static APC_SLOT: AtomicU32 = AtomicU32::new(0);
        static APC_ADDR: AtomicUsize = AtomicUsize::new(0);
        static APC_RESULT: AtomicBool = AtomicBool::new(false);
        static APC_DONE: AtomicBool = AtomicBool::new(false);

        APC_SLOT.store(slot as u32, Ordering::Release);
        APC_ADDR.store(address, Ordering::Release);
        APC_DONE.store(false, Ordering::Release);
        APC_RESULT.store(false, Ordering::Release);

        unsafe extern "system" fn apc_callback(_parameter: usize) {
            let slot_idx = APC_SLOT.load(Ordering::Acquire);
            let addr = APC_ADDR.load(Ordering::Acquire);
            let slot = match HwbpSlot::from_index(slot_idx as usize) {
                Some(s) => s,
                None => {
                    APC_DONE.store(true, Ordering::Release);
                    return;
                }
            };
            let ok = set_breakpoint(slot, addr).is_ok();
            APC_RESULT.store(ok, Ordering::Release);
            APC_DONE.store(true, Ordering::Release);
            tracing::info!(
                slot = slot_idx,
                addr = format!("{:#x}", addr),
                success = ok,
                "HWBP set via APC on main thread"
            );
        }

        let thread_handle = unsafe {
            OpenThread(THREAD_SET_CONTEXT, false, tid)
                .map_err(|e| format!("OpenThread({tid}) failed: {e}"))?
        };

        let queued = unsafe { QueueUserAPC(Some(apc_callback), thread_handle, 0) };

        // Handle is Copy in this windows crate version — no explicit close needed.
        let _ = thread_handle;

        if queued == 0 {
            return Err(format!("QueueUserAPC failed for TID {tid}"));
        }

        tracing::info!(
            tid,
            slot = slot as u32,
            addr = format!("{:#x}", address),
            "HWBP APC queued on main thread — will fire on next alertable wait"
        );

        // Don't block waiting — the APC fires asynchronously on the main thread.
        // The VEH handler + slot state are already set up, so once the APC fires
        // and sets the debug register, the hook will start working.
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
                let handled = callback(exception_info as *mut ());
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

    // Use QueueUserAPC to set the HWBP on EQ's main thread. The DLL init
    // runs on a thread pool worker (PoolParty), so GetCurrentThread() would
    // target the wrong thread. The APC fires on the main thread's next
    // alertable wait, setting the debug register in the correct context.
    platform::set_breakpoint_on_main_thread(slot, address)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    SLOTS[idx].active.store(true, Ordering::Release);
    tracing::info!(
        slot = idx,
        addr = format!("{:#x}", address),
        "HWBP registered (APC queued on main thread)"
    );
    Ok(())
}

pub fn unregister(slot: HwbpSlot) -> Result<(), Box<dyn std::error::Error>> {
    let idx = slot as usize;
    SLOTS[idx].active.store(false, Ordering::Release);
    platform::clear_breakpoint(slot).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    SLOTS[idx].address.store(0, Ordering::Release);
    CALLBACKS[idx].store(0, Ordering::Release);
    tracing::info!(slot = idx, "HWBP unregistered");
    Ok(())
}

pub fn remove_all() {
    for (i, slot_state) in SLOTS.iter().enumerate().take(MAX_SLOTS) {
        if slot_state.active.load(Ordering::Acquire) {
            if let Some(slot) = HwbpSlot::from_index(i) {
                if let Err(e) = unregister(slot) {
                    tracing::warn!(slot = i, error = %e, "Failed to unregister HWBP");
                }
            }
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
}
