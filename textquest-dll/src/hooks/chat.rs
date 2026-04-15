//! Chat message hook — intercepts `CEverQuest::dsp_chat` to capture all in-game
//! text.
//!
//! Uses hardware breakpoint DR1. When EQ calls `dsp_chat`, the VEH handler
//! fires, we read the chat text and color from the function arguments, buffer
//! the message internally, then resume the original function. Buffered messages
//! are returned to the orchestrator when it sends `Command::PollChat`; the DLL
//! replies with `Response::ChatBatch`. No unsolicited IPC response is sent per
//! chat line.
//!
//! `dsp_chat` signature (x64 Microsoft ABI):
//!   - RCX = this (CEverQuest*)
//!   - RDX = text (const char*)
//!   - R8  = color (int, default 273)
//!   - R9  = log (bool)
//!
//! We only need text and color. The remaining params (percent_convert,
//! stml_safe, chat_filter) are on the stack and not needed for capture.

use super::hwbp::{self, HwbpSlot};

const CHAT_HOOK_SLOT: HwbpSlot = HwbpSlot::Dr1;

fn should_forward_to_combat(parsed: Option<&textquest_common::chat::ChatEvent>) -> bool {
    parsed.is_none()
}

/// HWBP callback for the chat hook.
///
/// Reads the `text` (RDX) and `color` (R8) registers from the exception
/// context, then buffers it for retrieval through the `PollChat` IPC command.
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
            let parsed = textquest_common::chat::parse_chat_text(&text);
            // Only feed non-channel chat/system-like lines into cast outcome parsing.
            // Structured player chat (say/tell/group/etc.) is untrusted and may contain
            // spoofed substrings like "you don't have enough mana".
            if should_forward_to_combat(parsed.as_ref()) {
                let _ = crate::combat::observe_chat_message(&text);
            }

            let timestamp_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            // Push into the dedicated chat buffer so it can be retrieved via
            // Command::PollChat / Response::ChatBatch without affecting the
            // packet-event pipeline.
            crate::ipc::push_chat_message(text, color, timestamp_ms);
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
    fn only_non_channel_text_forwards_to_combat_parser() {
        let say = textquest_common::chat::parse_chat_text("Alice says, 'hello there'");
        let you_say = textquest_common::chat::parse_chat_text("You say, 'hello there'");
        let you_shout = textquest_common::chat::parse_chat_text("You shout, 'WTS Fungi!'");
        let you_group = textquest_common::chat::parse_chat_text("You tell the group, 'INC 3'");
        let you_guild =
            textquest_common::chat::parse_chat_text("You say to your guild, 'Good fight!'");
        let you_ooc =
            textquest_common::chat::parse_chat_text("You say out of character, 'Anyone?'");
        let you_raid = textquest_common::chat::parse_chat_text("You tell the raid, 'Pull!'");
        let you_auction = textquest_common::chat::parse_chat_text("You auction, 'WTB Fungi'");
        // The spoof payload: self-authored say containing a cast-feedback phrase.
        let you_say_spoof = textquest_common::chat::parse_chat_text(
            "You say, 'You don\\'t have enough mana to cast this spell.'",
        );
        let system_like = textquest_common::chat::parse_chat_text("You don't have enough mana.");

        // Parsed player chat should never be forwarded.
        assert!(!should_forward_to_combat(say.as_ref()));

        // Self-authored channel text is still structured player chat and should not
        // be treated like combat/system text, even if parser coverage lags behind.
        assert!(!should_forward_to_combat(you_say.as_ref()));

        // Unparsed system-like text should continue to forward.

        // Self-authored channel text is structured player chat — must not be forwarded
        // even though it arrives as "You <verb>, '...'" rather than "Sender <verb>,
        // '...'".
        assert!(!should_forward_to_combat(you_say.as_ref()));
        assert!(!should_forward_to_combat(you_shout.as_ref()));
        assert!(!should_forward_to_combat(you_group.as_ref()));
        assert!(!should_forward_to_combat(you_guild.as_ref()));
        assert!(!should_forward_to_combat(you_ooc.as_ref()));
        assert!(!should_forward_to_combat(you_raid.as_ref()));
        assert!(!should_forward_to_combat(you_auction.as_ref()));
        assert!(!should_forward_to_combat(you_say_spoof.as_ref()));

        // Unparsed system-like text (real cast feedback) must still be forwarded.
        assert!(should_forward_to_combat(system_like.as_ref()));
    }

    #[test]
    fn install_remove_roundtrip() {
        let _guard = hwbp::test_guard();
        hwbp::remove_all();
        let dummy_addr = 0xDEAD_BEEF;

        if std::env::var_os("TEXTQUEST_RUN_HWBP_TESTS").is_none() {
            tracing::warn!(
                "Skipping chat hook install/remove roundtrip check; set \
                 TEXTQUEST_RUN_HWBP_TESTS=1 to opt in"
            );
            hwbp::remove_all();
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
        hwbp::remove_all();
    }
}
