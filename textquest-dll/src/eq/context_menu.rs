//! `CContextMenuManager` — read active popup menus and dispatch item activation.
//!
//! # What this module provides
//!
//! - **`read_context_menus(eq_base)`** — walk `CContextMenuManager` and collect every
//!   registered `CContextMenu` into a `Vec<ContextMenuInfo>`.  Each menu's rows are
//!   read through the existing `CListWnd` layout (`eqgame::CLISTWND_ITEMS_ARRAY`).
//!
//! - **`activate_context_menu_item(eq_base, menu_index, item_index)`** — call
//!   `CContextMenuManager::HandleMenu(menu_index, item_index, CXPoint{0,0})` from the
//!   game-loop thread to programmatically select a context-menu entry.
//!
//! # Thread safety
//!
//! Both functions **must** be called from EQ's main game-loop thread.
//! The IPC handler queues these as PENDING_COMMANDS; the game loop drains them on each
//! tick.
//!
//! # Address calibration
//!
//! `PINST_CONTEXT_MENU_MANAGER` and `CONTEXT_MENU_MGR_HANDLE_MENU` in
//! `textquest_common::offsets` must match the running eqgame.exe binary.
//! Use Ghidra or binary search against the string `"GFContextMenu"` to locate the
//! manager singleton if the addresses need updating for a new patch.

#[cfg(windows)]
use std::mem::size_of;
use textquest_common::ipc::ContextMenuInfo;
#[cfg(windows)]
use textquest_common::ipc::ContextMenuItem;

// ─── Public API ───────────────────────────────────────────────────────────────

/// Walk `CContextMenuManager` and return a snapshot of every registered menu.
///
/// # Arguments
/// * `eq_base` — runtime base address of eqgame.exe.
///
/// Returns an empty `Vec` when:
/// - `eq_base` is zero (EQ not yet started).
/// - The `CContextMenuManager` instance pointer is null.
/// - No menus are currently registered.
pub fn read_context_menus(eq_base: u64) -> Vec<ContextMenuInfo> {
    #[cfg(windows)]
    {
        unsafe { read_context_menus_windows(eq_base) }
    }

    #[cfg(not(windows))]
    {
        let _ = eq_base;
        Vec::new()
    }
}

/// Activate a menu item by calling `CContextMenuManager::HandleMenu`.
///
/// # Arguments
/// * `eq_base`     — runtime base address of eqgame.exe.
/// * `menu_index`  — zero-based menu index within `CContextMenuManager`.
/// * `item_index`  — zero-based item (row) index within that menu.
///
/// Returns `Ok(())` on success, `Err(String)` describing the failure reason.
pub fn activate_context_menu_item(
    eq_base: u64,
    menu_index: u32,
    item_index: u32,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        unsafe { activate_context_menu_item_windows(eq_base, menu_index, item_index) }
    }

    #[cfg(not(windows))]
    {
        let _ = (eq_base, menu_index, item_index);
        Err("Context menu activation is only available on Windows builds".into())
    }
}

// ─── Windows implementation ───────────────────────────────────────────────────

/// Maximum number of menus to process — mirrors the safety cap applied in
/// both `read_context_menus_windows` and `activate_context_menu_item_windows`.
#[cfg(windows)]
const MAX_CONTEXT_MENUS: i32 = 64;

/// Maximum number of items per menu to process — mirrors the safety cap applied
/// in both `read_list_wnd_items` and `activate_context_menu_item_windows`.
#[cfg(windows)]
const MAX_MENU_ITEMS: i32 = 256;

#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn read_context_menus_windows(eq_base: u64) -> Vec<ContextMenuInfo> {
    use textquest_common::offsets::{PINST_CONTEXT_MENU_MANAGER, context_menu_mgr};

    if eq_base == 0 {
        return Vec::new();
    }

    // ── 1. Resolve the CContextMenuManager singleton pointer ──────────────────
    let mgr_ptr_addr = match textquest_common::offsets::rebase(PINST_CONTEXT_MENU_MANAGER, eq_base)
    {
        Some(a) => a,
        None => {
            tracing::warn!("CContextMenuManager: rebase failed for PINST_CONTEXT_MENU_MANAGER");
            return Vec::new();
        }
    };
    let mgr_ptr = *(mgr_ptr_addr as *const usize);
    if mgr_ptr == 0 {
        // Manager not yet initialized (e.g. no menus have been opened yet).
        return Vec::new();
    }

    // ── 2. Read the menu count and array pointer from CContextMenuManager ─────
    let menus_count = *((mgr_ptr + context_menu_mgr::MENUS_COUNT) as *const i32);
    let menus_data = *((mgr_ptr + context_menu_mgr::MENUS_DATA) as *const usize);

    if menus_count <= 0 || menus_data == 0 {
        return Vec::new();
    }

    let num_menus = menus_count.min(MAX_CONTEXT_MENUS) as usize; // safety cap

    let mut result = Vec::with_capacity(num_menus);

    for menu_idx in 0..num_menus {
        // Each entry is a CContextMenu* (pointer-sized)
        let menu_ptr = *((menus_data + menu_idx * size_of::<usize>()) as *const usize);
        if menu_ptr == 0 {
            continue;
        }

        let items = read_list_wnd_items(menu_ptr);
        result.push(ContextMenuInfo {
            menu_index: menu_idx as u32,
            items,
        });
    }

    result
}

/// Read item rows from a `CListWnd` (which `CContextMenu` inherits from).
///
/// Each row's first cell holds the item label.  An empty label with zero height
/// indicates a separator line.
#[cfg(windows)]
unsafe fn read_list_wnd_items(list_wnd_ptr: usize) -> Vec<ContextMenuItem> {
    use textquest_common::offsets::eqgame;

    // ── ItemsArray (ArrayClass<SListWndLine>) ──────────────────────────────
    let (items_count, items_array) = unsafe {
        (
            *((list_wnd_ptr + eqgame::CLISTWND_ITEMS_COUNT) as *const i32),
            *((list_wnd_ptr + eqgame::CLISTWND_ITEMS_ARRAY) as *const usize),
        )
    };

    if items_count <= 0 || items_array == 0 {
        return Vec::new();
    }

    let count = items_count.min(MAX_MENU_ITEMS) as usize; // safety cap
    let mut items = Vec::with_capacity(count);

    for row_idx in 0..count {
        let line_ptr = items_array + row_idx * eqgame::SLISTWNDLINE_SIZE;

        // ── Read the first cell's text (CXStr) ────────────────────────────
        let (cells_count, cells_array) = unsafe {
            (
                *((line_ptr + eqgame::SLISTWNDLINE_CELLS_COUNT) as *const i32),
                *((line_ptr + eqgame::SLISTWNDLINE_CELLS_ARRAY) as *const usize),
            )
        };

        let label = if cells_count > 0 && cells_array != 0 {
            // First cell text at cells_array[0] + SLISTWNDCELL_TEXT (CXStr pointer)
            let cell0_ptr = cells_array; // cells_array points directly to cell[0]
            let cxstr_ptr = unsafe { *((cell0_ptr + eqgame::SLISTWNDCELL_TEXT) as *const usize) };
            if cxstr_ptr != 0 {
                unsafe { crate::eq::widgets::read_cxstr(cxstr_ptr) }.unwrap_or_default()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // A separator row has an empty label and enabled=false.
        let is_separator = label.is_empty();

        items.push(ContextMenuItem {
            item_index: row_idx as u32,
            label,
            enabled: !is_separator, // CListWnd doesn't expose per-row disabled state via this path
            checked: false,
            is_separator,
        });
    }

    items
}

#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn activate_context_menu_item_windows(
    eq_base: u64,
    menu_index: u32,
    item_index: u32,
) -> Result<(), String> {
    use textquest_common::offsets::{
        CONTEXT_MENU_MGR_HANDLE_MENU, PINST_CONTEXT_MENU_MANAGER, context_menu_mgr, eqgame,
    };

    if eq_base == 0 {
        return Err("EQ base address not set".into());
    }

    // ── Resolve the CContextMenuManager singleton ─────────────────────────────
    let mgr_ptr_addr = textquest_common::offsets::rebase(PINST_CONTEXT_MENU_MANAGER, eq_base)
        .ok_or("rebase failed for PINST_CONTEXT_MENU_MANAGER")?;
    let mgr_ptr = *(mgr_ptr_addr as *const usize);
    if mgr_ptr == 0 {
        return Err("CContextMenuManager instance pointer is null".into());
    }

    // ── Bounds-check: verify menu_index is within range ───────────────────────
    let menus_count = *((mgr_ptr + context_menu_mgr::MENUS_COUNT) as *const i32);
    let menus_data = *((mgr_ptr + context_menu_mgr::MENUS_DATA) as *const usize);
    if menus_count <= 0 {
        return Err(format!(
            "menu_index {menu_index} out of range (manager has {menus_count} menus)"
        ));
    }
    let effective_menus = menus_count.min(MAX_CONTEXT_MENUS) as u32;
    if menu_index >= effective_menus {
        return Err(format!(
            "menu_index {menu_index} out of range (manager has {menus_count} menus, effective cap {effective_menus})"
        ));
    }
    if menus_data == 0 {
        return Err("context menu manager menus data pointer is null".into());
    }

    // ── Bounds-check: verify item_index is within selected menu range ─────────
    let menu_ptr = *((menus_data + menu_index as usize * size_of::<usize>()) as *const usize);
    if menu_ptr == 0 {
        return Err(format!("menu pointer is null for menu_index {menu_index}"));
    }
    let items_count = *((menu_ptr + eqgame::CLISTWND_ITEMS_COUNT) as *const i32);
    if items_count <= 0 {
        return Err(format!(
            "item_index {item_index} out of range for menu_index {menu_index} (menu has {items_count} items)"
        ));
    }
    let effective_items = items_count.min(MAX_MENU_ITEMS) as u32;
    if item_index >= effective_items {
        return Err(format!(
            "item_index {item_index} out of range for menu_index {menu_index} (menu has {items_count} items, effective cap {effective_items})"
        ));
    }

    // ── Resolve and validate HandleMenu function pointer ─────────────────────
    let handle_menu_addr = textquest_common::offsets::rebase(CONTEXT_MENU_MGR_HANDLE_MENU, eq_base)
        .ok_or("rebase failed for CONTEXT_MENU_MGR_HANDLE_MENU")?;

    if !crate::eq::validate_fn_ptr(handle_menu_addr, "CContextMenuManager::HandleMenu") {
        return Err("HandleMenu function pointer failed validation".into());
    }

    // ── Call HandleMenu(this, menu_index, item_index, x=0, y=0) ──────────────
    // Signature (x64 MSVC):
    //   void HandleMenu(int menuId, int itemId, const CXPoint& pt)
    //   this → RCX, menuId → RDX (i32), itemId → R8 (i32), &pt → R9
    //
    // CXPoint is two ints {x, y}.  We pass a local zero-initialised CXPoint on
    // the stack; EQ ignores the coordinates for programmatic activation.
    #[repr(C)]
    struct CXPoint {
        x: i32,
        y: i32,
    }
    let pt = CXPoint { x: 0, y: 0 };

    type HandleMenuFn =
        unsafe extern "C" fn(this: usize, menu_id: i32, item_id: i32, pt: *const CXPoint);
    let handle_menu: HandleMenuFn = core::mem::transmute(handle_menu_addr);

    handle_menu(
        mgr_ptr,
        menu_index as i32,
        item_index as i32,
        &pt as *const CXPoint,
    );

    tracing::info!(
        menu_index,
        item_index,
        "CContextMenuManager::HandleMenu dispatched"
    );
    Ok(())
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::ipc::ContextMenuItem;

    #[test]
    fn read_context_menus_no_crash_when_eq_base_zero() {
        let menus = read_context_menus(0);
        assert!(menus.is_empty());
    }

    #[test]
    fn activate_returns_err_when_eq_base_zero() {
        let result = activate_context_menu_item(0, 0, 0);
        assert!(result.is_err());
    }

    #[test]
    fn context_menu_info_default_is_empty() {
        let info = ContextMenuInfo {
            menu_index: 0,
            items: Vec::new(),
        };
        assert!(info.items.is_empty());
    }

    #[test]
    fn context_menu_item_fields() {
        let item = ContextMenuItem {
            item_index: 2,
            label: "Attack".into(),
            enabled: true,
            checked: false,
            is_separator: false,
        };
        assert_eq!(item.item_index, 2);
        assert_eq!(item.label, "Attack");
        assert!(item.enabled);
        assert!(!item.checked);
        assert!(!item.is_separator);
    }

    #[test]
    fn separator_item_has_empty_label() {
        let sep = ContextMenuItem {
            item_index: 1,
            label: String::new(),
            enabled: false,
            checked: false,
            is_separator: true,
        };
        assert!(sep.is_separator);
        assert!(sep.label.is_empty());
    }
}
