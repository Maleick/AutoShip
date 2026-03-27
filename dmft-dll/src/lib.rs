// dmft-dll: injected DLL for in-process EverQuest memory reading
//
// Minimal DllMain placeholder — real implementation will follow.

#[cfg(windows)]
#[unsafe(no_mangle)]
pub extern "system" fn DllMain(
    _h_module: *mut core::ffi::c_void,
    _ul_reason_for_call: u32,
    _lp_reserved: *mut core::ffi::c_void,
) -> i32 {
    1 // TRUE
}
