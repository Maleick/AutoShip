//! Direct memory-read access to the EQ chat window manager.
//!
//! Reads chat text stored inside `CChatWindowManager` → `CChatWindow` → `CStmlWnd`
//! → `STextLine` linked list.  This path gives access to chat *history* already
//! displayed in a chat window, complementing the `dsp_chat` HWBP hook which only
//! captures messages as they arrive.
//!
//! # Usage
//!
//! ```ignore
//! let lines = read_chat_window_lines(eq_base, window_index, max_lines);
//! let all_lines = read_all_chat_window_lines(eq_base, max_lines_per_window);
//! ```
//!
//! # Safety notes
//!
//! All reads are guarded with `is_readable` checks.  Out-of-range or null
//! pointers return an empty result rather than panicking.  Struct offsets are
//! sourced from MQ2 eqlib (client 20260310) and require live-client calibration
//! before use in a new patch cycle.

use textquest_common::ipc::ChatMessageInfo;
#[cfg(windows)]
use textquest_common::offsets;

// ─── Memory-read helpers ─────────────────────────────────────────────────────

/// Read a `usize`-width pointer from `addr`.
/// Returns `None` if the address is unreadable or the stored value is zero.
#[cfg(windows)]
fn read_ptr(addr: usize) -> Option<usize> {
    use crate::hooks::game_loop::is_readable;
    if !is_readable(addr, 8) {
        return None;
    }
    // SAFETY: is_readable confirmed 8 committed, accessible bytes at `addr`.
    let val = unsafe { std::ptr::read_unaligned(addr as *const usize) };
    if val == 0 { None } else { Some(val) }
}

/// Read a `u32` from `addr`.
#[cfg(windows)]
fn read_u32(addr: usize) -> Option<u32> {
    use crate::hooks::game_loop::is_readable;
    if !is_readable(addr, 4) {
        return None;
    }
    // SAFETY: is_readable confirmed 4 committed, accessible bytes at `addr`.
    Some(unsafe { std::ptr::read_unaligned(addr as *const u32) })
}

/// Read a `CXStr` at `addr` and return its UTF-8 content.
///
/// `CXStr` is a pointer to a `CStrRep` struct:
/// - +0x08: `u32` length in bytes
/// - +0x18: `char[]` data start
///
/// Offsets come from `textquest_common::offsets::eqmain`.
#[cfg(windows)]
fn read_cxstr(addr: usize) -> Option<String> {
    use crate::hooks::game_loop::is_readable;

    let rep_ptr = read_ptr(addr)?;
    let len = read_u32(rep_ptr + offsets::eqmain::CSTRREP_LENGTH)? as usize;
    if len == 0 || len > 4096 {
        return None;
    }
    let data_addr = rep_ptr + offsets::eqmain::CSTRREP_DATA;
    if !is_readable(data_addr, len) {
        return None;
    }
    // SAFETY: is_readable confirmed `len` accessible bytes at `data_addr`.
    let slice = unsafe { std::slice::from_raw_parts(data_addr as *const u8, len) };
    Some(String::from_utf8_lossy(slice).into_owned())
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Read recent lines from a single chat window by index.
///
/// `eq_base` — rebased eqgame.exe base address (from `EQ_BASE`).
/// `window_index` — zero-based index into `CChatWindowManager::ChatWndArray`.
/// `max_lines` — maximum number of lines to return (most-recent first).
///
/// Returns an empty `Vec` on non-Windows, on null/invalid pointers, or when
/// `window_index` is out of range.
pub fn read_chat_window_lines(
    eq_base: u64,
    window_index: usize,
    max_lines: usize,
) -> Vec<ChatMessageInfo> {
    #[cfg(windows)]
    {
        if eq_base == 0 || max_lines == 0 {
            return Vec::new();
        }
        // SAFETY: all pointer accesses inside are guarded by is_readable.
        unsafe { read_chat_window_lines_impl(eq_base, window_index, max_lines) }
    }
    #[cfg(not(windows))]
    {
        let _ = (eq_base, window_index, max_lines);
        Vec::new()
    }
}

/// Read recent lines from all active chat windows.
///
/// Windows are visited in order from `CChatWindowManager::ChatWndArray`.  Lines
/// from each window are appended sequentially; no deduplication is performed.
///
/// `max_lines_per_window` — maximum lines collected per individual window.
pub fn read_all_chat_window_lines(
    eq_base: u64,
    max_lines_per_window: usize,
) -> Vec<ChatMessageInfo> {
    #[cfg(windows)]
    {
        if eq_base == 0 || max_lines_per_window == 0 {
            return Vec::new();
        }
        // SAFETY: all pointer accesses inside are guarded by is_readable.
        unsafe { read_all_chat_window_lines_impl(eq_base, max_lines_per_window) }
    }
    #[cfg(not(windows))]
    {
        let _ = (eq_base, max_lines_per_window);
        Vec::new()
    }
}

// ─── Windows implementation ──────────────────────────────────────────────────

#[cfg(windows)]
unsafe fn resolve_chat_mgr(eq_base: u64) -> Option<usize> {
    if eq_base == 0 {
        return None;
    }
    let inst_addr = offsets::rebase(offsets::PINST_CCHAT_WINDOW_MANAGER, eq_base)?;
    read_ptr(inst_addr)
}

#[cfg(windows)]
unsafe fn read_chat_window_lines_impl(
    eq_base: u64,
    window_index: usize,
    max_lines: usize,
) -> Vec<ChatMessageInfo> {
    let mgr_ptr = match unsafe { resolve_chat_mgr(eq_base) } {
        Some(p) => p,
        None => return Vec::new(),
    };

        let num_windows = match read_u32(mgr_ptr + offsets::chat_window_mgr::NUM_WINDOWS) {
            Some(n) => n as usize,
            None => return Vec::new(),
        };
        if window_index >= num_windows {
            return Vec::new();
        }

        // ChatWndArray holds a `CChatWindow**` (pointer to pointer array).
        let array_ptr = match read_ptr(mgr_ptr + offsets::chat_window_mgr::CHAT_WND_ARRAY) {
            Some(p) => p,
            None => return Vec::new(),
        };
        let window_ptr = match read_ptr(array_ptr + window_index * size_of::<usize>()) {
            Some(p) => p,
            None => return Vec::new(),
        };

    unsafe { read_stml_lines(window_ptr, max_lines) }
}

#[cfg(windows)]
unsafe fn read_all_chat_window_lines_impl(
    eq_base: u64,
    max_lines_per_window: usize,
) -> Vec<ChatMessageInfo> {
    let mgr_ptr = match unsafe { resolve_chat_mgr(eq_base) } {
        Some(p) => p,
        None => return Vec::new(),
    };

        let num_windows = match read_u32(mgr_ptr + offsets::chat_window_mgr::NUM_WINDOWS) {
            Some(n) => n as usize,
            None => return Vec::new(),
        };
        let num_windows = num_windows.min(offsets::chat_window_mgr::MAX_CHAT_WINDOWS);

        let array_ptr = match read_ptr(mgr_ptr + offsets::chat_window_mgr::CHAT_WND_ARRAY) {
            Some(p) => p,
            None => return Vec::new(),
        };

    let mut all_lines = Vec::new();
    for idx in 0..num_windows {
        if let Some(window_ptr) = read_ptr(array_ptr + idx * size_of::<usize>()) {
            let lines = unsafe { read_stml_lines(window_ptr, max_lines_per_window) };
            all_lines.extend(lines);
        }

        all_lines
    }
}

/// Walk the `STextLine` doubly-linked list inside `CChatWindow::OutputWnd`
/// (a `CStmlWnd`) and collect up to `max_lines` rendered text lines.
///
/// Lines are returned most-recent first (tail-to-head order).
/// Color metadata is not available at this path (use the `dsp_chat` hook for that).
#[cfg(windows)]
unsafe fn read_stml_lines(window_ptr: usize, max_lines: usize) -> Vec<ChatMessageInfo> {
    if max_lines == 0 {
        return Vec::new();
    }

    // CChatWindow::OutputWnd → CStmlWnd*
    let output_wnd = match read_ptr(window_ptr + offsets::chat_window::OUTPUT_WND) {
        Some(p) => p,
        None => return Vec::new(),
    };

    // CStmlWnd::TextLines is a doubly-linked list with a sentinel head at
    // `output_wnd + TEXT_LINES`.  Real nodes start at head.Next.
    let list_head_addr = output_wnd + offsets::cstml_wnd::TEXT_LINES;
    let first_node = match read_ptr(list_head_addr + offsets::stext_line::NEXT) {
        Some(p) => p,
        None => return Vec::new(),
    };

    // Traverse forward to collect all node pointers, then return the last
    // `max_lines` in reverse order so the caller sees [most-recent, ..., oldest].
    let mut node_ptrs: Vec<usize> = Vec::new();
    let mut current = first_node;
    const MAX_WALK: usize = 4096; // guard against corrupt list
    let mut walked = 0usize;

    while current != 0 && current != list_head_addr && walked < MAX_WALK {
        node_ptrs.push(current);
        current = match read_ptr(current + offsets::stext_line::NEXT) {
            Some(p) => p,
            None => break,
        };
        walked += 1;
    }

    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    node_ptrs
        .iter()
        .rev()
        .take(max_lines)
        .filter_map(|&ptr| {
            let text = read_cxstr(ptr + offsets::stext_line::TEXT)?;
            if text.is_empty() {
                return None;
            }
            Some(ChatMessageInfo {
                text,
                color: 0, // color is not stored in STextLine; use dsp_chat for per-message color
                timestamp_ms,
            })
        })
        .collect()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn read_chat_window_lines_returns_empty_on_non_windows() {
        // On macOS / Linux this must return empty without panicking.
        let result = read_chat_window_lines(0, 0, 100);
        assert!(result.is_empty());
    }

    #[cfg(not(windows))]
    #[test]
    fn read_all_chat_window_lines_returns_empty_on_non_windows() {
        let result = read_all_chat_window_lines(0, 100);
        assert!(result.is_empty());
    }

    #[test]
    fn read_chat_window_lines_zero_max_lines_always_empty() {
        // Requesting 0 lines must return empty on all platforms.
        let result = read_chat_window_lines(0, 0, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn read_all_chat_window_lines_zero_max_per_window_always_empty() {
        let result = read_all_chat_window_lines(0, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn read_all_chat_window_lines_null_eq_base_returns_empty() {
        let result = read_all_chat_window_lines(0, 50);
        assert!(result.is_empty());
    }

    #[test]
    fn read_chat_window_lines_null_eq_base_returns_empty() {
        // eq_base=0 should short-circuit before any rebase or pointer reads.
        let result = read_chat_window_lines(0, 5, 50);
        assert!(result.is_empty());
    }
}
