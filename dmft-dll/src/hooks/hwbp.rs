//! Hardware breakpoint hooking engine — uses x86_64 debug registers (DR0-DR3).
//!
//! Placeholder for #345 HWBP hooking implementation.

/// Hardware breakpoint slot (maps to DR0-DR3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HwbpSlot {
    Dr0,
    Dr1,
    Dr2,
    Dr3,
}

type HwbpCallback = fn(*mut ()) -> bool;

pub fn register(
    _slot: HwbpSlot,
    _address: usize,
    _callback: HwbpCallback,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("HWBP registration placeholder (slot={:?})", _slot);
    Ok(())
}

pub fn unregister(_slot: HwbpSlot) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

pub fn is_active(_slot: HwbpSlot) -> bool {
    false
}

pub fn remove_all() {
    // Placeholder — full implementation in #345.
}
