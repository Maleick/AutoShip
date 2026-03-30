//! Shared EQ widget interaction primitives.
//!
//! These functions wrap confirmed-working patterns for manipulating EQ's UI
//! widgets from injected DLL code. They are used by the login system, IPC
//! handler, and game loop for all CXWnd/CXStr/button interactions.
//!
//! # What works (confirmed live 2026-03-29)
//!
//! - **`find_window_by_name()`** — CXWndManager window array scan, exact match by WindowText
//! - **`find_window_by_text_contains()`** — same scan, substring match (for fuzzy UI detection)
//! - **`find_child_button_by_text()`** — walk CXWnd child TList for button lookup
//! - **`read_cxstr()`** — read a CXStr value from a raw CStrRep pointer
//! - **`write_cxstr_inplace()`** — overwrite an existing CStrRep buffer (non-null only)
//! - **`clone_cstrrep()`** — HeapAlloc a new CStrRep cloned from a donor (for null CXStr fields)
//! - **`click_button_via_vtable()`** — WndNotification(XWM_LCLICK) through CXWnd vtable
//!
//! # What does NOT work
//!
//! - **`set_edit_text_via_vtable()`** — SetWindowText vtable 0x280 does not work in eqmain.dll
//!   context. Kept for reference and possible eqgame.exe use. Use `write_cxstr_inplace()` instead.
//! - **PostMessageW (WM_CHAR/VK_RETURN)** — EQ uses DirectInput, not Win32 message pump.
//! - **EQLogin char array write alone** — UI doesn't read from backend arrays; must also write CXStr.
//!
//! # Thread safety
//!
//! Button clicks via vtable **must** be called from EQ's main game loop thread.
//! Use `game_loop::queue_button_click()` to safely schedule clicks from IPC threads.
//! CXStr reads/writes are safe from any thread as long as the game isn't concurrently
//! modifying the same widget (which it won't during login screens).
//!
//! # CXStr memory layout
//!
//! ```text
//! CXStr = pointer to CStrRep
//! CStrRep layout:
//!   +0x00  refCount  (i32)
//!   +0x04  alloc     (u32)   — allocated buffer capacity
//!   +0x08  length    (u32)   — current string length
//!   +0x0C  encoding  (u32)   — 0=ASCII
//!   +0x10  freeList  (usize) — EQ's CXFreeList pointer (critical for dealloc)
//!   +0x18  data[]    (bytes) — null-terminated string data
//! ```

/// Maximum number of windows to scan (safety limit against corrupted data).
const MAX_WINDOW_COUNT: u32 = 500;

/// Maximum child nodes to walk (prevents infinite loops on corrupted TLists).
const MAX_CHILD_WALK: u32 = 200;

// ─── Window Finding ───

/// Find a SIDL window by exact WindowText match (case-insensitive).
///
/// Walks CXWndManager's `pWindows` array and compares each window's
/// `WindowText` CXStr against `name`.
///
/// # Arguments
/// * `cxwnd_mgr` — resolved CXWndManager pointer (from `eqmain::resolve_cxwnd_manager()`)
/// * `name` — exact window text to match (case-insensitive)
///
/// # Returns
/// Raw pointer to the CXWnd, or `None` if not found.
///
/// # Safety
/// `cxwnd_mgr` must be a valid CXWndManager pointer. Called from game process context.
#[cfg(windows)]
pub unsafe fn find_window_by_name(cxwnd_mgr: usize, name: &str) -> Option<usize> {
    use dmft_common::offsets::eqmain as off;

    let array_ptr = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_ARRAY) as *const usize);
    let count = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_COUNT) as *const u32);

    if array_ptr == 0 || count == 0 || count > MAX_WINDOW_COUNT {
        return None;
    }

    for i in 0..count as usize {
        let wnd_ptr = *((array_ptr + i * 8) as *const usize);
        if wnd_ptr == 0 { continue; }

        if let Some(text) = read_cxstr(wnd_ptr + off::CXWND_WINDOW_TEXT) {
            if text.eq_ignore_ascii_case(name) {
                return Some(wnd_ptr);
            }
        }
    }
    None
}

#[cfg(not(windows))]
pub unsafe fn find_window_by_name(_cxwnd_mgr: usize, _name: &str) -> Option<usize> {
    None
}

/// Find a window whose WindowText contains `substring` (case-insensitive).
///
/// Used for fuzzy matching pre-login screens whose exact text varies between patches.
///
/// # Safety
/// `cxwnd_mgr` must be a valid CXWndManager pointer.
#[cfg(windows)]
pub unsafe fn find_window_by_text_contains(cxwnd_mgr: usize, substring: &str) -> Option<usize> {
    use dmft_common::offsets::eqmain as off;
    let needle = substring.to_ascii_lowercase();

    let array_ptr = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_ARRAY) as *const usize);
    let count = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_COUNT) as *const u32);

    if array_ptr == 0 || count == 0 || count > MAX_WINDOW_COUNT {
        return None;
    }

    for i in 0..count as usize {
        let wnd_ptr = *((array_ptr + i * 8) as *const usize);
        if wnd_ptr == 0 { continue; }

        if let Some(text) = read_cxstr(wnd_ptr + off::CXWND_WINDOW_TEXT) {
            if text.to_ascii_lowercase().contains(&needle) {
                return Some(wnd_ptr);
            }
        }
    }
    None
}

#[cfg(not(windows))]
pub unsafe fn find_window_by_text_contains(_cxwnd_mgr: usize, _substring: &str) -> Option<usize> {
    None
}

/// Walk a parent window's child TList looking for a child whose WindowText
/// contains `button_text` (case-insensitive). Recurses one level into grandchildren.
///
/// CXWnd children: first child at `CXWND_FIRST_NODE`, next sibling at `CXWND_NEXT`.
///
/// # Safety
/// `parent_wnd` must be a valid CXWnd pointer.
#[cfg(windows)]
pub unsafe fn find_child_button_by_text(parent_wnd: usize, button_text: &str) -> Option<usize> {
    use dmft_common::offsets::eqmain as off;
    let needle = button_text.to_ascii_lowercase();

    let mut child = *((parent_wnd + off::CXWND_FIRST_NODE) as *const usize);
    let mut count = 0u32;

    while child != 0 && count < MAX_CHILD_WALK {
        count += 1;

        if let Some(text) = read_cxstr(child + off::CXWND_WINDOW_TEXT) {
            if text.to_ascii_lowercase().contains(&needle) {
                return Some(child);
            }
        }

        // Recurse one level into grandchildren
        let mut grandchild = *((child + off::CXWND_FIRST_NODE) as *const usize);
        let mut gc_count = 0u32;
        while grandchild != 0 && gc_count < MAX_CHILD_WALK {
            gc_count += 1;
            if let Some(text) = read_cxstr(grandchild + off::CXWND_WINDOW_TEXT) {
                if text.to_ascii_lowercase().contains(&needle) {
                    return Some(grandchild);
                }
            }
            grandchild = *((grandchild + off::CXWND_NEXT) as *const usize);
        }

        child = *((child + off::CXWND_NEXT) as *const usize);
    }
    None
}

#[cfg(not(windows))]
pub unsafe fn find_child_button_by_text(_parent_wnd: usize, _button_text: &str) -> Option<usize> {
    None
}

/// Iterate all windows in CXWndManager, calling `callback(index, wnd_ptr, window_text)`.
/// Stops early if the callback returns `false`.
///
/// # Safety
/// `cxwnd_mgr` must be a valid CXWndManager pointer.
#[cfg(windows)]
pub unsafe fn for_each_window<F>(cxwnd_mgr: usize, mut callback: F)
where
    F: FnMut(usize, usize, Option<&str>) -> bool,
{
    use dmft_common::offsets::eqmain as off;

    let array_ptr = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_ARRAY) as *const usize);
    let count = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_COUNT) as *const u32);

    if array_ptr == 0 || count == 0 || count > MAX_WINDOW_COUNT {
        return;
    }

    for i in 0..count as usize {
        let wnd_ptr = *((array_ptr + i * 8) as *const usize);
        if wnd_ptr == 0 { continue; }

        let text = read_cxstr(wnd_ptr + off::CXWND_WINDOW_TEXT);
        if !callback(i, wnd_ptr, text.as_deref()) {
            break;
        }
    }
}

#[cfg(not(windows))]
pub unsafe fn for_each_window<F>(_cxwnd_mgr: usize, _callback: F)
where
    F: FnMut(usize, usize, Option<&str>) -> bool,
{
}

// ─── CXStr Read/Write ───

/// Read a CXStr value from a raw address.
///
/// CXStr is a single pointer to CStrRep. Returns `None` if the CStrRep pointer
/// is null, the length is 0, or the length exceeds 256 (likely corrupt).
///
/// # Safety
/// `cxstr_addr` must point to a valid CXStr field (a `usize` holding a CStrRep pointer).
#[cfg(windows)]
pub unsafe fn read_cxstr(cxstr_addr: usize) -> Option<String> {
    use dmft_common::offsets::eqmain as off;

    let rep_ptr = *(cxstr_addr as *const usize);
    if rep_ptr == 0 || rep_ptr < 0x10000 {
        return None;
    }

    let length = *((rep_ptr + off::CSTRREP_LENGTH) as *const u32) as usize;
    if length == 0 || length > 256 {
        return None;
    }

    let data_ptr = (rep_ptr + off::CSTRREP_DATA) as *const u8;
    let bytes = std::slice::from_raw_parts(data_ptr, length);
    String::from_utf8(bytes.to_vec()).ok()
}

#[cfg(not(windows))]
pub unsafe fn read_cxstr(_cxstr_addr: usize) -> Option<String> {
    None
}

/// Write a string into an existing CXStr's CStrRep buffer.
///
/// Overwrites the data in-place. Fails if the CStrRep is null (use `clone_cstrrep()`
/// to allocate one first) or if the text exceeds the allocated buffer size.
///
/// # Safety
/// `cxstr_addr` must point to a valid CXStr field with a non-null CStrRep.
/// The CStrRep buffer must have enough allocated space for `text`.
#[cfg(windows)]
pub unsafe fn write_cxstr_inplace(cxstr_addr: usize, text: &str) -> bool {
    use dmft_common::offsets::eqmain as off;

    let rep_ptr = *(cxstr_addr as *const usize);
    if rep_ptr == 0 {
        tracing::warn!("CXStr rep is null — need donor CStrRep");
        return false;
    }

    let alloc = *((rep_ptr + off::CSTRREP_ALLOC) as *const u32) as usize;
    if text.len() >= alloc {
        tracing::warn!(text_len = text.len(), alloc, "CXStr buffer too small for text");
        return false;
    }

    let data_ptr = (rep_ptr + off::CSTRREP_DATA) as *mut u8;
    std::ptr::copy_nonoverlapping(text.as_ptr(), data_ptr, text.len());
    *data_ptr.add(text.len()) = 0; // null-terminate
    *((rep_ptr + off::CSTRREP_LENGTH) as *mut u32) = text.len() as u32;

    true
}

#[cfg(not(windows))]
pub unsafe fn write_cxstr_inplace(_cxstr_addr: usize, _text: &str) -> bool {
    false
}

/// Clone a CStrRep from a donor, allocating on the Windows process heap.
///
/// EQ manages CStrRep memory through its own CXFreeList mechanism. Using the
/// process default heap (via `HeapAlloc`) ensures EQ can safely free the buffer.
/// The freeList pointer is copied from the donor so EQ's deallocator works correctly.
///
/// Returns the address of the new CStrRep, or `None` on allocation failure.
///
/// # Safety
/// `donor_rep` must be a valid CStrRep pointer. The returned CStrRep is empty
/// (length=0) and must be written to via `write_cxstr_inplace()`.
#[cfg(windows)]
pub unsafe fn clone_cstrrep(donor_rep: usize) -> Option<usize> {
    use dmft_common::offsets::eqmain as off;
    use windows::Win32::System::Memory::{GetProcessHeap, HeapAlloc, HEAP_ZERO_MEMORY};

    let donor_alloc = *((donor_rep + off::CSTRREP_ALLOC) as *const u32) as usize;
    let total_size = off::CSTRREP_DATA + donor_alloc.max(128);

    let heap = GetProcessHeap().ok()?;
    let new_rep = HeapAlloc(heap, HEAP_ZERO_MEMORY, total_size);
    if new_rep.is_null() {
        tracing::error!("HeapAlloc failed for CStrRep clone");
        return None;
    }

    let new_rep_addr = new_rep as usize;

    // Fill header from donor
    *(new_rep_addr as *mut i32) = 1; // refCount = 1
    *((new_rep_addr + off::CSTRREP_ALLOC) as *mut u32) = donor_alloc.max(128) as u32;
    *((new_rep_addr + off::CSTRREP_LENGTH) as *mut u32) = 0; // empty initially
    *((new_rep_addr + off::CSTRREP_ENCODING) as *mut u32) =
        *((donor_rep + off::CSTRREP_ENCODING) as *const u32);
    // Copy freeList pointer — critical for EQ's deallocation
    *((new_rep_addr + 0x10) as *mut usize) = *((donor_rep + 0x10) as *const usize);

    Some(new_rep_addr)
}

#[cfg(not(windows))]
pub unsafe fn clone_cstrrep(_donor_rep: usize) -> Option<usize> {
    None
}

/// Allocate a fresh CStrRep on the process heap with the given text.
///
/// Unlike `clone_cstrrep()`, this creates a standalone CStrRep without a donor.
/// The freeList is set to 0 (no CXFreeList). EQ will still free it via HeapFree
/// when the refCount drops to 0.
///
/// Ownership is transferred to EQ — do NOT free the returned pointer from Rust.
///
/// # Safety
/// The returned pointer must only be stored in a CXStr field that EQ manages.
#[cfg(windows)]
pub unsafe fn alloc_cstrrep(text: &str) -> Option<usize> {
    use dmft_common::offsets::eqmain as off;
    use windows::Win32::System::Memory::{GetProcessHeap, HeapAlloc, HEAP_ZERO_MEMORY};

    let text_len = text.len();
    let alloc_size = text_len + 64; // extra room
    let total_size = off::CSTRREP_DATA + alloc_size;

    let heap = match GetProcessHeap() {
        Ok(h) => h,
        Err(_) => {
            tracing::error!("GetProcessHeap failed");
            return None;
        }
    };
    let rep = HeapAlloc(heap, HEAP_ZERO_MEMORY, total_size);
    if rep.is_null() {
        tracing::error!("HeapAlloc failed for CStrRep");
        return None;
    }
    let rep_addr = rep as usize;

    *(rep_addr as *mut i32) = 1; // refCount = 1
    *((rep_addr + off::CSTRREP_ALLOC) as *mut u32) = alloc_size as u32;
    *((rep_addr + off::CSTRREP_LENGTH) as *mut u32) = text_len as u32;
    *((rep_addr + off::CSTRREP_ENCODING) as *mut u32) = 0; // ASCII

    let data_ptr = (rep_addr + off::CSTRREP_DATA) as *mut u8;
    std::ptr::copy_nonoverlapping(text.as_ptr(), data_ptr, text_len);
    *data_ptr.add(text_len) = 0; // null terminate

    Some(rep_addr)
}

#[cfg(not(windows))]
pub unsafe fn alloc_cstrrep(_text: &str) -> Option<usize> {
    None
}

// ─── Button Clicking ───

/// Click a button widget by calling `WndNotification(XWM_LCLICK)` through the CXWnd vtable.
///
/// This is the confirmed working approach for all EQ button interactions — login,
/// splash dismiss, server select, character select, etc.
///
/// # Thread safety
/// **Must** be called from EQ's main game loop thread. Use
/// `game_loop::queue_button_click()` to schedule clicks from other threads.
///
/// # Safety
/// `button_wnd` must be a valid CXWnd pointer with an intact vtable.
#[cfg(windows)]
pub unsafe fn click_button_via_vtable(button_wnd: usize) {
    use dmft_common::offsets::eqmain as off;

    let vtable = *(button_wnd as *const usize);
    if vtable == 0 {
        tracing::warn!("Button vtable is null");
        return;
    }

    let wnd_notification_ptr = *((vtable + off::CXWND_VTABLE_WND_NOTIFICATION) as *const usize);
    if wnd_notification_ptr == 0 {
        tracing::warn!("WndNotification function pointer is null");
        return;
    }

    // x64 calling convention: rcx=this, rdx=sender, r8=message, r9=data
    type WndNotificationFn = unsafe extern "C" fn(usize, usize, u32, usize) -> i32;
    let func: WndNotificationFn = std::mem::transmute(wnd_notification_ptr);
    func(button_wnd, button_wnd, off::XWM_LCLICK, 0);
}

#[cfg(not(windows))]
pub unsafe fn click_button_via_vtable(_button_wnd: usize) {}

/// Set text on a CEditWnd by calling SetWindowText through the vtable.
///
/// **WARNING: Does NOT work in eqmain.dll context** (login screens). The vtable
/// function at offset 0x280 appears to be a different virtual in eqmain's CXWnd
/// class hierarchy. Kept for potential use in eqgame.exe (character select, chat).
///
/// Uses `alloc_cstrrep()` to create a heap-allocated CStrRep and transfers
/// ownership to EQ via refCount.
///
/// # Safety
/// `edit_wnd` must be a valid CEditWnd pointer. Only call from game loop thread.
#[cfg(windows)]
pub unsafe fn set_edit_text_via_vtable(edit_wnd: usize, text: &str) -> bool {
    use dmft_common::offsets::eqmain as off;

    let vtable = *(edit_wnd as *const usize);
    if vtable == 0 {
        tracing::warn!("CEditWnd vtable is null");
        return false;
    }

    let set_window_text_ptr =
        *((vtable + off::CXWND_VTABLE_SET_WINDOW_TEXT) as *const usize);
    if set_window_text_ptr == 0 {
        tracing::warn!("SetWindowText function pointer is null");
        return false;
    }

    let Some(rep_addr) = alloc_cstrrep(text) else {
        return false;
    };

    // CXStr is just a pointer to CStrRep. SetWindowText takes `const CXStr&`
    // which means a pointer to the CXStr (pointer to pointer to CStrRep).
    let cxstr: usize = rep_addr;
    let cxstr_ref: *const usize = &cxstr;

    // x64: RCX=this(edit_wnd), RDX=&CXStr
    type SetWindowTextFn = unsafe extern "C" fn(usize, *const usize);
    let func: SetWindowTextFn = std::mem::transmute(set_window_text_ptr);
    func(edit_wnd, cxstr_ref);

    tracing::info!(
        wnd = format!("{:#x}", edit_wnd),
        vtable_fn = format!("{:#x}", set_window_text_ptr),
        text_len = text.len(),
        "Called CEditWnd::SetWindowText via vtable"
    );

    // Don't free the CStrRep — EQ now owns it via refCount.
    true
}

#[cfg(not(windows))]
pub unsafe fn set_edit_text_via_vtable(_edit_wnd: usize, _text: &str) -> bool {
    false
}

// ─── Window Property Reads ───

/// Read the visibility flag (`dShow`) from a CXWnd.
///
/// # Safety
/// `wnd_ptr` must be a valid CXWnd pointer.
#[cfg(windows)]
pub unsafe fn is_visible(wnd_ptr: usize) -> bool {
    use dmft_common::offsets::eqmain as off;
    *((wnd_ptr + off::CXWND_DSHOW) as *const u8) != 0
}

#[cfg(not(windows))]
pub unsafe fn is_visible(_wnd_ptr: usize) -> bool {
    false
}

/// Read the XMLIndex from a CXWnd.
///
/// # Safety
/// `wnd_ptr` must be a valid CXWnd pointer.
#[cfg(windows)]
pub unsafe fn xml_index(wnd_ptr: usize) -> i32 {
    use dmft_common::offsets::eqmain as off;
    *((wnd_ptr + off::CXWND_XML_INDEX) as *const i32)
}

#[cfg(not(windows))]
pub unsafe fn xml_index(_wnd_ptr: usize) -> i32 {
    -1
}

// ─── SIDL-Based Window Finding ───

/// Find a visible window by its SIDL name (CSidlScreenWnd::SidlText at +0x270).
///
/// This is the MQ2 AutoLogin approach: scan CXWndManager's window array, read
/// each window's SidlText, and check the dShow visibility flag. SIDL names
/// are stable across patches (e.g., "connect", "serverselect", "yesnodialog").
///
/// Works for both eqmain.dll and eqgame.exe contexts — the caller provides the
/// correct CXWndManager pointer and specifies which offsets to use.
///
/// # Arguments
/// * `cxwnd_mgr` — resolved CXWndManager pointer
/// * `sidl_name` — SIDL name to match (case-insensitive)
/// * `sidl_text_offset` — offset of SidlText in the window struct (differs between eqmain/eqgame)
/// * `array_offset` — CXWndManager array offset
/// * `count_offset` — CXWndManager count offset
///
/// # Returns
/// Raw pointer to the CXWnd if found and visible, or `None`.
///
/// # Safety
/// `cxwnd_mgr` must be a valid CXWndManager pointer.
#[cfg(windows)]
pub unsafe fn find_visible_window_by_sidl_name(
    cxwnd_mgr: usize,
    sidl_name: &str,
    sidl_text_offset: usize,
    array_offset: usize,
    count_offset: usize,
) -> Option<usize> {
    if cxwnd_mgr == 0 {
        return None;
    }

    let array_ptr = *((cxwnd_mgr + array_offset) as *const usize);
    let count = *((cxwnd_mgr + count_offset) as *const u32);

    if array_ptr == 0 || count == 0 || count > MAX_WINDOW_COUNT {
        return None;
    }

    for i in 0..count as usize {
        let wnd_ptr = *((array_ptr + i * 8) as *const usize);
        if wnd_ptr == 0 { continue; }

        // Check dShow first (cheap) before reading SidlText
        if !is_visible(wnd_ptr) { continue; }

        if let Some(text) = read_cxstr(wnd_ptr + sidl_text_offset) {
            if text.eq_ignore_ascii_case(sidl_name) {
                return Some(wnd_ptr);
            }
        }
    }
    None
}

#[cfg(not(windows))]
pub unsafe fn find_visible_window_by_sidl_name(
    _cxwnd_mgr: usize,
    _sidl_name: &str,
    _sidl_text_offset: usize,
    _array_offset: usize,
    _count_offset: usize,
) -> Option<usize> {
    None
}

/// Find a window by WindowText, but only if it's visible (dShow != 0).
///
/// Like `find_window_by_name()` but also checks the visibility flag.
///
/// # Safety
/// `cxwnd_mgr` must be a valid CXWndManager pointer.
#[cfg(windows)]
pub unsafe fn find_visible_window_by_name(cxwnd_mgr: usize, name: &str) -> Option<usize> {
    use dmft_common::offsets::eqmain as off;

    if cxwnd_mgr == 0 {
        return None;
    }

    let array_ptr = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_ARRAY) as *const usize);
    let count = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_COUNT) as *const u32);

    if array_ptr == 0 || count == 0 || count > MAX_WINDOW_COUNT {
        return None;
    }

    for i in 0..count as usize {
        let wnd_ptr = *((array_ptr + i * 8) as *const usize);
        if wnd_ptr == 0 { continue; }

        if !is_visible(wnd_ptr) { continue; }

        if let Some(text) = read_cxstr(wnd_ptr + off::CXWND_WINDOW_TEXT) {
            if text.eq_ignore_ascii_case(name) {
                return Some(wnd_ptr);
            }
        }
    }
    None
}

#[cfg(not(windows))]
pub unsafe fn find_visible_window_by_name(_cxwnd_mgr: usize, _name: &str) -> Option<usize> {
    None
}

// ─── CListWnd Item Reading ───

/// Find a child window by SidlText (eqgame.exe offsets).
///
/// Walks the CXWnd child TList (FirstNode/Next) and compares each child's
/// SidlText (CSidlScreenWnd +0x270) against `sidl_name` (case-insensitive).
///
/// # Safety
/// `parent_wnd` must be a valid CXWnd pointer in eqgame.exe context.
#[cfg(windows)]
pub unsafe fn find_child_by_sidl_text(parent_wnd: usize, sidl_name: &str) -> Option<usize> {
    use dmft_common::offsets::eqmain as off;
    use dmft_common::offsets::eqgame as eqg;

    let mut child = *((parent_wnd + off::CXWND_FIRST_NODE) as *const usize);
    let mut count = 0u32;

    while child != 0 && count < MAX_CHILD_WALK {
        count += 1;

        if let Some(sidl_text) = read_cxstr(child + eqg::CSIDL_SCREEN_WND_SIDL_TEXT) {
            if sidl_text.eq_ignore_ascii_case(sidl_name) {
                return Some(child);
            }
        }

        child = *((child + off::CXWND_NEXT) as *const usize);
    }
    None
}

#[cfg(not(windows))]
pub unsafe fn find_child_by_sidl_text(_parent_wnd: usize, _sidl_name: &str) -> Option<usize> {
    None
}

/// Read text from a CListWnd cell at (row, column).
///
/// Walks the CListWnd's ItemsArray → SListWndLine → SListWndCell → Text (CXStr).
/// Returns `None` if the row/column is out of bounds or the text is empty/corrupt.
///
/// # Safety
/// `list_wnd` must be a valid CListWnd pointer.
#[cfg(windows)]
pub unsafe fn read_list_item_text(list_wnd: usize, row: usize, col: usize) -> Option<String> {
    use dmft_common::offsets::eqgame as eqg;

    let row_count = *((list_wnd + eqg::CLISTWND_ITEMS_COUNT) as *const i32);
    if row_count <= 0 || row >= row_count as usize {
        return None;
    }

    let row_array = *((list_wnd + eqg::CLISTWND_ITEMS_ARRAY) as *const usize);
    if row_array == 0 {
        return None;
    }

    let row_ptr = row_array + row * eqg::SLISTWNDLINE_SIZE;

    let cell_count = *((row_ptr + eqg::SLISTWNDLINE_CELLS_COUNT) as *const i32);
    if cell_count <= 0 || col >= cell_count as usize {
        return None;
    }

    let cell_array = *((row_ptr + eqg::SLISTWNDLINE_CELLS_ARRAY) as *const usize);
    if cell_array == 0 {
        return None;
    }

    let cell_ptr = cell_array + col * eqg::SLISTWNDCELL_SIZE;
    read_cxstr(cell_ptr + eqg::SLISTWNDCELL_TEXT)
}

#[cfg(not(windows))]
pub unsafe fn read_list_item_text(_list_wnd: usize, _row: usize, _col: usize) -> Option<String> {
    None
}

/// Count the number of rows in a CListWnd.
///
/// # Safety
/// `list_wnd` must be a valid CListWnd pointer.
#[cfg(windows)]
pub unsafe fn list_row_count(list_wnd: usize) -> usize {
    let count = *((list_wnd + dmft_common::offsets::eqgame::CLISTWND_ITEMS_COUNT) as *const i32);
    if count < 0 { 0 } else { count as usize }
}

#[cfg(not(windows))]
pub unsafe fn list_row_count(_list_wnd: usize) -> usize {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_cxstr_returns_none_on_non_windows() {
        assert!(unsafe { read_cxstr(0) }.is_none());
    }

    #[test]
    fn write_cxstr_inplace_returns_false_on_non_windows() {
        assert!(!unsafe { write_cxstr_inplace(0, "test") });
    }

    #[test]
    fn clone_cstrrep_returns_none_on_non_windows() {
        assert!(unsafe { clone_cstrrep(0) }.is_none());
    }

    #[test]
    fn alloc_cstrrep_returns_none_on_non_windows() {
        assert!(unsafe { alloc_cstrrep("test") }.is_none());
    }

    #[test]
    fn find_window_by_name_returns_none_on_non_windows() {
        assert!(unsafe { find_window_by_name(0, "test") }.is_none());
    }

    #[test]
    fn find_window_by_text_contains_returns_none_on_non_windows() {
        assert!(unsafe { find_window_by_text_contains(0, "test") }.is_none());
    }

    #[test]
    fn find_child_button_returns_none_on_non_windows() {
        assert!(unsafe { find_child_button_by_text(0, "test") }.is_none());
    }

    #[test]
    fn click_button_via_vtable_noop_on_non_windows() {
        unsafe { click_button_via_vtable(0); } // should not panic
    }

    #[test]
    fn set_edit_text_via_vtable_returns_false_on_non_windows() {
        assert!(!unsafe { set_edit_text_via_vtable(0, "test") });
    }

    #[test]
    fn is_visible_returns_false_on_non_windows() {
        assert!(!unsafe { is_visible(0) });
    }

    #[test]
    fn xml_index_returns_negative_on_non_windows() {
        assert_eq!(unsafe { xml_index(0) }, -1);
    }

    #[test]
    fn find_child_by_sidl_text_returns_none_on_non_windows() {
        assert!(unsafe { find_child_by_sidl_text(0, "Character_List") }.is_none());
    }

    #[test]
    fn read_list_item_text_returns_none_on_non_windows() {
        assert!(unsafe { read_list_item_text(0, 0, 2) }.is_none());
    }

    #[test]
    fn list_row_count_returns_zero_on_non_windows() {
        assert_eq!(unsafe { list_row_count(0) }, 0);
    }

    #[test]
    fn find_visible_window_by_sidl_name_returns_none_on_non_windows() {
        assert!(unsafe { find_visible_window_by_sidl_name(0, "connect", 0x270, 0x010, 0x018) }.is_none());
    }

    #[test]
    fn find_visible_window_by_name_returns_none_on_non_windows() {
        assert!(unsafe { find_visible_window_by_name(0, "test") }.is_none());
    }
}
