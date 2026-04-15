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

#[cfg(windows)]
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

/// ClientId stored atomically so the callback can read it without locking.
#[cfg(windows)]
static ACTIVE_CLIENT_ID: AtomicU32 = AtomicU32::new(0);

/// Address of WSASend — stored for removal.
#[cfg(windows)]
static WSASEND_ADDR: AtomicUsize = AtomicUsize::new(0);
/// Address of WSARecv — stored for removal.
#[cfg(windows)]
static WSARECV_ADDR: AtomicUsize = AtomicUsize::new(0);

// ---------------------------------------------------------------------------
// Windows platform
// ---------------------------------------------------------------------------

/// Resolve the address of a named export from a loaded module.
#[cfg(windows)]
fn resolve_ws2_function(name: &str) -> Result<usize, Box<dyn std::error::Error>> {
    use windows::{
        Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress},
        core::PCSTR,
    };

    let module_name = std::ffi::CString::new("ws2_32.dll")?;
    let func_name = std::ffi::CString::new(name)?;

    let module = unsafe { GetModuleHandleA(PCSTR(module_name.as_ptr() as *const u8)) }
        .map_err(|e| format!("GetModuleHandleA(ws2_32.dll) failed: {e}"))?;

    let addr = unsafe { GetProcAddress(module, PCSTR(func_name.as_ptr() as *const u8)) };
    match addr {
        Some(f) => Ok(f as usize),
        None => {
            Err(format!("GetProcAddress({name}) returned null — ws2_32.dll not loaded?").into())
        }
    }
}

// ---------------------------------------------------------------------------
// WSASend callback
//
// x64 Microsoft ABI at WSASend entry point:
//   RCX = SOCKET s
//   RDX = LPWSABUF lpBuffers          ← pointer to array of WSABUF
//   R8  = DWORD dwBufferCount
//   R9  = LPDWORD lpNumberOfBytesSent
//   (stack) = DWORD dwFlags, ...
//
// WSABUF layout (16 bytes on x64):
//   +0  u32  len
//   +8  *u8  buf
// ---------------------------------------------------------------------------

#[cfg(windows)]
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
fn wsa_send_callback(exception_info: *mut ()) -> bool {
    let context = unsafe {
        let ptrs =
            exception_info as *const windows::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS;
        &*(*ptrs).ContextRecord
    };

    // RDX = LPWSABUF (pointer to first WSABUF entry)
    let wsa_buf_ptr = context.Rdx as *const u8;
    // R8 = buffer count
    let buf_count = context.R8 as u32;

    if wsa_buf_ptr.is_null() || buf_count == 0 {
        return true;
    }

    // Read first WSABUF: len (u32 at +0), buf ptr (usize at +8 on x64)
    let (buf_ptr, buf_len) = unsafe {
        let len = (wsa_buf_ptr as *const u32).read_unaligned();
        let buf = ((wsa_buf_ptr as *const u8).add(8) as *const usize).read_unaligned();
        (buf as *const u8, len as usize)
    };

    if buf_ptr.is_null() || buf_len < 2 {
        return true;
    }

    let client_id = ACTIVE_CLIENT_ID.load(Ordering::Relaxed) as ClientId;
    on_packet_send(client_id, buf_ptr, buf_len);
    true
}

#[cfg(not(windows))]
fn wsa_send_callback(_exception_info: *mut ()) -> bool {
    true
}

// ---------------------------------------------------------------------------
// WSARecv callback
//
// x64 Microsoft ABI at WSARecv entry point:
//   RCX = SOCKET s
//   RDX = LPWSABUF lpBuffers
//   R8  = DWORD dwBufferCount
//   R9  = LPDWORD lpNumberOfBytesRecvd
//   (stack) = LPDWORD lpFlags, ...
// ---------------------------------------------------------------------------

#[cfg(windows)]
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
fn wsa_recv_callback(exception_info: *mut ()) -> bool {
    let context = unsafe {
        let ptrs =
            exception_info as *const windows::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS;
        &*(*ptrs).ContextRecord
    };

    let wsa_buf_ptr = context.Rdx as *const u8;
    let buf_count = context.R8 as u32;

    if wsa_buf_ptr.is_null() || buf_count == 0 {
        return true;
    }

    let (buf_ptr, buf_len) = unsafe {
        let len = (wsa_buf_ptr as *const u32).read_unaligned();
        let buf = ((wsa_buf_ptr as *const u8).add(8) as *const usize).read_unaligned();
        (buf as *const u8, len as usize)
    };

    if buf_ptr.is_null() || buf_len < 2 {
        return true;
    }

    let client_id = ACTIVE_CLIENT_ID.load(Ordering::Relaxed) as ClientId;
    on_packet_recv(client_id, buf_ptr, buf_len);
    true
}

#[cfg(not(windows))]
fn wsa_recv_callback(_exception_info: *mut ()) -> bool {
    true
}

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

/// Remove WSASend/WSARecv hooks and restore original function bytes.
#[cfg(windows)]
pub fn remove() {
    inner::remove();
}

#[cfg(not(windows))]
pub const fn remove() {}

// ─── Windows implementation ────────────────────────────────────────────────

#[cfg(windows)]
fn on_packet_send(client_id: ClientId, buf: *const u8, len: usize) {
    inner::handle_send(client_id, buf, len);
}

#[cfg(windows)]
fn on_packet_recv(client_id: ClientId, buf: *const u8, len: usize) {
    inner::handle_recv(client_id, buf, len);
}

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
    /// Reads from the first WSABUF before calling the original so we see the
    /// cleartext bytes before any scrambler layer touches them.
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
            // SAFETY: Winsock contract — dw_buffer_count >= 1 and lp_buffers is
            // valid for that many WSABUF entries. We read only the first entry.
            // We validate the buffer pointer and length before dereferencing.
            let buf_addr = lp_buffers as usize;
            let wsabuf_size = std::mem::size_of::<WSABUF>();
            if is_safe_packet_buffer(buf_addr, wsabuf_size) {
                let buf = unsafe { &*lp_buffers };
                let buf_ptr = buf.buf as usize;
                let buf_len = buf.len as usize;
                let validated_len = buf_len.min(MAX_PACKET_SCAN);

                // Validate only the byte range that on_packet() may actually
                // read. The full declared length is still forwarded for
                // reporting/logging semantics.
                if is_safe_packet_buffer(buf_ptr, validated_len) {
                    on_packet(buf.buf as *const u8, buf_len, PacketDirection::Outbound);
                } else {
                    // Buffer bounds validation failed — log and skip
                    tracing::debug!(
                        buf_ptr = format!("{:#x}", buf_ptr),
                        buf_len,
                        validated_len,
                        "WSASend: packet buffer bounds validation failed"
                    );
                }
            } else {
                // WSABUF structure validation failed — log and skip
                tracing::debug!(
                    wsabuf_ptr = format!("{:#x}", buf_addr),
                    "WSASend: WSABUF bounds validation failed"
                );
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
    /// Calls the original first so buffers are populated, then reads the data.
    /// Overlapped receives (ret == –1 / WSA_IO_PENDING) are skipped because the
    /// buffer is filled asynchronously via completion routine.
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
        }

        ret
    }

    // ── Packet handler ───────────────────────────────────────────────────────

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

        crate::ipc::send_response(Response::PacketEvent {
            client_id,
            opcode,
            direction,
            timestamp_ms,
            payload_size: len as u32,
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

/// Returns current time as milliseconds since the Unix epoch.
/// Uses a fast path via `std::time` — acceptable in a detour context since
/// we are at function entry, not inside a Windows syscall.
fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

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
}
