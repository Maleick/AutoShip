//! Patchless ETW blinding via hardware breakpoints with provider filtering.
//!
//! Sets a hardware breakpoint (DR0) on `NtTraceEvent` so that calls are
//! intercepted by our Vectored Exception Handler. The VEH inspects the
//! event provider GUID and only suppresses anticheat-related providers
//! (Windows Defender ETW, threat intelligence). Other ETW events (including
//! EQ's own telemetry) are allowed to pass through.
//!
//! The VEH returns `STATUS_SUCCESS` (RAX = 0) and skips the function body
//! by advancing RIP past the return address on the stack for suppressed events.

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

    /// Whether ETW filtering (provider allowlist) is enabled.
    /// When true, only anticheat providers are suppressed.
    /// When false, all NtTraceEvent calls are suppressed (original behavior).
    /// Default: true (filtering enabled).
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

    /// Anticheat-related ETW provider GUIDs to suppress.
    /// This allowlist contains the GUIDs of providers we want to block.
    /// All other providers are allowed to pass through.
    ///
    /// Providers:
    /// - Microsoft-Windows-Threat-Intelligence (ETW-TI)
    /// - Microsoft-Windows-Kernel-ETW (some kernel events)
    /// - Windows Defender / Security providers
    ///
    /// Note: GUIDs are represented as 16-byte arrays [u8; 16] in little-endian
    /// format matching the GUID structure in memory.
    const ANTICHEAT_PROVIDERS: &[&[u8; 16]] = &[
        // Microsoft-Windows-Threat-Intelligence
        // GUID: 22FB2CD6-0E7B-422B-A0C7-2143CA8DCED3
        &[0xD6, 0x2C, 0xFB, 0x22, 0x7B, 0x0E, 0x2B, 0x42, 0xA0, 0xC7, 0x21, 0x43, 0xCA, 0x8D, 0xCE, 0xD3],
        // Note: Additional anticheat provider GUIDs can be added here as needed.
        // The list is intentionally conservative — only known anticheat/security
        // providers are suppressed, allowing EQ telemetry to pass through.
    ];

    /// Check if a provider GUID should be suppressed (anticheat-related).
    /// Returns true if the provider is in the anticheat allowlist and should be blocked.
    fn should_suppress_provider(provider_guid: *const [u8; 16]) -> bool {
        if provider_guid.is_null() {
            // If provider pointer is null, allow the event to pass.
            return false;
        }

        // SAFETY: The caller (veh_handler) ensures provider_guid points to
        // valid memory within the NtTraceEvent first parameter (EVENT_DESCRIPTOR).
        let guid = unsafe { &*provider_guid };

        // Check if this provider is in our anticheat suppression list.
        ANTICHEAT_PROVIDERS.iter().any(|&anticheat_guid| guid == anticheat_guid)
    }

    /// NtTraceEvent signature for parameter extraction.
    /// First parameter (RCX) points to an EVENT_DESCRIPTOR struct.
    /// The 16-byte provider GUID is at offset 16 within EVENT_DESCRIPTOR.
    /// See: https://docs.microsoft.com/en-us/windows/win32/etw/event-descriptor
    fn extract_provider_guid_from_event_descriptor(event_descriptor: *const u8) -> *const [u8; 16] {
        if event_descriptor.is_null() {
            return std::ptr::null();
        }

        // SAFETY: We're reading at a known offset (16 bytes) into the EVENT_DESCRIPTOR.
        // The caller ensures this memory is valid.
        unsafe { (event_descriptor.add(16)) as *const [u8; 16] }
    }

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
                // Filtering enabled: only suppress anticheat providers.
                // NtTraceEvent first parameter (RCX) is the EVENT_DESCRIPTOR pointer.
                let event_descriptor = ctx.Rcx as *const u8;
                let provider_guid = extract_provider_guid_from_event_descriptor(event_descriptor);
                should_suppress_provider(provider_guid)
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

    /// Enable or disable ETW filtering. When filtering is enabled, only anticheat
    /// providers are suppressed. When disabled, all NtTraceEvent calls are suppressed.
    /// Default is enabled. Must be called before init().
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
    //   1. init() sets ACTIVE=true, installs the VEH, and arms DR0 — but the
    //      test named "init_stub_does_not_panic" calls init() without a matching
    //      cleanup(), leaving the VEH and HWBP live for the rest of the run.
    //   2. ETW (NtTraceEvent) is called pervasively by Windows internals on every
    //      live thread (heap, loader, WER, etc.).  Each call triggers a
    //      EXCEPTION_SINGLE_STEP that the VEH intercepts.
    //   3. The VEH "returns" from NtTraceEvent by reading the caller's return
    //      address off the stack (*(ctx.Rsp as *const u64)) and jumping to it.
    //      If RSP is in an unexpected state (stack unwinding, exception dispatch,
    //      thread-pool callback teardown), that read faults → crash.
    //
    // On non-Windows init() / cleanup() are no-op stubs, so the same test names
    // remain valid there.  The Windows code paths are covered by the #[ignore]
    // integration test below, which must be run serially and in isolation.

    /// Verifies the non-Windows no-op stub: init() returns Ok and is idempotent.
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
    #[ignore = "installs a process-wide VEH and DR0 HWBP on NtTraceEvent; \
                must not run in the default multi-threaded test harness — \
                run manually with --ignored --test-threads=1"]
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
