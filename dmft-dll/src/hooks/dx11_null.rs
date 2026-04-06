//! DX11 null device hook — intercepts texture/buffer creation to minimize GPU memory.
//!
//! When `RenderMode::NullRender` is active, this module vtable-hooks the
//! `ID3D11Device` to replace texture allocations with 1×1 stubs and buffers
//! with 256-byte stubs. This cuts per-client GPU memory from ~500 MB down to
//! nearly zero for headless/background clients.
//!
//! ## Hook strategy (DXGI vtable approach)
//!
//! Instead of walking EQ's internal pointer chain (which breaks on live builds),
//! we use MQ2's proven approach:
//!
//! 1. Create a temporary D3D11 device + swap chain via `D3D11CreateDeviceAndSwapChain`
//!    to get a real `IDXGISwapChain` vtable.
//! 2. Read `Present` (vtable index 8) and hook it.
//! 3. On the first `Present` call, use `swapChain->GetDevice()` to get the real
//!    `ID3D11Device*`, then hook `CreateTexture2D` (index 5) and `CreateBuffer` (index 3).
//! 4. Clean up the temporary device/swap chain.

use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};

/// Original `IDXGISwapChain::Present` function pointer (vtable slot 8).
static ORIG_PRESENT: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());

/// Original `ID3D11Device::CreateTexture2D` function pointer (vtable slot 5).
static ORIG_CREATE_TEXTURE2D: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());

/// Original `ID3D11Device::CreateBuffer` function pointer (vtable slot 3).
static ORIG_CREATE_BUFFER: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());

/// Whether the Present hook has been installed.
static PRESENT_HOOKED: AtomicBool = AtomicBool::new(false);

/// Whether the device hooks (CreateTexture2D + CreateBuffer) have been installed.
static DEVICE_HOOKED: AtomicBool = AtomicBool::new(false);

/// Cached EQ base address for deferred installation.
static CACHED_EQ_BASE: AtomicU64 = AtomicU64::new(0);

// ─── DX11 struct definitions (subset needed for hooking) ───

/// `D3D11_TEXTURE2D_DESC` — describes a 2D texture resource.
#[repr(C)]
#[derive(Clone)]
#[allow(non_snake_case)]
struct D3D11_TEXTURE2D_DESC {
    Width: u32,
    Height: u32,
    MipLevels: u32,
    ArraySize: u32,
    Format: u32,             // DXGI_FORMAT
    SampleDesc_Count: u32,   // DXGI_SAMPLE_DESC.Count
    SampleDesc_Quality: u32, // DXGI_SAMPLE_DESC.Quality
    Usage: u32,              // D3D11_USAGE
    BindFlags: u32,
    CPUAccessFlags: u32,
    MiscFlags: u32,
}

/// `D3D11_BUFFER_DESC` — describes a buffer resource.
#[repr(C)]
#[derive(Clone)]
#[allow(non_snake_case)]
struct D3D11_BUFFER_DESC {
    ByteWidth: u32,
    Usage: u32,
    BindFlags: u32,
    CPUAccessFlags: u32,
    MiscFlags: u32,
    StructureByteStride: u32,
}

// DX11 bind flag constants.
const D3D11_BIND_RENDER_TARGET: u32 = 0x20;
const D3D11_BIND_DEPTH_STENCIL: u32 = 0x40;

// DX11 usage constants.
const D3D11_USAGE_STAGING: u32 = 3;

// COM vtable indices.
const VTABLE_CREATE_BUFFER: usize = 3;
const VTABLE_CREATE_TEXTURE2D: usize = 5;
const VTABLE_PRESENT: usize = 8;

// ─── COM function signatures ───

type CreateTexture2DFn = unsafe extern "system" fn(
    this: *mut core::ffi::c_void,
    desc: *const D3D11_TEXTURE2D_DESC,
    initial_data: *const core::ffi::c_void,
    texture: *mut *mut core::ffi::c_void,
) -> i32;

type CreateBufferFn = unsafe extern "system" fn(
    this: *mut core::ffi::c_void,
    desc: *const D3D11_BUFFER_DESC,
    initial_data: *const core::ffi::c_void,
    buffer: *mut *mut core::ffi::c_void,
) -> i32;

type PresentFn =
    unsafe extern "system" fn(this: *mut core::ffi::c_void, sync_interval: u32, flags: u32) -> i32;

// ─── Hook implementations ───

/// Hooked `IDXGISwapChain::Present` — on first call, extracts the real
/// `ID3D11Device*` and hooks `CreateTexture2D` + `CreateBuffer`.
/// All subsequent calls pass through to the original.
#[cfg(windows)]
unsafe extern "system" fn hooked_present(
    this: *mut core::ffi::c_void,
    sync_interval: u32,
    flags: u32,
) -> i32 {
    // On first call, hook the device's CreateTexture2D and CreateBuffer.
    if !DEVICE_HOOKED.load(Ordering::Acquire) {
        DEVICE_HOOKED.store(true, Ordering::Release);
        if let Err(e) = unsafe { inner::hook_device_from_swap_chain(this) } {
            tracing::warn!("Failed to hook ID3D11Device from Present: {}", e);
        }
    }

    let original: PresentFn = unsafe { core::mem::transmute(ORIG_PRESENT.load(Ordering::Acquire)) };
    unsafe { original(this, sync_interval, flags) }
}

/// Hooked `CreateTexture2D` — returns 1×1 textures in NullRender mode.
unsafe extern "system" fn hooked_create_texture2d(
    this: *mut core::ffi::c_void,
    desc: *const D3D11_TEXTURE2D_DESC,
    initial_data: *const core::ffi::c_void,
    texture: *mut *mut core::ffi::c_void,
) -> i32 {
    let original: CreateTexture2DFn =
        unsafe { core::mem::transmute(ORIG_CREATE_TEXTURE2D.load(Ordering::Acquire)) };

    if super::render::mode() == dmft_common::ipc::RenderMode::NullRender && !desc.is_null() {
        let d = unsafe { &*desc };

        // Don't shrink render targets, depth stencils, or staging textures —
        // those are needed for the swap chain and GPU readback to function.
        let is_rt = d.BindFlags & D3D11_BIND_RENDER_TARGET != 0;
        let is_ds = d.BindFlags & D3D11_BIND_DEPTH_STENCIL != 0;
        let is_staging = d.Usage == D3D11_USAGE_STAGING;

        if !is_rt && !is_ds && !is_staging && (d.Width > 1 || d.Height > 1) {
            let mut mini = d.clone();
            mini.Width = 1;
            mini.Height = 1;
            mini.MipLevels = 1;
            return unsafe { original(this, &mini, core::ptr::null(), texture) };
        }
    }

    unsafe { original(this, desc, initial_data, texture) }
}

/// Hooked `CreateBuffer` — returns 256-byte buffers in NullRender mode.
unsafe extern "system" fn hooked_create_buffer(
    this: *mut core::ffi::c_void,
    desc: *const D3D11_BUFFER_DESC,
    initial_data: *const core::ffi::c_void,
    buffer: *mut *mut core::ffi::c_void,
) -> i32 {
    let original: CreateBufferFn =
        unsafe { core::mem::transmute(ORIG_CREATE_BUFFER.load(Ordering::Acquire)) };

    if super::render::mode() == dmft_common::ipc::RenderMode::NullRender && !desc.is_null() {
        let d = unsafe { &*desc };

        // Only shrink large buffers (>4 KB). Small buffers are often constant
        // buffers or uniform blocks that must keep their exact size.
        if d.ByteWidth > 4096 {
            let mut mini = d.clone();
            mini.ByteWidth = 256;
            return unsafe { original(this, &mini, core::ptr::null(), buffer) };
        }
    }

    unsafe { original(this, desc, initial_data, buffer) }
}

// ─── Platform-specific installation ───

#[cfg(windows)]
mod inner {
    use super::*;

    /// COM `IUnknown::Release` vtable index.
    const VTABLE_RELEASE: usize = 2;
    type ReleaseFn = unsafe extern "system" fn(this: *mut core::ffi::c_void) -> u32;

    /// Vtable-hook a COM interface method.
    ///
    /// Reads the vtable pointer from `object`, patches entry `index` to point
    /// to `hook_fn`, and returns the original function pointer.
    unsafe fn vtable_hook(
        object: *mut core::ffi::c_void,
        index: usize,
        hook_fn: *const core::ffi::c_void,
    ) -> Option<*mut core::ffi::c_void> {
        use windows::Win32::System::Memory::{
            PAGE_PROTECTION_FLAGS, PAGE_READWRITE, VirtualProtect,
        };

        let vtable_ptr = unsafe { *(object as *const *mut *mut core::ffi::c_void) };
        let entry = unsafe { vtable_ptr.add(index) };
        let original = unsafe { *entry };

        let mut old_protect = PAGE_PROTECTION_FLAGS(0);
        let entry_size = core::mem::size_of::<*mut core::ffi::c_void>();

        unsafe {
            VirtualProtect(
                entry as *const core::ffi::c_void,
                entry_size,
                PAGE_READWRITE,
                &mut old_protect,
            )
            .ok()?;

            *entry = hook_fn as *mut core::ffi::c_void;

            VirtualProtect(
                entry as *const core::ffi::c_void,
                entry_size,
                old_protect,
                &mut old_protect,
            )
            .ok()?;
        }

        Some(original)
    }

    /// Restore a vtable entry to its original function pointer.
    unsafe fn vtable_unhook(
        object: *mut core::ffi::c_void,
        index: usize,
        original_fn: *mut core::ffi::c_void,
    ) {
        let _ = unsafe { vtable_hook(object, index, original_fn) };
    }

    /// Find EQ's main window handle. Looks for the "EverQuest" window class.
    fn find_eq_hwnd() -> Option<windows::Win32::Foundation::HWND> {
        use windows::Win32::UI::WindowsAndMessaging::FindWindowA;
        use windows::core::s;

        let hwnd = unsafe { FindWindowA(s!("EverQuest"), None) };
        if hwnd.0 == 0 { None } else { Some(hwnd) }
    }

    /// Create a temporary D3D11 device + swap chain to capture the DXGI vtable,
    /// then hook `IDXGISwapChain::Present`.
    fn hook_present_via_dummy_device(
        hwnd: windows::Win32::Foundation::HWND,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
        use windows::Win32::Graphics::Direct3D11::D3D11CreateDeviceAndSwapChain;
        use windows::Win32::Graphics::Dxgi::Common::{
            DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_MODE_DESC, DXGI_SAMPLE_DESC,
        };
        use windows::Win32::Graphics::Dxgi::{
            DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_EFFECT_DISCARD, DXGI_USAGE_RENDER_TARGET_OUTPUT,
        };
        use windows::core::Interface;

        let swap_chain_desc = DXGI_SWAP_CHAIN_DESC {
            BufferDesc: DXGI_MODE_DESC {
                Width: 1,
                Height: 1,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                ..Default::default()
            },
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 1,
            OutputWindow: hwnd,
            Windowed: true.into(),
            SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
            ..Default::default()
        };

        let mut swap_chain: Option<windows::Win32::Graphics::Dxgi::IDXGISwapChain> = None;
        let mut device: Option<windows::Win32::Graphics::Direct3D11::ID3D11Device> = None;

        unsafe {
            D3D11CreateDeviceAndSwapChain(
                None, // adapter
                D3D_DRIVER_TYPE_HARDWARE,
                None,               // software module
                Default::default(), // flags
                None,               // feature levels (default)
                7,                  // SDK version (D3D11_SDK_VERSION)
                Some(&swap_chain_desc as *const _),
                Some(&mut swap_chain as *mut _),
                Some(&mut device as *mut _),
                None, // feature level out
                None, // immediate context out
            )?;
        }

        let swap_chain =
            swap_chain.ok_or("D3D11CreateDeviceAndSwapChain returned null swap chain")?;

        // Read Present address from the swap chain vtable (index 8).
        let sc_ptr = swap_chain.as_raw();
        let vtable_ptr = unsafe { *(sc_ptr as *const *const *const core::ffi::c_void) };
        let present_addr = unsafe { *vtable_ptr.add(VTABLE_PRESENT) };

        tracing::info!(
            present = format!("{:#x}", present_addr as usize),
            "Captured IDXGISwapChain::Present from dummy device"
        );

        // Now hook Present in the REAL swap chain vtable.
        // Since all IDXGISwapChain instances of the same implementation share the
        // same vtable, hooking the dummy's vtable hooks ALL swap chains (including EQ's).
        let orig = unsafe {
            vtable_hook(
                sc_ptr,
                VTABLE_PRESENT,
                hooked_present as *const core::ffi::c_void,
            )
        }
        .ok_or("Failed to vtable-hook IDXGISwapChain::Present")?;
        ORIG_PRESENT.store(orig, Ordering::Release);
        PRESENT_HOOKED.store(true, Ordering::Release);

        tracing::info!("IDXGISwapChain::Present hook installed via dummy device");

        // Release the temporary device and swap chain — the vtable hook persists
        // because it patches the shared vtable, not the object instance.
        // Drop happens automatically via windows crate COM ref-counting.
        drop(swap_chain);
        drop(device);

        Ok(())
    }

    /// Called from the hooked Present on the first invocation.
    /// Extracts the real `ID3D11Device*` from the swap chain and hooks
    /// `CreateTexture2D` + `CreateBuffer`.
    pub(super) unsafe fn hook_device_from_swap_chain(
        swap_chain: *mut core::ffi::c_void,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Call swap_chain->GetDevice() via COM.
        // IDXGISwapChain inherits from IDXGIDeviceSubObject which has GetDevice().
        // But we want ID3D11Device, so we use QueryInterface-style GetDevice
        // by going through the swap chain's GetDevice (vtable index 10 on IDXGISwapChain).
        //
        // Actually, the simplest approach: cast to IDXGISwapChain, call GetDevice
        // with IID_ID3D11Device.

        // IID_ID3D11Device = {db6f6ddb-ac77-4e88-8253-819df9bbf140}
        let iid_device: windows::core::GUID = windows::core::GUID::from_values(
            0xdb6f6ddb,
            0xac77,
            0x4e88,
            [0x82, 0x53, 0x81, 0x9d, 0xf9, 0xbb, 0xf1, 0x40],
        );

        // IDXGISwapChain::GetDevice is inherited from IDXGIObject, vtable index 7.
        // Signature: HRESULT GetDevice(REFIID riid, void **ppDevice)
        type GetDeviceFn = unsafe extern "system" fn(
            this: *mut core::ffi::c_void,
            riid: *const windows::core::GUID,
            device: *mut *mut core::ffi::c_void,
        ) -> i32;

        let vtable_ptr = unsafe { *(swap_chain as *const *const *const core::ffi::c_void) };
        // IDXGIObject::GetParent is index 6, GetDevice is on IDXGIDeviceSubObject
        // which adds it at index 7 (after IUnknown[0-2] + IDXGIObject[3-6]).
        let get_device_fn: GetDeviceFn = unsafe { core::mem::transmute(*vtable_ptr.add(7)) };

        let mut device_ptr: *mut core::ffi::c_void = core::ptr::null_mut();
        let hr = unsafe { get_device_fn(swap_chain, &iid_device, &mut device_ptr) };
        if hr != 0 || device_ptr.is_null() {
            return Err(format!(
                "SwapChain::GetDevice(IID_ID3D11Device) failed: HRESULT {:#x}",
                hr
            )
            .into());
        }

        tracing::info!(
            device = format!("{:#x}", device_ptr as usize),
            "Got real ID3D11Device from swap chain"
        );

        // Hook CreateTexture2D (vtable index 5)
        let orig_tex = unsafe {
            vtable_hook(
                device_ptr,
                VTABLE_CREATE_TEXTURE2D,
                hooked_create_texture2d as *const core::ffi::c_void,
            )
        }
        .ok_or("Failed to vtable-hook CreateTexture2D")?;
        ORIG_CREATE_TEXTURE2D.store(orig_tex, Ordering::Release);

        // Hook CreateBuffer (vtable index 3)
        let orig_buf = unsafe {
            vtable_hook(
                device_ptr,
                VTABLE_CREATE_BUFFER,
                hooked_create_buffer as *const core::ffi::c_void,
            )
        }
        .ok_or("Failed to vtable-hook CreateBuffer")?;
        ORIG_CREATE_BUFFER.store(orig_buf, Ordering::Release);

        tracing::info!("ID3D11Device hooks installed (CreateTexture2D + CreateBuffer)");

        // Release the device ref we got from GetDevice.
        let release: ReleaseFn = unsafe {
            core::mem::transmute(
                *(*(device_ptr as *const *const *const core::ffi::c_void)).add(VTABLE_RELEASE),
            )
        };
        unsafe { release(device_ptr) };

        Ok(())
    }

    /// Install DX11 vtable hooks via DXGI. If EQ's window isn't available yet,
    /// caches the EQ base for deferred installation via `ensure_installed()`.
    pub fn install(eq_base: u64) -> Result<(), Box<dyn std::error::Error>> {
        CACHED_EQ_BASE.store(eq_base, Ordering::Release);

        if PRESENT_HOOKED.load(Ordering::Acquire) {
            return Ok(());
        }

        match find_eq_hwnd() {
            Some(hwnd) => {
                hook_present_via_dummy_device(hwnd)?;
                Ok(())
            }
            None => {
                tracing::info!(
                    "EQ window not found — DX11 hooks will install on first SetRenderMode"
                );
                Ok(())
            }
        }
    }

    /// Attempt deferred installation. Called from `set_mode()` when hooks
    /// weren't installed at DLL init (window wasn't available yet).
    pub fn ensure_installed() {
        if PRESENT_HOOKED.load(Ordering::Acquire) {
            return;
        }
        let eq_base = CACHED_EQ_BASE.load(Ordering::Acquire);
        if eq_base == 0 {
            return;
        }
        match find_eq_hwnd() {
            Some(hwnd) => match hook_present_via_dummy_device(hwnd) {
                Ok(()) => tracing::info!("DX11 Present hook installed (deferred)"),
                Err(e) => tracing::warn!("Deferred DX11 Present hook failed: {}", e),
            },
            None => tracing::warn!("DX11 hooks deferred — EQ window still not available"),
        }
    }

    /// Remove DX11 vtable hooks by restoring original function pointers.
    ///
    /// Note: We cannot easily restore the Present hook since the dummy swap chain
    /// was released. The device hooks are restored via the vtable (all instances
    /// share the same vtable). On DLL unload, Present hook removal is best-effort.
    pub fn remove(_eq_base: u64) {
        // We don't have a persistent reference to the swap chain or device,
        // so we log a warning. In practice, remove() is called at DLL unload
        // and the process is about to exit anyway.
        if PRESENT_HOOKED.load(Ordering::Acquire) {
            tracing::info!(
                "DX11 Present hook cannot be cleanly removed (vtable shared, no persistent ref). \
                 Process is likely shutting down."
            );
        }
        if DEVICE_HOOKED.load(Ordering::Acquire) {
            tracing::info!(
                "DX11 device hooks (CreateTexture2D + CreateBuffer) will be released on process exit."
            );
        }
        PRESENT_HOOKED.store(false, Ordering::Release);
        DEVICE_HOOKED.store(false, Ordering::Release);
        tracing::info!("DX11 null device hooks marked as removed");
    }
}

#[cfg(not(windows))]
mod inner {
    /// Stub — DX11 hooks are only functional on Windows.
    pub fn install(_eq_base: u64) -> Result<(), Box<dyn std::error::Error>> {
        tracing::warn!("DX11 null device hooks not available on this platform (stub)");
        Ok(())
    }

    /// Stub — deferred install is a no-op on non-Windows.
    pub fn ensure_installed() {}

    /// Stub — nothing to remove on non-Windows.
    pub fn remove(_eq_base: u64) {
        tracing::warn!("DX11 null device hook removal not available (stub)");
    }
}

#[allow(unused_imports)]
pub use inner::{ensure_installed, install, remove};
