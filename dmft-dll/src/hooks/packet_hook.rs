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
//! - `WSASend` (ws2_32.dll) — outbound packets
//! - `WSARecv` (ws2_32.dll) — inbound packets
//!
//! The hooks must be installed BEFORE the EQ scrambler layer to capture
//! cleartext opcodes. See `project_packet_hook_strategy` for details on
//! hooking send/recv before the scrambler.

use dmft_common::ipc::{PacketDirection, Response};
use dmft_common::types::ClientId;

/// Install WSASend/WSARecv hooks.
///
/// # Safety
///
/// Requires the game process to have ws2_32.dll loaded. Must be called
/// from the DLL initialization thread after the game loop is active.
#[cfg(windows)]
pub fn install(_client_id: ClientId) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Resolve WSASend and WSARecv addresses from ws2_32.dll
    //   let ws2 = GetModuleHandleA("ws2_32.dll")?;
    //   let wsa_send = GetProcAddress(ws2, "WSASend")?;
    //   let wsa_recv = GetProcAddress(ws2, "WSARecv")?;

    // TODO: Install detour or HWBP hooks on WSASend/WSARecv
    //   Use the same HWBP mechanism from hooks/hwbp.rs for stealth,
    //   or inline detours for simplicity during development.

    tracing::info!("Packet hooks installed (stub — no actual hooks yet)");
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
    // TODO: Unhook WSASend/WSARecv, restore original bytes/HWBP
    tracing::info!("Packet hooks removed (stub)");
}

#[cfg(not(windows))]
pub fn remove() {}

/// Called from the WSASend detour. Extracts the opcode and payload size
/// from the WSABUF, then enqueues a PacketEvent for the orchestrator.
///
/// # Safety
///
/// Called from a detour context — must not allocate, panic, or block.
#[allow(dead_code)]
fn on_packet_send(_client_id: ClientId, _buf: *const u8, _len: usize) {
    // TODO: Parse EQ packet header to extract opcode
    //   let opcode = u16::from_le_bytes([buf[0], buf[1]]);
    //   let timestamp_ms = current_timestamp_ms();
    //
    //   crate::ipc::send_response(Response::PacketEvent {
    //       client_id,
    //       opcode,
    //       direction: PacketDirection::Outbound,
    //       timestamp_ms,
    //       payload_size: len as u32,
    //   });
    let _ = (
        PacketDirection::Outbound,
        Response::Pong {
            client_id: 0,
            timestamp_ms: 0,
        },
    );
}

/// Called from the WSARecv detour. Extracts the opcode and payload size
/// from the received buffer, then enqueues a PacketEvent for the orchestrator.
///
/// # Safety
///
/// Called from a detour context — must not allocate, panic, or block.
#[allow(dead_code)]
fn on_packet_recv(_client_id: ClientId, _buf: *const u8, _len: usize) {
    // TODO: Parse EQ packet header to extract opcode
    //   let opcode = u16::from_le_bytes([buf[0], buf[1]]);
    //   let timestamp_ms = current_timestamp_ms();
    //
    //   crate::ipc::send_response(Response::PacketEvent {
    //       client_id,
    //       opcode,
    //       direction: PacketDirection::Inbound,
    //       timestamp_ms,
    //       payload_size: len as u32,
    //   });
    let _ = (
        PacketDirection::Inbound,
        Response::Pong {
            client_id: 0,
            timestamp_ms: 0,
        },
    );
}

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
}
