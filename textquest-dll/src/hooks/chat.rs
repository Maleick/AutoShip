//! Chat message hook — intercepts `CEverQuest::dsp_chat` to capture all in-game text.
//!
//! Uses hardware breakpoint DR1. When EQ calls `dsp_chat`, the VEH handler fires,
//! we read the chat text and color from the function arguments, publish a
//! `Response::ChatMessage` via IPC, then resume the original function.
//!
//! `dsp_chat` signature (x64 Microsoft ABI):
//!   - RCX = this (CEverQuest*)
//!   - RDX = text (const char*)
//!   - R8  = color (int, default 273)
//!   - R9  = log (bool)
//!
//! We only need text and color. The remaining params (percent_convert, stml_safe,
//! chat_filter) are on the stack and not needed for capture.

use super::hwbp::{self, HwbpSlot};

const CHAT_HOOK_SLOT: HwbpSlot = HwbpSlot::Dr1;

/// HWBP callback for the chat hook.
///
/// Reads the `text` (RDX) and `color` (R8) registers from the exception context,
/// then publishes a `ChatMessage` response via IPC.
#[cfg(windows)]
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
fn chat_callback(exception_info: *mut ()) -> bool {
    // SAFETY: exception_info is a valid EXCEPTION_POINTERS from the VEH handler.
    let context = unsafe {
        let ptrs =
            exception_info as *const windows::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS;
        &*(*ptrs).ContextRecord
    };

    // x64 Microsoft ABI: RDX = text ptr, R8 = color
    let text_ptr = context.Rdx as *const u8;
    let color = context.R8 as i32;

    if !text_ptr.is_null() {
        // SAFETY: We verify every byte address is readable before dereferencing.
        let text = unsafe {
            let mut len = 0usize;
            while len < 4096 {
                let current = text_ptr.add(len);
                if !super::game_loop::is_readable(current as usize, 1) {
                    break;
                }
                if *current == 0 {
                    break;
                }
                len += 1;
            }
            let slice = std::slice::from_raw_parts(text_ptr, len);
            String::from_utf8_lossy(slice).into_owned()
        };

        if !text.is_empty() {
            let _ = crate::combat::observe_chat_message(&text);

            let timestamp_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            crate::ipc::send_response(textquest_common::ipc::Response::ChatMessage {
                text,
                color,
                timestamp_ms,
            });
        }
    }

    true
}

#[cfg(not(windows))]
fn chat_callback(_exception_info: *mut ()) -> bool {
    // macOS/Linux stub — no EQ process to hook.
    true
}

/// Install the chat hook on the resolved `dsp_chat` address.
pub fn install(dsp_chat_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
    hwbp::register(CHAT_HOOK_SLOT, dsp_chat_addr, chat_callback)?;
    tracing::info!(
        addr = format!("{:#x}", dsp_chat_addr),
        "Chat hook installed (DR1)"
    );
    Ok(())
}

/// Remove the chat hook.
pub fn remove() {
    if hwbp::is_active(CHAT_HOOK_SLOT) {
        if let Err(e) = hwbp::unregister(CHAT_HOOK_SLOT) {
            tracing::warn!("Failed to remove chat hook HWBP: {}", e);
        }
    }
    tracing::info!("Chat hook removed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_callback_stub_returns_true() {
        // On macOS the stub should always return true.
        #[cfg(not(windows))]
        assert!(chat_callback(std::ptr::null_mut()));
    }

    #[test]
    fn chat_hook_slot_is_dr1() {
        assert_eq!(CHAT_HOOK_SLOT, HwbpSlot::Dr1);
    }

    #[test]
    fn install_remove_roundtrip() {
        let dummy_addr = 0xDEAD_BEEF;

        if std::env::var_os("TEXTQUEST_RUN_HWBP_TESTS").is_none() {
            tracing::warn!(
                "Skipping chat hook install/remove roundtrip check; \
                 set TEXTQUEST_RUN_HWBP_TESTS=1 to opt in"
            );
            return;
        }

        #[cfg(windows)]
        match install(dummy_addr) {
            Ok(()) => {}
            Err(err)
                if err.to_string().contains("EverQuest window not found")
                    || err
                        .to_string()
                        .contains("GetWindowThreadProcessId returned 0") =>
            {
                tracing::warn!(
                    error = %err,
                    "Skipping chat hook install/remove roundtrip check without an EQ window"
                );
                return;
            }
            Err(err) => panic!("install(dummy_addr) failed: {err}"),
        }

        #[cfg(not(windows))]
        assert!(install(dummy_addr).is_ok());
        assert!(hwbp::is_active(HwbpSlot::Dr1));
        remove();
        assert!(!hwbp::is_active(HwbpSlot::Dr1));
    }
}
