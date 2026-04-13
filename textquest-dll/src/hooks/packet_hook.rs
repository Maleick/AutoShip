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
//! - `WSASend` (ws2_32.dll) — outbound packets, DR2
//! - `WSARecv` (ws2_32.dll) — inbound packets, DR3
//!
//! The hooks must be installed BEFORE the EQ scrambler layer to capture
//! cleartext opcodes. See `project_packet_hook_strategy` for details on
//! hooking send/recv before the scrambler.
//!
//! # HWBP slot allocation
//!
//! | Slot | Hook         |
//! |------|--------------|
//! | DR0  | game_loop    |
//! | DR1  | chat/eqmain  |
//! | DR2  | WSASend      |
//! | DR3  | WSARecv      |

#[cfg(windows)]
use super::hwbp;
use super::hwbp::HwbpSlot;
use textquest_common::ipc::{PacketDirection, Response};
use textquest_common::types::ClientId;

const WSASEND_SLOT: HwbpSlot = HwbpSlot::Dr2;
const WSARECV_SLOT: HwbpSlot = HwbpSlot::Dr3;

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
    use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
    use windows::core::PCSTR;

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
/// # Safety
///
/// Requires the game process to have ws2_32.dll loaded. Must be called
/// from the DLL initialization thread after the game loop is active.
#[cfg(windows)]
pub fn install(client_id: ClientId) -> Result<(), Box<dyn std::error::Error>> {
    ACTIVE_CLIENT_ID.store(client_id as u32, Ordering::Relaxed);

    let wsa_send_addr = resolve_ws2_function("WSASend")?;
    let wsa_recv_addr = resolve_ws2_function("WSARecv")?;

    WSASEND_ADDR.store(wsa_send_addr, Ordering::Release);
    WSARECV_ADDR.store(wsa_recv_addr, Ordering::Release);

    hwbp::register(WSASEND_SLOT, wsa_send_addr, wsa_send_callback)
        .map_err(|e| format!("WSASend HWBP registration failed: {e}"))?;

    hwbp::register(WSARECV_SLOT, wsa_recv_addr, wsa_recv_callback).map_err(|e| {
        // Roll back WSASend hook on failure
        let _ = hwbp::unregister(WSASEND_SLOT);
        format!("WSARecv HWBP registration failed: {e}")
    })?;

    tracing::info!(
        wsa_send = format!("{:#x}", wsa_send_addr),
        wsa_recv = format!("{:#x}", wsa_recv_addr),
        client_id,
        "Packet hooks installed (DR2=WSASend, DR3=WSARecv)"
    );
    Ok(())
}

#[cfg(not(windows))]
pub fn install(_client_id: ClientId) -> Result<(), Box<dyn std::error::Error>> {
    tracing::debug!("Packet hooks: no-op on non-Windows");
    Ok(())
}

/// Remove WSASend/WSARecv hooks.
#[cfg(windows)]
pub fn remove() {
    if hwbp::is_active(WSASEND_SLOT) {
        if let Err(e) = hwbp::unregister(WSASEND_SLOT) {
            tracing::warn!(error = %e, "Failed to remove WSASend hook (DR2)");
        } else {
            tracing::info!("WSASend hook removed (DR2)");
        }
    }
    if hwbp::is_active(WSARECV_SLOT) {
        if let Err(e) = hwbp::unregister(WSARECV_SLOT) {
            tracing::warn!(error = %e, "Failed to remove WSARecv hook (DR3)");
        } else {
            tracing::info!("WSARecv hook removed (DR3)");
        }
    }
    WSASEND_ADDR.store(0, Ordering::Release);
    WSARECV_ADDR.store(0, Ordering::Release);
    tracing::info!("Packet hooks removed");
}

#[cfg(not(windows))]
pub fn remove() {}

// ---------------------------------------------------------------------------
// Packet event handlers — called from HWBP callbacks
// ---------------------------------------------------------------------------

/// Called from the WSASend HWBP callback. Parses the EQ opcode from the first
/// two bytes of the WSABUF payload and enqueues a `PacketEvent` for the
/// orchestrator. Must not allocate, panic, or block.
fn on_packet_send(client_id: ClientId, buf: *const u8, len: usize) {
    // EQ packet header: first 2 bytes are the opcode (little-endian).
    // Minimum packet size is 2 bytes (opcode only).
    if len < 2 {
        return;
    }

    // SAFETY: caller verified buf is non-null and len >= 2.
    let opcode = unsafe { u16::from_le_bytes([*buf, *buf.add(1)]) };

    let timestamp_ms = current_timestamp_ms();

    crate::ipc::send_response(Response::PacketEvent {
        client_id,
        opcode,
        direction: PacketDirection::Outbound,
        timestamp_ms,
        payload_size: len as u32,
    });
}

/// Called from the WSARecv HWBP callback. Parses the EQ opcode from the first
/// two bytes of the received WSABUF payload and enqueues a `PacketEvent`.
/// Must not allocate, panic, or block.
fn on_packet_recv(client_id: ClientId, buf: *const u8, len: usize) {
    if len < 2 {
        return;
    }

    // SAFETY: caller verified buf is non-null and len >= 2.
    let opcode = unsafe { u16::from_le_bytes([*buf, *buf.add(1)]) };

    let timestamp_ms = current_timestamp_ms();

    crate::ipc::send_response(Response::PacketEvent {
        client_id,
        opcode,
        direction: PacketDirection::Inbound,
        timestamp_ms,
        payload_size: len as u32,
    });
}

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

    #[test]
    fn install_stub_succeeds() {
        assert!(install(0).is_ok());
    }

    #[test]
    fn remove_stub_does_not_panic() {
        remove();
    }

    #[test]
    fn wsasend_slot_is_dr2() {
        assert_eq!(WSASEND_SLOT, HwbpSlot::Dr2);
    }

    #[test]
    fn wsarecv_slot_is_dr3() {
        assert_eq!(WSARECV_SLOT, HwbpSlot::Dr3);
    }

    #[test]
    fn on_packet_send_short_buffer_does_not_panic() {
        // Buffer of only 1 byte should be a no-op (len < 2 guard).
        let buf = [0xABu8];
        on_packet_send(0, buf.as_ptr(), 1);
    }

    #[test]
    fn on_packet_recv_short_buffer_does_not_panic() {
        let buf = [0xCDu8];
        on_packet_recv(0, buf.as_ptr(), 1);
    }

    #[test]
    fn opcode_parsing_send() {
        // Two-byte opcode 0x1234 stored little-endian.
        let buf = [0x34u8, 0x12, 0xDE, 0xAD, 0xBE, 0xEF];
        // Just verify we don't panic — actual IPC state not inspected in unit tests.
        on_packet_send(42, buf.as_ptr(), buf.len());
    }

    #[test]
    fn opcode_parsing_recv() {
        let buf = [0x78u8, 0x56, 0x00, 0x01];
        on_packet_recv(42, buf.as_ptr(), buf.len());
    }

    #[test]
    fn current_timestamp_ms_is_nonzero() {
        assert!(current_timestamp_ms() > 0);
    }

    #[test]
    fn wsa_send_callback_stub_returns_true() {
        #[cfg(not(windows))]
        assert!(wsa_send_callback(std::ptr::null_mut()));
    }

    #[test]
    fn wsa_recv_callback_stub_returns_true() {
        #[cfg(not(windows))]
        assert!(wsa_recv_callback(std::ptr::null_mut()));
    }
}
