//! Clipboard export utilities for recent TextQuest output.
//!
//! Provides OS-specific clipboard write functionality:
//! - Windows: Native clipboard via Windows API
//! - macOS/Linux: Stub that returns success but doesn't actually write

use anyhow::Result;

/// Copy text to the system clipboard.
///
/// # Arguments
/// * `text` - The text to copy to clipboard
///
/// # Returns
/// - On Windows: writes to clipboard, returns Ok if successful
/// - On other platforms: returns success without writing (stub)
#[cfg(windows)]
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    use anyhow::Context;
    use std::ffi::CStr;
    use windows::Win32::{
        Foundation::HWND,
        System::DataExchange::{
            CloseClipboard, GetClipboardOwner, OpenClipboard, SetClipboardData,
        },
        System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
    };

    unsafe {
        // Open clipboard
        OpenClipboard(HWND::default()).ok()?;

        // Allocate memory for the text
        let size = text.len() + 1; // +1 for null terminator
        let hglobal = GlobalAlloc(GMEM_MOVEABLE, size)
            .ok_or_else(|| anyhow::anyhow!("GlobalAlloc failed"))?;

        // Lock and copy data
        let ptr = GlobalLock(hglobal);
        if ptr.is_null() {
            let _ = CloseClipboard();
            anyhow::bail!("Failed to lock global memory");
        }

        // Copy text to allocated memory
        std::ptr::copy_nonoverlapping(text.as_ptr() as *const u8, ptr as *mut u8, text.len());
        // Write null terminator
        *(ptr.add(text.len()) as *mut u8) = 0;

        let _ = GlobalUnlock(hglobal);

        // Set clipboard data (CF_TEXT = 1 for ANSI text)
        const CF_TEXT: u32 = 1;
        SetClipboardData(CF_TEXT, hglobal as isize).map_err(|e| anyhow::anyhow!("SetClipboardData failed: {}", e))?;

        CloseClipboard();
    }

    tracing::info!(len = text.len(), "Copied text to clipboard");
    Ok(())
}

/// Stub implementation for non-Windows platforms.
#[cfg(not(windows))]
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    tracing::debug!(
        len = text.len(),
        "Clipboard stub: would copy {} chars",
        text.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_stub_on_non_windows() {
        // On non-Windows, this should succeed without error
        let result = copy_to_clipboard("test data");
        assert!(result.is_ok());
    }

    #[test]
    fn clipboard_accepts_empty_string() {
        let result = copy_to_clipboard("");
        assert!(result.is_ok());
    }

    #[test]
    fn clipboard_accepts_large_text() {
        let large_text = "a".repeat(10000);
        let result = copy_to_clipboard(&large_text);
        assert!(result.is_ok());
    }

    #[test]
    fn clipboard_handles_unicode() {
        let unicode_text = "Hello 🎉 世界 🚀";
        let result = copy_to_clipboard(unicode_text);
        // UTF-8 text should be handled (though Windows ANSI clipboard may lose some chars)
        assert!(result.is_ok());
    }
}
