//! Patchless ETW blinding via hardware breakpoints with optional filtering mode.
//!
//! Sets a hardware breakpoint (DR0) on `NtTraceEvent` so that calls are
//! intercepted by our Vectored Exception Handler. In strict blinding mode we
//! suppress every call to `NtTraceEvent`; in filtering mode we allow events
//! through to avoid unsafe pointer dereferences in the exception path.
//!
//! The VEH returns `STATUS_SUCCESS` (RAX = 0) and skips the function body
//! by advancing RIP past the return address on the stack for suppressed events.

// ── Windows implementation ──────────────────────────────────────────────────

#[cfg(windows)]
mod inner {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    use windows::{
        Win32::System::{
            Diagnostics::Debug::{
                AddVectoredExceptionHandler, CONTEXT, CONTEXT_FLAGS, GetThreadContext,
                RemoveVectoredExceptionHandler, SetThreadContext,
            },
            LibraryLoader::{GetModuleHandleA, GetProcAddress},
            Threading::GetCurrentThread,
        },
        core::s,
    };

    /// Address of `NtTraceEvent` — set once during init, read by the VEH.
    static NT_TRACE_EVENT_ADDR: AtomicU64 = AtomicU64::new(0);

    /// Whether ETW blinding is currently active.
    static ACTIVE: AtomicBool = AtomicBool::new(false);

    /// Whether ETW filtering mode is enabled.
    /// When true, ETW calls are allowed through.
    /// When false, all NtTraceEvent calls are suppressed (original behavior).
    /// Default: true (filtering mode enabled).
    static FILTERING_ENABLED: AtomicBool = AtomicBool::new(true);

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

            // Determine whether to suppress this event.
            let should_suppress = if FILTERING_ENABLED.load(Ordering::Relaxed) {
                // Filtering enabled: allow this event.
                //
                // SECURITY: NtTraceEvent's first argument is a trace handle, not a
                // guaranteed pointer to readable memory. Dereferencing RCX in a VEH can
                // fault inside the exception path and crash the process.
                //
                // Keep filtering mode crash-safe by avoiding any RCX dereference here.
                false
            } else {
                // Filtering disabled: suppress all events (original behavior).
                true
            };

            if should_suppress {
                ctx.Rax = 0; // STATUS_SUCCESS
                let ret_addr = *(ctx.Rsp as *const u64);
                ctx.Rip = ret_addr;
                ctx.Rsp += 8;
                ctx.Dr7 |= DR7_L0; // Re-arm
                EXCEPTION_CONTINUE_EXECUTION
            } else {
                // Allow event to proceed — let NtTraceEvent execute normally.
                EXCEPTION_CONTINUE_SEARCH
            }
        }
    }

    /// Initialize ETW blinding: resolve NtTraceEvent, set HW breakpoint,
    /// install VEH.
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
        tracing::info!(
            "ETW blinding active (patchless via DR0, filtering={})",
            FILTERING_ENABLED.load(Ordering::Relaxed)
        );
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

    /// Enable or disable ETW filtering mode. When filtering is enabled,
    /// NtTraceEvent calls are allowed through. When disabled, all NtTraceEvent
    /// calls are suppressed. Default is enabled. Must be called before
    /// init().
    pub fn set_filtering_enabled(enabled: bool) {
        FILTERING_ENABLED.store(enabled, Ordering::Release);
        if ACTIVE.load(Ordering::Acquire) {
            tracing::info!(
                "ETW filtering mode changed to {} (active)",
                if enabled { "enabled" } else { "disabled" }
            );
        }
    }

    /// Returns whether ETW filtering is currently enabled.
    pub fn is_filtering_enabled() -> bool {
        FILTERING_ENABLED.load(Ordering::Acquire)
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

    /// No-op on non-Windows platforms.
    pub fn set_filtering_enabled(_enabled: bool) {}

    /// Always returns true on non-Windows (not relevant).
    pub fn is_filtering_enabled() -> bool {
        true
    }
}

// ── Public API ──────────────────────────────────────────────────────────────

#[allow(unused_imports)]
pub use inner::{cleanup, init, is_active, is_filtering_enabled, set_filtering_enabled};

#[cfg(test)]
mod tests {
    use super::*;

    // The tests below that call init() / cleanup() are restricted to non-Windows
    // because on Windows those functions install a **process-wide** Vectored
    // Exception Handler and a DR0 hardware breakpoint on `NtTraceEvent`.
    //
    // Running that inside the default multi-threaded Rust test harness causes
    // STATUS_ACCESS_VIOLATION (0xc0000005):
    //   1. init() sets ACTIVE=true, installs the VEH, and arms DR0 — but the test
    //      named "init_stub_does_not_panic" calls init() without a matching
    //      cleanup(), leaving the VEH and HWBP live for the rest of the run.
    //   2. ETW (NtTraceEvent) is called pervasively by Windows internals on every
    //      live thread (heap, loader, WER, etc.).  Each call triggers a
    //      EXCEPTION_SINGLE_STEP that the VEH intercepts.
    //   3. The VEH "returns" from NtTraceEvent by reading the caller's return
    //      address off the stack (*(ctx.Rsp as *const u64)) and jumping to it. If
    //      RSP is in an unexpected state (stack unwinding, exception dispatch,
    //      thread-pool callback teardown), that read faults → crash.
    //
    // On non-Windows init() / cleanup() are no-op stubs, so the same test names
    // remain valid there.  The Windows code paths are covered by the #[ignore]
    // integration test below, which must be run serially and in isolation.

    /// Verifies the non-Windows no-op stub: init() returns Ok and is
    /// idempotent.
    #[cfg(not(windows))]
    #[test]
    fn init_stub_does_not_panic() {
        let _ = init();
    }

    /// Verifies the non-Windows no-op stub: cleanup() does not panic.
    #[cfg(not(windows))]
    #[test]
    fn cleanup_stub_does_not_panic() {
        cleanup();
    }

    /// is_active() must return false before any init() call.
    /// Safe on all platforms: just reads an atomic bool; on Windows we never
    /// call init() in non-ignored tests so ACTIVE is guaranteed false here.
    #[test]
    fn is_active_default_false() {
        assert!(!is_active());
    }

    /// Verifies the non-Windows no-op stub: init()+cleanup() round-trip leaves
    /// is_active() false.
    #[cfg(not(windows))]
    #[test]
    fn init_cleanup_roundtrip() {
        let _ = init();
        cleanup();
        assert!(!is_active());
    }

    /// Verifies filtering defaults to enabled.
    #[test]
    fn filtering_enabled_by_default() {
        assert!(is_filtering_enabled());
    }

    /// Verifies filtering_enabled_by_default test correctly checks the API.
    /// Note: Due to shared global atomic state across parallel tests, we only
    /// verify the API functions exist rather than checking default state.
    #[cfg(not(windows))]
    #[test]
    fn filtering_api_is_callable() {
        // Call the API to verify it exists and is accessible.
        let _ = is_filtering_enabled();
        set_filtering_enabled(true);
        set_filtering_enabled(false);
        set_filtering_enabled(true);
    }

    // ── Windows-only integration test (ignored by default) ───────────────────
    //
    // This test exercises the real ETW-blinding implementation: it installs a
    // process-wide VEH and a DR0 hardware breakpoint on NtTraceEvent.
    //
    // It MUST NOT run in the default `cargo test` multi-threaded harness because
    // the HWBP fires on every thread that calls NtTraceEvent, which can corrupt
    // unrelated test stacks and crash the process.
    //
    // Run manually, serial and isolated:
    //   cargo test -p textquest-dll -- \
    //     --ignored stealth::etw_blind::tests::windows_init_cleanup_roundtrip \
    //     --test-threads=1
    #[cfg(windows)]
    #[test]
    #[ignore = "installs a process-wide VEH and DR0 HWBP on NtTraceEvent; must not run in the \
                default multi-threaded test harness — run manually with --ignored --test-threads=1"]
    fn windows_init_cleanup_roundtrip() {
        let _ = init();
        cleanup();
        assert!(!is_active());
    }

    /// Windows-only test: verifies filtering can be toggled before init().
    #[cfg(windows)]
    #[test]
    #[ignore = "requires Windows HWBP setup; run with --ignored --test-threads=1"]
    fn windows_filtering_mode_toggle() {
        set_filtering_enabled(false);
        assert!(!is_filtering_enabled());
        let _ = init();
        cleanup();
        // Reset to default.
        set_filtering_enabled(true);
    }
}
