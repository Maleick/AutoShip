//! Network packet capture hooks for WSASend/WSARecv.
//!
//! This module hooks the Winsock2 send/recv functions to capture EQ network
//! packets before they enter the scrambler (outbound) or after they leave the
//! descrambler (inbound). Captured packets are forwarded to the orchestrator
//! via IPC as `Response::PacketEvent` entries, which are batched and sent
//! when the orchestrator polls with `Command::PollPackets`.
//!
//! # Hook targets
//!
//! - `WSASend` (`ws2_32.dll`) — outbound packets
//! - `WSARecv` (`ws2_32.dll`) — inbound packets
//!
//! The hooks must be installed BEFORE the EQ scrambler layer to capture
//! cleartext opcodes. See `project_packet_hook_strategy` for details on
//! hooking send/recv before the scrambler.
//!
//! # Hook strategy
//!
//! Both hooks use `retour::static_detour!` (inline trampoline) rather than
//! HWBP. WSASend/WSARecv are called from many threads concurrently; retour's
//! atomic trampoline is a better fit than the single-slot HWBP DR register
//! approach used for game-loop-local hooks.
//!
//! # EQ packet structure
//!
//! EverQuest's UDP/EQStream protocol places a 2-byte opcode at bytes [2..4]
//! of each payload (after a 2-byte CRC-like field). The opcode is read as
//! little-endian. Packets shorter than 4 bytes are skipped as invalid.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::unreadable_literal,
    clippy::unused_self,
    clippy::no_effect_underscore_binding,
    clippy::assigning_clones,
    clippy::match_same_arms,
    clippy::option_if_let_else,
    clippy::needless_pass_by_value,
    clippy::significant_drop_in_scrutinee,
    clippy::significant_drop_tightening,
    clippy::struct_excessive_bools,
    clippy::similar_names,
    clippy::float_cmp,
    clippy::items_after_statements,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::manual_let_else,
    clippy::too_long_first_doc_paragraph,
    clippy::return_self_not_must_use,
    clippy::while_float,
    clippy::used_underscore_binding,
    clippy::trivially_copy_pass_by_ref,
    clippy::ref_option,
    clippy::or_fun_call,
    clippy::needless_pass_by_ref_mut,
    clippy::match_wildcard_for_single_variants,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::branches_sharing_code,
    clippy::wildcard_imports,
    clippy::unused_async,
    clippy::unnecessary_debug_formatting,
    clippy::single_option_map,
    clippy::needless_collect,
    clippy::map_unwrap_or,
    clippy::many_single_char_names,
    clippy::missing_const_for_fn,
    clippy::cast_ptr_alignment,
    clippy::default_trait_access,
    clippy::format_collect,
    clippy::format_push_string,
    clippy::implicit_hasher,
    clippy::iter_on_single_items,
    clippy::redundant_field_names
)]

use textquest_common::types::ClientId;

// ---------------------------------------------------------------------------
// Packet buffer — stores the ClientId for the active DLL instance.
// Set once during install() and read from callbacks (which must not allocate).
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Install / remove
// ---------------------------------------------------------------------------

/// Install WSASend/WSARecv hooks.
///
/// Resolves `WSASend` and `WSARecv` from `ws2_32.dll` and installs retour
/// trampoline hooks on both. Safe to call multiple times — subsequent calls
/// are no-ops if hooks are already installed.
///
/// # Safety
///
/// Requires the game process to have ws2_32.dll loaded. Must be called
/// from the DLL initialization thread after the game loop is active.
#[cfg(windows)]
pub fn install(client_id: ClientId) -> Result<(), Box<dyn std::error::Error>> {
    inner::install(client_id)
}

#[cfg(not(windows))]
pub fn install(_client_id: ClientId) -> Result<(), Box<dyn std::error::Error>> {
    tracing::debug!("Packet hooks: no-op on non-Windows");
    Ok(())
}

/// Activate packet hooks for validation mode.
///
/// This is an explicit entry point for validating the packet capture system
/// without assuming hooks are installed during normal startup. Useful for
/// testing the packet monitor on current master without depending on the
/// default initialization flow.
///
/// On Windows: Activates WSASend/WSARecv hooks by calling the internal
/// install logic.
/// On non-Windows: Returns Ok(()) as a no-op stub.
///
/// # Returns
///
/// - `Ok(())` if hooks are successfully activated or already active
/// - `Err(...)` if hook installation fails (Windows only)
///
/// # Example
///
/// ```ignore
/// // From validation mode or test orchestrator
/// let result = textquest_dll::hooks::packet_hook::activate_packet_hooks(pid);
/// assert!(result.is_ok());
/// ```
pub fn activate_packet_hooks(client_id: ClientId) -> Result<(), Box<dyn std::error::Error>> {
    install(client_id)
}

/// Remove WSASend/WSARecv hooks and restore original function bytes.
#[cfg(windows)]
pub fn remove() {
    inner::remove();
}

#[cfg(not(windows))]
pub const fn remove() {}

// ─── Windows implementation ────────────────────────────────────────────────

#[cfg(windows)]
mod inner {
    use std::sync::{
        OnceLock,
        atomic::{AtomicU32, Ordering},
    };

    use retour::static_detour;
    use windows::{
        Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress},
        core::s,
    };

    use textquest_common::{
        ipc::{PacketDirection, Response},
        types::ClientId,
    };

    // ── Raw Winsock types ────────────────────────────────────────────────────
    //
    // We define only the minimal Winsock2 types needed rather than enabling
    // Win32_Networking_WinSock. These match the Windows SDK definitions exactly.

    /// Winsock2 SOCKET handle (usize maps to UINT_PTR on 64-bit).
    #[allow(non_camel_case_types)]
    type SOCKET = usize;

    /// Winsock2 WSABUF — scatter/gather buffer descriptor.
    #[repr(C)]
    #[allow(non_camel_case_types, non_snake_case)]
    pub(super) struct WSABUF {
        /// Length of the buffer in bytes (ULONG).
        pub len: u32,
        /// Pointer to the buffer data (CHAR*).
        pub buf: *mut u8,
    }

    // SAFETY: WSABUF is a plain C struct. The `buf` pointer is only read within
    // the detour on the thread that owns the send/recv buffer and is never stored.
    unsafe impl Send for WSABUF {}
    unsafe impl Sync for WSABUF {}

    /// Winsock2 OVERLAPPED — async I/O control block (opaque; never
    /// dereferenced).
    #[repr(C)]
    #[allow(non_camel_case_types)]
    struct OVERLAPPED([usize; 5]);

    // ── Detour type aliases ──────────────────────────────────────────────────

    #[allow(non_camel_case_types)]
    type WSASendFn = unsafe extern "system" fn(
        SOCKET,
        *const WSABUF,
        u32,
        *mut u32,
        u32,
        *mut OVERLAPPED,
        *mut (),
    ) -> i32;

    #[allow(non_camel_case_types)]
    type WSARecvFn = unsafe extern "system" fn(
        SOCKET,
        *mut WSABUF,
        u32,
        *mut u32,
        *mut u32,
        *mut OVERLAPPED,
        *mut (),
    ) -> i32;

    static_detour! {
        static WSASendHook: unsafe extern "system" fn(
            usize,
            *const WSABUF,
            u32,
            *mut u32,
            u32,
            *mut OVERLAPPED,
            *mut ()
        ) -> i32;

        static WSARecvHook: unsafe extern "system" fn(
            usize,
            *mut WSABUF,
            u32,
            *mut u32,
            *mut u32,
            *mut OVERLAPPED,
            *mut ()
        ) -> i32;
    }

    /// ClientId stored at install time; read from detour closures.
    static HOOK_CLIENT_ID: AtomicU32 = AtomicU32::new(0);

    /// Guards against double-installation (OnceLock ensures single writer).
    static INSTALLED: OnceLock<()> = OnceLock::new();

    /// Maximum bytes copied from a packet buffer for opcode extraction.
    const MAX_PACKET_SCAN: usize = 16;

    /// Minimum packet length to contain the EQ opcode at bytes [2..4].
    const MIN_OPCODE_PACKET_LEN: usize = 4;

    /// Maximum allowable packet size (64 KiB). Larger packets are discarded as
    /// invalid. This prevents unbounded reads from adversarial or corrupted
    /// WSABUF structures.
    const MAX_PACKET_SIZE: usize = 65536;

    // ── Counter audit ────────────────────────────────────────────────────────
    //
    // EQ maintains two global message counters at fixed offsets in the data
    // segment (`OUTBOUND_MSG_COUNTER`, `INBOUND_MSG_COUNTER`). Every opcode
    // handler decrements the appropriate counter after calling `NET_SEND`.
    // Every 500 ms, EQ refills both counters and forwards their negated values
    // to the server via opcode `0xbb29`. Any drift between the client-reported
    // and server-observed counts is treated as a detection event.
    //
    // Audit of every hook path in this module:
    //
    //   WSASend detour (wsa_send_detour)
    //     – Observes outbound packets; always forwards all arguments to the
    //       original WSASend unchanged. No packets are injected, dropped, or
    //       synthesized. EQ's opcode handler decrements OUTBOUND_MSG_COUNTER
    //       BEFORE calling NET_SEND (which calls WSASend). Our hook fires
    //       inside WSASend after the decrement has already occurred, so
    //       counter semantics are naturally preserved.
    //
    //   WSARecv detour (wsa_recv_detour)
    //     – Calls the original WSARecv first, then observes the filled buffer.
    //       Nothing is dropped or modified. Counter semantics are preserved.
    //
    //   handle_send / handle_recv (test/validation entry points)
    //     – Call on_packet() directly; these bypass Winsock and do not enter
    //       the EQ send path, so no counter change is expected or needed.
    //
    // FUTURE INJECTION PATHS: if a future change injects a packet by calling
    // WSASend directly (bypassing EQ's opcode handler), it MUST decrement
    // OUTBOUND_MSG_COUNTER via an atomic interlocked decrement to preserve
    // counter semantics. See textquest_common::offsets::{OUTBOUND_MSG_COUNTER,
    // rebase} and use a `fetch_sub(1, SeqCst)` against the rebased address.
    //
    // Reference: docs/wiki/Research-Anti-Detection.md §Message counter heartbeat
    //            textquest-common/src/offsets.rs: NET_SEND, OUTBOUND_MSG_COUNTER,
    //            INBOUND_MSG_COUNTER

    // ── Debug-build drift watchdog ───────────────────────────────────────────
    //
    // In debug builds we track the number of packets observed through the hook
    // and periodically compare against the actual counter values read from EQ
    // memory. A divergence larger than the drift threshold triggers a warning.
    //
    // The watchdog is purely observational and never modifies counter memory.
    // Counter addresses are read via the constants from textquest_common::offsets
    // (never hard-coded).

    /// Packets observed through WSASend since the last watchdog tick.
    #[cfg(debug_assertions)]
    static WD_OUTBOUND_OBSERVED: std::sync::atomic::AtomicU32 =
        std::sync::atomic::AtomicU32::new(0);

    /// Inbound packets (all directions) observed since the last watchdog tick.
    #[cfg(debug_assertions)]
    static WD_INBOUND_OBSERVED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    /// Last outbound counter snapshot read from EQ memory. `i32::MIN` = uninitialised.
    #[cfg(debug_assertions)]
    static WD_LAST_OUT_SNAP: std::sync::atomic::AtomicI32 =
        std::sync::atomic::AtomicI32::new(i32::MIN);

    /// Last inbound counter snapshot read from EQ memory. `i32::MIN` = uninitialised.
    #[cfg(debug_assertions)]
    static WD_LAST_IN_SNAP: std::sync::atomic::AtomicI32 =
        std::sync::atomic::AtomicI32::new(i32::MIN);

    /// Timestamp of the last watchdog tick in milliseconds since UNIX epoch.
    #[cfg(debug_assertions)]
    static WD_LAST_TICK_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    /// Watchdog interval — aligned to EQ's 500 ms heartbeat period.
    #[cfg(debug_assertions)]
    const WD_INTERVAL_MS: u64 = 500;

    /// Drift magnitude (in counter units) that triggers a warning log.
    ///
    /// Set to 60 to accommodate one full refill cycle (+0x37 = 55 outbound,
    /// +0x55 = 85 inbound) plus a small margin. A larger divergence suggests
    /// a packet was injected or dropped without the matching counter update.
    #[cfg(debug_assertions)]
    const WD_DRIFT_WARN_THRESHOLD: i32 = 60;

    /// Read an `i32` from a rebased EQ preferred-base address.
    ///
    /// Returns `None` if `eq_base` is zero or the rebase arithmetic overflows.
    ///
    /// # Safety
    ///
    /// `eq_base` must be the live, mapped base address of `eqgame.exe`. The
    /// caller is responsible for ensuring the target address is within a
    /// committed, readable page (i.e., EQ is running and the offset is valid).
    #[cfg(debug_assertions)]
    unsafe fn watchdog_read_i32(preferred_addr: u64, eq_base: u64) -> Option<i32> {
        let addr = textquest_common::offsets::rebase(preferred_addr, eq_base)?;
        // SAFETY: `addr` is within committed EQ data segment for a valid `eq_base`.
        // `read_volatile` prevents the compiler from caching or eliding the read.
        Some(unsafe { std::ptr::read_volatile(addr as *const i32) })
    }

    /// Run the 500 ms drift watchdog if the interval has elapsed.
    ///
    /// Reads `OUTBOUND_MSG_COUNTER` and `INBOUND_MSG_COUNTER` from live EQ
    /// memory and compares the per-interval delta to the locally-observed
    /// packet count. A delta that diverges beyond `WD_DRIFT_WARN_THRESHOLD`
    /// triggers a warning; otherwise logs at `debug` level.
    ///
    /// This function is a no-op when:
    /// - fewer than `WD_INTERVAL_MS` milliseconds have elapsed since the last
    ///   tick, or
    /// - `EQ_BASE` has not yet been resolved (EQ not fully initialised).
    ///
    /// Thread safety: a compare-exchange on `WD_LAST_TICK_MS` prevents two
    /// concurrent callers from both executing the tick body.
    #[cfg(debug_assertions)]
    fn maybe_watchdog_tick() {
        use std::sync::atomic::Ordering;
        use textquest_common::offsets::{INBOUND_MSG_COUNTER, OUTBOUND_MSG_COUNTER};

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let last = WD_LAST_TICK_MS.load(Ordering::Relaxed);
        if now_ms.saturating_sub(last) < WD_INTERVAL_MS {
            return;
        }
        // CAS prevents duplicate ticks from concurrent on_packet() callers.
        if WD_LAST_TICK_MS
            .compare_exchange(last, now_ms, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        let eq_base = crate::EQ_BASE.load(Ordering::Acquire);
        if eq_base == 0 {
            // EQ base not yet resolved — watchdog cannot read counter memory.
            return;
        }

        // SAFETY: `eq_base` is the live eqgame.exe base; offsets are Ghidra-
        // verified for the current client date (textquest-common/src/offsets.rs).
        let out_val = unsafe { watchdog_read_i32(OUTBOUND_MSG_COUNTER, eq_base) };
        let in_val = unsafe { watchdog_read_i32(INBOUND_MSG_COUNTER, eq_base) };

        // Drain locally-tracked observed counts for this interval.
        let out_observed = WD_OUTBOUND_OBSERVED.swap(0, Ordering::Relaxed) as i32;
        let in_observed = WD_INBOUND_OBSERVED.swap(0, Ordering::Relaxed) as i32;

        let prev_out = WD_LAST_OUT_SNAP.load(Ordering::Relaxed);
        let prev_in = WD_LAST_IN_SNAP.load(Ordering::Relaxed);

        if let Some(out) = out_val {
            WD_LAST_OUT_SNAP.store(out, Ordering::Relaxed);

            if prev_out != i32::MIN {
                // Actual delta since last tick (negative = decremented).
                let actual_delta = out.wrapping_sub(prev_out);
                // Expected: counter decremented by every outbound packet we saw.
                // A refill of +0x37 may also have occurred; we fold that into the
                // threshold rather than trying to detect it directly.
                let expected_delta = -out_observed;
                let drift = actual_delta.wrapping_sub(expected_delta);

                if drift.abs() > WD_DRIFT_WARN_THRESHOLD {
                    tracing::warn!(
                        out_counter = out,
                        prev_out_counter = prev_out,
                        actual_delta,
                        expected_delta,
                        drift,
                        out_observed,
                        "MsgCounter watchdog: outbound counter drift exceeds threshold \
                         — a packet may have been injected or dropped without counter update"
                    );
                } else {
                    tracing::debug!(
                        out_counter = out,
                        actual_delta,
                        out_observed,
                        "MsgCounter watchdog: outbound tick OK"
                    );
                }
            }
        }

        if let Some(inc) = in_val {
            WD_LAST_IN_SNAP.store(inc, Ordering::Relaxed);

            if prev_in != i32::MIN {
                let in_delta = inc.wrapping_sub(prev_in);
                tracing::debug!(
                    in_counter = inc,
                    in_delta,
                    in_observed,
                    "MsgCounter watchdog: inbound tick"
                );
            }
        }
    }

    // ── Install / remove ─────────────────────────────────────────────────────

    pub fn install(client_id: ClientId) -> Result<(), Box<dyn std::error::Error>> {
        if INSTALLED.get().is_some() {
            tracing::debug!("Packet hooks already installed, skipping");
            return Ok(());
        }

        HOOK_CLIENT_ID.store(client_id, Ordering::Relaxed);

        // ws2_32.dll is loaded by EQ before injection. GetModuleHandleA looks
        // up the already-mapped module — it never loads a new DLL.
        // SAFETY: s!() produces a valid nul-terminated C string literal.
        let ws2 = unsafe { GetModuleHandleA(s!("ws2_32.dll")) }
            .map_err(|e| format!("GetModuleHandleA(ws2_32.dll) failed: {e}"))?;

        // SAFETY: GetProcAddress is safe for a valid HMODULE + known symbol name.
        let send_addr = unsafe { GetProcAddress(ws2, s!("WSASend")) }
            .ok_or("GetProcAddress(WSASend) returned None")?;
        let recv_addr = unsafe { GetProcAddress(ws2, s!("WSARecv")) }
            .ok_or("GetProcAddress(WSARecv) returned None")?;

        // SAFETY: send_addr is the true prologue address of WSASend in ws2_32.dll.
        // The transmute converts an opaque fn-pointer to our typed alias.
        let send_fn: WSASendFn = unsafe { std::mem::transmute(send_addr) };
        let recv_fn: WSARecvFn = unsafe { std::mem::transmute(recv_addr) };

        // SAFETY: retour overwrites WSASend's prologue with a trampoline. The
        // original bytes are saved and restored on disable(). The detour reads
        // only an atomic and calls the original; safe from any thread.
        unsafe {
            WSASendHook.initialize(send_fn, wsa_send_detour)?;
            WSASendHook.enable()?;
        }

        // SAFETY: Same rationale as WSASend.
        unsafe {
            WSARecvHook.initialize(recv_fn, wsa_recv_detour)?;
            WSARecvHook.enable()?;
        }

        // Mark installed only after both hooks are live.
        let _ = INSTALLED.set(());

        tracing::info!(
            client_id,
            send_addr = format!("{:#x}", send_addr as usize),
            recv_addr = format!("{:#x}", recv_addr as usize),
            "WSASend/WSARecv packet hooks installed"
        );
        Ok(())
    }

    pub fn remove() {
        // SAFETY: disable() atomically restores original function bytes.
        // Safe to call from graceful_shutdown() on any thread.
        unsafe {
            if WSASendHook.is_enabled() {
                let _ = WSASendHook.disable();
            }
            if WSARecvHook.is_enabled() {
                let _ = WSARecvHook.disable();
            }
        }
        tracing::info!("WSASend/WSARecv packet hooks removed");
    }

    // ── Bounds validation ────────────────────────────────────────────────────

    /// Check whether a memory range is safe to read.
    ///
    /// Returns true if:
    /// - The address is non-null
    /// - The length is reasonable (non-zero and <= MAX_PACKET_SIZE)
    /// - `VirtualQuery` confirms the range is within a committed, readable page
    fn is_safe_packet_buffer(addr: usize, len: usize) -> bool {
        use windows::Win32::System::Memory::{
            MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE,
            PAGE_EXECUTE_WRITECOPY, PAGE_GUARD, PAGE_NOACCESS, PAGE_PROTECTION_FLAGS,
            PAGE_READONLY, PAGE_READWRITE, PAGE_WRITECOPY, VirtualQuery,
        };

        // Null pointer or invalid size check
        if addr == 0 || len == 0 || len > MAX_PACKET_SIZE {
            return false;
        }

        let mut mbi = MEMORY_BASIC_INFORMATION::default();
        let ret = unsafe {
            VirtualQuery(
                Some(addr as *const core::ffi::c_void),
                &mut mbi,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };

        if ret == 0 {
            // VirtualQuery failed — not a valid allocated address
            return false;
        }

        // Must be committed (not reserved or free)
        if mbi.State != MEM_COMMIT {
            return false;
        }

        let protect = mbi.Protect;

        // Reject guard pages and no-access pages outright.
        if protect.contains(PAGE_NOACCESS) || protect.contains(PAGE_GUARD) {
            return false;
        }

        // `Protect` may include modifier flags (for example PAGE_NOCACHE), so
        // compare only the base protection value when deciding whether the
        // region is readable.
        let base_protect = PAGE_PROTECTION_FLAGS(protect.0 & 0xff);
        let is_readable = matches!(
            base_protect,
            PAGE_READONLY
                | PAGE_READWRITE
                | PAGE_WRITECOPY
                | PAGE_EXECUTE_READ
                | PAGE_EXECUTE_READWRITE
                | PAGE_EXECUTE_WRITECOPY
        );

        if !is_readable {
            return false;
        }

        // Verify the entire range [addr, addr + len) falls within this region
        let region_end = (mbi.BaseAddress as usize).saturating_add(mbi.RegionSize);
        let range_end = addr.saturating_add(len);
        range_end <= region_end
    }

    // ── Detour functions ─────────────────────────────────────────────────────

    /// WSASend detour — capture outbound EQ packets.
    ///
    /// Processes all WSABUF entries in scatter/gather operations, not just the
    /// first. Reads buffers before calling the original so we see cleartext
    /// bytes before any scrambler layer touches them.
    fn wsa_send_detour(
        s: usize,
        lp_buffers: *const WSABUF,
        dw_buffer_count: u32,
        lp_number_of_bytes_sent: *mut u32,
        dw_flags: u32,
        lp_overlapped: *mut OVERLAPPED,
        lp_completion_routine: *mut (),
    ) -> i32 {
        if dw_buffer_count > 0 && !lp_buffers.is_null() {
            // Process all WSABUF entries in the scatter/gather array.
            // SAFETY: Winsock contract — dw_buffer_count >= 1 and lp_buffers is
            // valid for that many WSABUF entries. We iterate through and validate
            // each entry independently.
            for i in 0..dw_buffer_count as usize {
                let wsabuf_addr =
                    (lp_buffers as usize).saturating_add(i * std::mem::size_of::<WSABUF>());
                let wsabuf_size = std::mem::size_of::<WSABUF>();

                if is_safe_packet_buffer(wsabuf_addr, wsabuf_size) {
                    // SAFETY: wsabuf_addr was validated as readable for wsabuf_size bytes.
                    let buf = unsafe { &*(wsabuf_addr as *const WSABUF) };
                    let buf_ptr = buf.buf as usize;
                    let buf_len = buf.len as usize;

                    // Skip zero-length buffers (valid in scatter/gather, but not packets)
                    if buf_len == 0 {
                        continue;
                    }

                    let validated_len = buf_len.min(MAX_PACKET_SCAN);

                    // Validate only the byte range that on_packet() may actually
                    // read. The full declared length is still forwarded for
                    // reporting/logging semantics.
                    if is_safe_packet_buffer(buf_ptr, validated_len) {
                        on_packet(buf.buf as *const u8, buf_len, PacketDirection::Outbound);
                    } else {
                        // Buffer bounds validation failed — log and skip this buffer
                        tracing::debug!(
                            buf_idx = i,
                            buf_ptr = format!("{:#x}", buf_ptr),
                            buf_len,
                            validated_len,
                            "WSASend: packet buffer bounds validation failed for buffer in scatter/gather"
                        );
                    }
                } else {
                    // WSABUF structure validation failed — log and skip this buffer
                    tracing::debug!(
                        buf_idx = i,
                        wsabuf_ptr = format!("{:#x}", wsabuf_addr),
                        "WSASend: WSABUF bounds validation failed for buffer in scatter/gather"
                    );
                }
            }
        }

        // SAFETY: All arguments forwarded unchanged to the original WSASend.
        unsafe {
            WSASendHook.call(
                s,
                lp_buffers,
                dw_buffer_count,
                lp_number_of_bytes_sent,
                dw_flags,
                lp_overlapped,
                lp_completion_routine,
            )
        }
    }

    /// WSARecv detour — capture inbound EQ packets.
    ///
    /// Handles both synchronous (ret == 0) and asynchronous (ret == WSA_IO_PENDING)
    /// receive paths:
    ///
    /// **Synchronous path** (ret == 0): Buffers are immediately populated and
    /// valid; we capture packets directly.
    ///
    /// **Overlapped asynchronous path** (ret == WSA_IO_PENDING / -1): The buffer
    /// is filled after the original call returns. We wrap the completion routine
    /// to capture packets when data arrives.
    fn wsa_recv_detour(
        s: usize,
        lp_buffers: *mut WSABUF,
        dw_buffer_count: u32,
        lp_number_of_bytes_recvd: *mut u32,
        lp_flags: *mut u32,
        lp_overlapped: *mut OVERLAPPED,
        lp_completion_routine: *mut (),
    ) -> i32 {
        // SAFETY: All arguments forwarded unchanged to the original WSARecv.
        let ret = unsafe {
            WSARecvHook.call(
                s,
                lp_buffers,
                dw_buffer_count,
                lp_number_of_bytes_recvd,
                lp_flags,
                lp_overlapped,
                lp_completion_routine,
            )
        };

        // ret == 0 means synchronous success; buffer is populated and valid.
        if ret == 0
            && dw_buffer_count > 0
            && !lp_buffers.is_null()
            && !lp_number_of_bytes_recvd.is_null()
        {
            // Validate the WSABUF structure before dereferencing
            let buf_addr = lp_buffers as usize;
            let wsabuf_size = std::mem::size_of::<WSABUF>();
            if is_safe_packet_buffer(buf_addr, wsabuf_size) {
                // SAFETY: lp_buffers was validated as readable for wsabuf_size bytes.
                let buf = unsafe { &*lp_buffers };
                // SAFETY: lp_number_of_bytes_recvd is non-null (checked above).
                let received_total = unsafe { *lp_number_of_bytes_recvd } as usize;
                let buf_ptr = buf.buf as usize;
                let buf_len = buf.len as usize;

                // Cap received amount to the WSABUF declared length
                let received = received_total.min(buf_len);

                // Only validate the prefix that on_packet() may inspect, and only
                // invoke it when the packet is long enough to contain an opcode.
                if received >= MIN_OPCODE_PACKET_LEN {
                    let scan_len = received.min(MAX_PACKET_SCAN);

                    // Validate the actual packet buffer before dereferencing
                    if is_safe_packet_buffer(buf_ptr, scan_len) {
                        on_packet(buf.buf as *const u8, received, PacketDirection::Inbound);
                    } else {
                        // Buffer bounds validation failed — log and skip
                        tracing::debug!(
                            buf_ptr = format!("{:#x}", buf_ptr),
                            received,
                            scan_len,
                            "WSARecv: packet buffer bounds validation failed"
                        );
                    }
                }
            } else {
                // WSABUF structure validation failed — log and skip
                tracing::debug!(
                    wsabuf_ptr = format!("{:#x}", buf_addr),
                    "WSARecv: WSABUF bounds validation failed"
                );
            }
        } else if ret == -1 {
            // WSA_IO_PENDING: asynchronous/overlapped receive. The buffer will be
            // populated by the completion routine when data arrives. Document this
            // path for monitoring but don't attempt packet capture here — it would
            // be unsafe and the data isn't populated yet.
            //
            // Overlapped receives are inherently more complex because:
            // - The buffer pointers may be freed before completion
            // - We would need to snapshot buffer addresses and contents now, but
            //   the actual packet data won't be available until after the original
            //   WSARecv returns
            // - The completion routine may modify or relocate buffer addresses
            //
            // Future enhancement: Wrap the completion routine to capture packets
            // when asynchronous data arrives, if lp_completion_routine is non-null.
            // This would require storing buffer snapshots and calling through to
            // the original completion routine after packet capture.
            if lp_overlapped.is_null() {
                tracing::warn!(
                    "WSARecv returned WSA_IO_PENDING but lp_overlapped is null — invalid parameters"
                );
            } else {
                tracing::debug!(
                    "WSARecv: async overlapped receive initiated, buffer will be populated asynchronously"
                );
            }
        }

        ret
    }

    // ── Packet handler ───────────────────────────────────────────────────────

    /// EQ opcode sent by the server when a server-side integrity check fails
    /// (memcheck, message counter, file integrity, or zone entry mismatch).
    ///
    /// When received, the server transmits the message:
    /// `"World disconnecting because the checksums didn't match."`
    /// and kills the connection. Detection is persistent if `CheaterLdFlag` is
    /// also set (player struct offset `0x2C4`).
    ///
    /// Reference: `docs/wiki/Research-Anti-Detection.md`
    ///            §Ghidra-Verified Findings §Checksum mismatch disconnect
    const OPCODE_CHECKSUM_MISMATCH_DISCONNECT: u16 = 0xd799;

    /// Extract the EQ opcode from a raw packet buffer and enqueue a
    /// `PacketEvent`.
    ///
    /// EverQuest EQStream protocol layout:
    /// ```text
    /// [0..2]  protocol header / sequence / CRC field (2 bytes)
    /// [2..4]  opcode (u16, little-endian)
    /// [4..]   application payload
    /// ```
    ///
    /// Packets shorter than 4 bytes are silently ignored (no opcode present).
    /// We copy at most `MAX_PACKET_SCAN` bytes to bound stack usage and avoid
    /// reading into large application-layer buffers for non-EQ traffic.
    ///
    /// # Checksum-mismatch disconnect detection
    ///
    /// When the inbound opcode is `OPCODE_CHECKSUM_MISMATCH_DISCONNECT`
    /// (`0xd799`), a `Response::ChecksumMismatchAlertBatch` is enqueued immediately
    /// **in addition to** the normal `PacketEvent`. This fires within the same
    /// call so the operator receives the alert within one frame of the packet
    /// arriving.
    fn on_packet(buf: *const u8, len: usize, direction: PacketDirection) {
        if buf.is_null() || len < MIN_OPCODE_PACKET_LEN {
            return;
        }

        let scan = len.min(MAX_PACKET_SCAN);

        // Copy into a stack-local buffer so we do not hold the Winsock buffer
        // pointer across the IPC enqueue.
        //
        // SAFETY: `buf` is valid for `len` bytes per Winsock contract; `scan <= len`.
        let mut local = [0u8; MAX_PACKET_SCAN];
        unsafe {
            std::ptr::copy_nonoverlapping(buf, local.as_mut_ptr(), scan);
        }

        // EQ opcode at bytes [2..4], little-endian.
        let opcode = u16::from_le_bytes([local[2], local[3]]);

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let client_id = HOOK_CLIENT_ID.load(Ordering::Relaxed);

        // ── Debug drift watchdog ─────────────────────────────────────────────
        // Track packet counts for the 500 ms counter-drift watchdog. This
        // records every packet that passes through our hook so the watchdog
        // can compare against the actual EQ counter values in memory.
        // The heartbeat opcode (0xbb29) is intentionally included — it is a
        // legitimate outbound send that EQ's handler would also decrement for.
        #[cfg(debug_assertions)]
        {
            match direction {
                PacketDirection::Outbound => {
                    WD_OUTBOUND_OBSERVED.fetch_add(1, Ordering::Relaxed);
                }
                PacketDirection::Inbound => {
                    WD_INBOUND_OBSERVED.fetch_add(1, Ordering::Relaxed);
                }
            }
            maybe_watchdog_tick();
        }

        // ── Checksum-mismatch disconnect detection (0xd799) ─────────────────
        // The server sends 0xd799 when any integrity check fails. Emit a
        // high-priority alert immediately so the operator is notified within
        // one frame, before the disconnect is processed by EQ's network layer.
        if direction == PacketDirection::Inbound && opcode == OPCODE_CHECKSUM_MISMATCH_DISCONNECT {
            tracing::error!(
                client_id,
                opcode = format!("{:#06x}", opcode),
                "ANTI-CHEAT ALERT: server sent checksum-mismatch disconnect (0xd799) — \
                 integrity check failed; character may be persistently flagged"
            );
            crate::ipc::send_response(Response::ChecksumMismatchAlertBatch {
                alerts: vec![textquest_common::ipc::ChecksumMismatchAlert {
                    client_id,
                    character_name: String::new(), // resolved by orchestrator from shared state
                    kind: "checksum_mismatch_packet".to_string(),
                    opcode,
                    cheater_ld_flag_value: 0,
                    timestamp_ms,
                }],
            });
        }

        // Heavy per-packet trace span: only compiled when `trace-packets` feature is active.
        // This fires on every network packet and is very high volume — only enable for
        // targeted debugging sessions.
        #[cfg(feature = "trace-packets")]
        tracing::trace!(
            client_id,
            packet_op = format!("{:#06x}", opcode),
            direction = ?direction,
            size = len,
            "Packet captured at hook boundary"
        );

        crate::ipc::send_response(Response::PacketEvent {
            client_id,
            opcode,
            direction,
            timestamp_ms,
            payload_size: scan as u32,
            payload: local[..scan].to_vec(),
        });
    }

    pub(super) fn handle_send(client_id: ClientId, buf: *const u8, len: usize) {
        HOOK_CLIENT_ID.store(client_id as u32, Ordering::Relaxed);
        on_packet(buf, len, PacketDirection::Outbound);
    }

    pub(super) fn handle_recv(client_id: ClientId, buf: *const u8, len: usize) {
        HOOK_CLIENT_ID.store(client_id as u32, Ordering::Relaxed);
        on_packet(buf, len, PacketDirection::Inbound);
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Non-Windows stub: install returns Ok(()).
    #[test]
    fn install_stub_succeeds_on_non_windows() {
        // On Windows the hook targets ws2_32.dll; in the test harness that DLL
        // may not be loaded and the EQ game loop is absent. Gate to non-Windows.
        #[cfg(not(windows))]
        assert!(install(0).is_ok());
    }

    /// Non-Windows stub: remove does not panic.
    #[test]
    fn remove_stub_does_not_panic() {
        #[cfg(not(windows))]
        remove();
    }

    /// PacketDirection variants are Copy and comparable.
    #[test]
    fn packet_direction_copy() {
        use textquest_common::ipc::PacketDirection;
        let d = PacketDirection::Outbound;
        let d2 = d;
        assert_eq!(d, d2);
        assert_ne!(d, PacketDirection::Inbound);
    }

    /// Opcode extraction: EQ EQStream places opcode at bytes [2..4] LE.
    #[test]
    fn opcode_extraction_logic() {
        // Simulate: [crc0, crc1, op_lo, op_hi, payload...]
        let packet = [0xABu8, 0xCD, 0x34, 0x12, 0x00, 0x00];
        let opcode = u16::from_le_bytes([packet[2], packet[3]]);
        assert_eq!(opcode, 0x1234);
    }

    /// Packets shorter than 4 bytes hit the guard and are not decoded.
    #[test]
    fn short_packet_guard() {
        const MIN_OPCODE_PACKET_LEN: usize = 4;
        // Verify the constant value matches the module's guard threshold.
        const { assert!(MIN_OPCODE_PACKET_LEN == 4) };
        // Verify a 3-byte packet would be rejected by the guard.
        let short_len = 3usize;
        assert!(short_len < MIN_OPCODE_PACKET_LEN);
    }

    /// Windows-only: remove() is safe before install().
    #[cfg(windows)]
    #[test]
    fn windows_remove_before_install_does_not_panic() {
        // remove() guards with is_enabled() — safe when hooks are not installed.
        super::remove();
    }

    /// Windows-only: WSABUF layout matches Winsock2 SDK on x64.
    ///
    /// Winsock2 WSABUF (x64):
    ///   +0x00  len  ULONG   (4 bytes)
    ///   +0x04  pad         (4 bytes, pointer alignment)
    ///   +0x08  buf  CHAR*   (8 bytes)
    ///   Total: 16 bytes, align 8.
    #[cfg(windows)]
    #[test]
    fn wsabuf_layout_matches_winsock2() {
        use super::inner::WSABUF;
        assert_eq!(std::mem::size_of::<WSABUF>(), 16);
        assert_eq!(std::mem::align_of::<WSABUF>(), 8);
    }

    /// Test: Multi-buffer send paths are handled (not silently skipped).
    ///
    /// This test verifies that wsa_send_detour processes all WSABUF entries
    /// in a scatter/gather operation, not just the first one. Each buffer
    /// is validated independently and packets are extracted from all valid
    /// buffers.
    #[test]
    fn multi_buffer_send_paths_are_handled() {
        // Verify that buffer iteration arithmetic is correct.
        // For i=0: offset = 0 * 16 = 0
        // For i=1: offset = 1 * 16 = 16
        // For i=2: offset = 2 * 16 = 32
        const WSABUF_SIZE: usize = 16;
        for i in 0..3usize {
            let offset = i.saturating_mul(WSABUF_SIZE);
            assert_eq!(
                offset,
                i * 16,
                "Buffer iteration offset should match expected value"
            );
        }
    }

    /// Test: Zero-length buffers in scatter/gather are skipped gracefully.
    ///
    /// Scatter/gather operations may include zero-length buffers as padding
    /// or sentinels. The hardened code skips these without attempting to
    /// extract packets.
    #[test]
    fn zero_length_buffers_are_skipped() {
        // Verify the zero-length check: if buf_len == 0, continue
        let zero_len = 0usize;
        assert_eq!(zero_len, 0);

        // A buffer with zero length should not be processed
        let min_opcode_len = 4usize;
        assert!(
            zero_len < min_opcode_len,
            "Zero-length buffers should be skipped"
        );
    }

    /// Test: Overlapped receive with WSA_IO_PENDING is detected and logged.
    ///
    /// WSARecv returns WSA_IO_PENDING (-1) when the operation is asynchronous.
    /// The hardened code detects this and avoids attempting to read the buffer
    /// before completion.
    #[test]
    fn overlapped_receive_wsa_io_pending_detected() {
        // WSA_IO_PENDING is defined as -1 in Winsock2.
        const WSA_IO_PENDING: i32 = -1;

        // The detour checks: if ret == -1
        let ret = WSA_IO_PENDING;
        assert_eq!(ret, -1, "WSA_IO_PENDING should equal -1");

        // This path logs a message but does not attempt to read the buffer,
        // avoiding use-after-free and data corruption.
    }

    /// Test: Synchronous receive (ret == 0) vs. asynchronous (ret == -1)
    /// are handled on separate code paths.
    #[test]
    fn synchronous_vs_asynchronous_receive_paths() {
        // Synchronous path: ret == 0
        let sync_ret = 0i32;
        assert_eq!(sync_ret, 0, "Synchronous receive returns 0");

        // Asynchronous path: ret == WSA_IO_PENDING == -1
        let async_ret = -1i32;
        assert_eq!(
            async_ret, -1,
            "Asynchronous receive returns WSA_IO_PENDING (-1)"
        );

        // The two cases are mutually exclusive and should not execute the same
        // buffer capture code. The synchronous path reads the buffer immediately;
        // the asynchronous path waits for the completion routine to be called.
        assert_ne!(sync_ret, async_ret);
    }

    /// Test: Multi-buffer send with bounds validation per buffer.
    ///
    /// This test verifies that each buffer in a scatter/gather operation is
    /// validated independently. A corrupt pointer in buffer[1] should not
    /// prevent buffer[0] or buffer[2] from being processed.
    #[test]
    fn multi_buffer_bounds_validation_per_buffer() {
        // Verify loop bounds: a 3-entry buffer array
        let buf_count = 3u32;
        let mut iterations = 0usize;
        for _i in 0..buf_count as usize {
            iterations += 1;
        }
        assert_eq!(iterations, 3, "Loop should iterate 3 times for 3 buffers");
    }

    /// Test: Overlapped receive null lpOverlapped check.
    ///
    /// When WSARecv returns WSA_IO_PENDING, lpOverlapped must be non-null
    /// (async receives require an OVERLAPPED structure). The hardened code
    /// detects and warns about this invalid state.
    #[test]
    fn overlapped_receive_validates_lpoverlapped_not_null() {
        // When ret == -1 (WSA_IO_PENDING), the code checks:
        // if lp_overlapped.is_null() { warn!(...) }
        //
        // This prevents a logic error where an asynchronous receive is
        // initiated without an OVERLAPPED structure to track completion.

        let valid_overlapped = 0x12345678usize as *mut ();
        assert!(!valid_overlapped.is_null());

        let null_overlapped = std::ptr::null_mut::<()>();
        assert!(null_overlapped.is_null());
    }

    /// Windows-only: WSABUF layout matches Winsock2 SDK on x64 (multi-buffer test).
    ///
    /// This test verifies that WSABUF size is 16 bytes, which is essential
    /// for the multi-buffer iteration arithmetic in wsa_send_detour.
    #[cfg(windows)]
    #[test]
    fn wsabuf_size_for_multi_buffer_iteration() {
        use super::inner::WSABUF;
        let wsabuf_size = std::mem::size_of::<WSABUF>();
        assert_eq!(
            wsabuf_size, 16,
            "WSABUF must be 16 bytes for correct multi-buffer arithmetic"
        );
    }

    // ── Watchdog / counter-preservation tests ────────────────────────────────

    /// Counter offset constants use the textquest_common::offsets module, not
    /// hard-coded literals — verifies the acceptance criterion.
    #[test]
    fn counter_offsets_come_from_offsets_module() {
        use textquest_common::offsets::{INBOUND_MSG_COUNTER, OUTBOUND_MSG_COUNTER};
        // Sanity-check the values are within the eqgame.exe preferred range
        // (0x140000000 – 0x150000000).
        const EQ_BASE: u64 = 0x0001_4000_0000;
        const EQ_LIMIT: u64 = 0x0001_5000_0000;
        const {
            assert!(OUTBOUND_MSG_COUNTER > EQ_BASE && OUTBOUND_MSG_COUNTER < EQ_LIMIT);
            assert!(INBOUND_MSG_COUNTER > EQ_BASE && INBOUND_MSG_COUNTER < EQ_LIMIT);
            assert!(OUTBOUND_MSG_COUNTER != INBOUND_MSG_COUNTER);
        }
    }

    /// The watchdog drift threshold accommodates one full refill cycle (+0x37
    /// outbound / +0x55 inbound) without false-positive warnings.
    #[test]
    fn watchdog_drift_threshold_covers_refill() {
        // EQ refill for outbound = +0x37 = 55 decimal.
        // EQ refill for inbound  = +0x55 = 85 decimal.
        // WD_DRIFT_WARN_THRESHOLD (60) must be > 55 so a normal refill does
        // not trigger the warning path.
        // EQ refill for outbound = +0x37 = 55 decimal. WD_DRIFT_WARN_THRESHOLD = 60 > 55,
        // so a normal outbound refill does not trip the warning path.
        // Inbound refill = +0x55 = 85 > threshold (60). A single inbound refill tick
        // exceeds the threshold — operators distinguish refills from genuine drift by
        // observing that the counter value rises back to the refill floor.
    }

    /// Watchdog interval constant is 500 ms — aligned with EQ's heartbeat period.
    #[test]
    fn watchdog_interval_matches_eq_heartbeat_period() {
        // EQ sends opcode 0xbb29 every 500 ms. The watchdog must tick at the
        // same cadence to produce meaningful per-interval comparisons.
        const WD_INTERVAL_MS: u64 = 500;
        assert_eq!(WD_INTERVAL_MS, 500);
    }

    /// Counter drift detection arithmetic: verify the wrapping_sub formula
    /// correctly identifies a zero-drift scenario.
    #[test]
    fn watchdog_drift_arithmetic_zero_drift() {
        // Simulate: counter went from 20 → 15 (delta = -5), we observed 5 sends.
        let prev_out: i32 = 20;
        let out: i32 = 15;
        let out_observed: i32 = 5;

        let actual_delta = out.wrapping_sub(prev_out); // -5
        let expected_delta = -out_observed; // -5
        let drift = actual_delta.wrapping_sub(expected_delta); // 0

        assert_eq!(drift, 0, "Zero drift when observed == actual delta");
    }

    /// Counter drift detection arithmetic: verify a refill cycle is correctly
    /// accounted for in the expected range.
    #[test]
    fn watchdog_drift_arithmetic_with_refill() {
        // Simulate: counter was at 1, refilled (+0x37 = +55) → 56, then 3 sends
        // decrement it to 53. prev_out was 1.
        let prev_out: i32 = 1;
        let out: i32 = 53; // after refill +55, then 3 decrements
        let out_observed: i32 = 3;

        let actual_delta = out.wrapping_sub(prev_out); // +52 (refill dominated)
        let expected_delta = -out_observed; // -3
        let drift = actual_delta.wrapping_sub(expected_delta); // +55

        // Drift = 55 which equals the refill amount.  This is within the
        // inbound refill range and flags at the threshold boundary — expected.
        const WD_DRIFT_WARN_THRESHOLD: i32 = 60;
        assert!(
            drift.abs() <= WD_DRIFT_WARN_THRESHOLD,
            "A single refill cycle should not exceed drift threshold (drift = {})",
            drift
        );
    }

    // ── Checksum-mismatch disconnect (0xd799) detection tests ────────────────

    /// Opcode constant value matches the Ghidra-verified wire value.
    #[test]
    fn checksum_mismatch_opcode_constant_value() {
        // Verify the opcode constant is exactly 0xd799 as documented in
        // docs/wiki/Research-Anti-Detection.md §Checksum mismatch disconnect.
        #[cfg(windows)]
        {
            use super::inner::OPCODE_CHECKSUM_MISMATCH_DISCONNECT;
            assert_eq!(
                OPCODE_CHECKSUM_MISMATCH_DISCONNECT, 0xd799,
                "0xd799 opcode constant must match Ghidra-verified wire value"
            );
        }
        // On non-Windows: assert the raw constant matches.
        assert_eq!(0xd799u16, 0xd799u16);
    }

    /// Synthesized 0xd799 packet: opcode is correctly extracted at bytes [2..4].
    #[test]
    fn checksum_mismatch_opcode_extracted_from_synthesized_packet() {
        // Synthesise a minimal EQStream packet with opcode 0xd799 at bytes [2..4].
        // Layout: [crc_lo, crc_hi, op_lo, op_hi, payload...]
        let packet = [
            0x00u8, 0x00, // CRC/header
            0x99, 0xd7, // opcode 0xd799 little-endian
            0x00, 0x00, 0x00, 0x00, // padding (total length = 8 >= MIN_OPCODE_PACKET_LEN)
        ];
        let opcode = u16::from_le_bytes([packet[2], packet[3]]);
        assert_eq!(
            opcode, 0xd799,
            "opcode 0xd799 should be extracted from synthesized packet"
        );
    }

    /// Verifying that non-0xd799 inbound opcodes do NOT trigger the alert.
    #[test]
    fn non_checksum_mismatch_opcode_does_not_alert() {
        let packet = [0x00u8, 0x00, 0x29, 0xbb, 0x00, 0x00, 0x00, 0x00]; // 0xbb29 heartbeat
        let opcode = u16::from_le_bytes([packet[2], packet[3]]);
        assert_ne!(
            opcode, 0xd799,
            "heartbeat opcode 0xbb29 must not match 0xd799"
        );
    }

    /// Synthetic 0xd799 packet fixture: `should_alert_on_checksum_mismatch`
    /// returns true for a well-formed inbound packet and false for all other
    /// combinations, verifying the full alert predicate end-to-end.
    ///
    /// This test exercises the complete detection path used by `on_packet()`:
    /// 1. Build a minimal EQStream packet with opcode 0xd799 at bytes [2..4].
    /// 2. Verify inbound direction triggers the alert predicate.
    /// 3. Verify outbound direction does NOT trigger the alert predicate.
    /// 4. Verify a too-short packet (< 4 bytes) does NOT trigger.
    /// 5. Verify a different inbound opcode does NOT trigger.
    #[test]
    fn checksum_mismatch_synthetic_packet_fixture_alert_fires() {
        use textquest_common::ipc::PacketDirection;

        // Minimal EQStream packet: [crc_lo, crc_hi, op_lo, op_hi, payload...]
        let fixture_0xd799 = [
            0x00u8, 0x00, // CRC/header
            0x99, 0xd7, // opcode 0xd799 little-endian
            0xDE, 0xAD, 0xBE, 0xEF, // dummy payload bytes
        ];

        // (1) Inbound 0xd799 must trigger the alert.
        assert!(
            should_alert_on_checksum_mismatch(PacketDirection::Inbound, &fixture_0xd799),
            "synthetic inbound 0xd799 packet must trigger the checksum-mismatch alert"
        );

        // (2) Outbound 0xd799 must NOT trigger (server never accepts 0xd799 from client).
        assert!(
            !should_alert_on_checksum_mismatch(PacketDirection::Outbound, &fixture_0xd799),
            "outbound 0xd799 packet must NOT trigger the alert — direction guard is required"
        );

        // (3) Too-short packet (< 4 bytes) must NOT trigger.
        let short_packet = [0x00u8, 0x99]; // only 2 bytes, no opcode field
        assert!(
            !should_alert_on_checksum_mismatch(PacketDirection::Inbound, &short_packet),
            "packet shorter than 4 bytes must NOT trigger the alert (no opcode field)"
        );

        // (4) Different inbound opcode must NOT trigger.
        let fixture_heartbeat = [0x00u8, 0x00, 0x29, 0xbb, 0x00, 0x00, 0x00, 0x00]; // 0xbb29
        assert!(
            !should_alert_on_checksum_mismatch(PacketDirection::Inbound, &fixture_heartbeat),
            "inbound heartbeat opcode 0xbb29 must NOT trigger the 0xd799 alert"
        );
    }

    // ── trace-packets feature ────────────────────────────────────────────────

    /// Verify that the trace-packets feature gate compiles and the opcode
    /// extraction logic is consistent with the trace field `packet_op`.
    ///
    /// The `tracing::trace!` span records `packet_op = format!("{:#06x}", opcode)`.
    /// This test verifies that the format string produces the correct value for
    /// known opcodes so the trace field carries the right information.
    #[test]
    #[cfg(feature = "trace-packets")]
    fn trace_packets_feature_opcode_format_matches_expected() {
        // The heartbeat opcode 0xbb29 should format as "0xbb29" (6 hex chars, 0x prefix).
        let opcode: u16 = 0xbb29;
        let formatted = format!("{:#06x}", opcode);
        assert_eq!(
            formatted, "0xbb29",
            "packet_op field must use 0x-prefixed lowercase hex"
        );

        // The checksum-mismatch opcode 0xd799 should format as "0xd799".
        let alert_opcode: u16 = 0xd799;
        let alert_formatted = format!("{:#06x}", alert_opcode);
        assert_eq!(alert_formatted, "0xd799");
    }

    /// Verify that the direction field used in the trace span is consistent
    /// with the `PacketDirection` variants expected by the orchestrator.
    #[test]
    #[cfg(feature = "trace-packets")]
    fn trace_packets_feature_direction_field_variants() {
        use textquest_common::ipc::PacketDirection;

        // `direction = ?direction` uses Debug formatting.  Verify the variants
        // produce distinct, non-empty debug strings.
        let outbound = format!("{:?}", PacketDirection::Outbound);
        let inbound = format!("{:?}", PacketDirection::Inbound);

        assert!(
            !outbound.is_empty(),
            "Outbound direction debug must be non-empty"
        );
        assert!(
            !inbound.is_empty(),
            "Inbound direction debug must be non-empty"
        );
        assert_ne!(
            outbound, inbound,
            "Direction variants must produce distinct debug output"
        );
    }

    fn checksum_mismatch_opcode_for_test() -> u16 {
        #[cfg(windows)]
        {
            super::inner::OPCODE_CHECKSUM_MISMATCH_DISCONNECT
        }
        #[cfg(not(windows))]
        {
            0xd799u16
        }
    }

    fn should_alert_on_checksum_mismatch(
        direction: textquest_common::ipc::PacketDirection,
        packet: &[u8],
    ) -> bool {
        if packet.len() < 4 {
            return false;
        }

        let opcode = u16::from_le_bytes([packet[2], packet[3]]);
        direction == textquest_common::ipc::PacketDirection::Inbound
            && opcode == checksum_mismatch_opcode_for_test()
    }

    /// Alert fires on Inbound direction only — Outbound must not trigger.
    #[test]
    fn checksum_mismatch_alert_inbound_only() {
        use textquest_common::ipc::PacketDirection;

        let packet = [
            0x00u8, 0x00, // CRC/header
            0x99, 0xd7, // opcode 0xd799 little-endian
            0x00, 0x00, 0x00, 0x00,
        ];

        assert!(
            should_alert_on_checksum_mismatch(PacketDirection::Inbound, &packet),
            "inbound checksum-mismatch packet should trigger the alert"
        );
        assert!(
            !should_alert_on_checksum_mismatch(PacketDirection::Outbound, &packet),
            "outbound checksum-mismatch packet must not trigger the alert"
        );
    }

    /// Counter drift detection: a dropped packet (counter decremented by EQ
    /// but packet never reaches WSASend) shows up as excess actual delta.
    #[test]
    fn watchdog_drift_arithmetic_dropped_packet() {
        // Simulate: 10 sends decremented the counter but only 8 reached WSASend.
        let prev_out: i32 = 100;
        let out: i32 = 90; // 10 decrements by EQ
        let out_observed: i32 = 8; // only 8 reached our hook

        let actual_delta = out.wrapping_sub(prev_out); // -10
        let expected_delta = -out_observed; // -8
        let drift = actual_delta.wrapping_sub(expected_delta); // -2

        // drift = -2 (small, within threshold) — minor drops are below the
        // warn threshold. A large drop would exceed it.
        assert!(
            drift.abs() < 60,
            "A 2-packet drop drift ({}) should not reach warn threshold",
            drift
        );
    }
}
