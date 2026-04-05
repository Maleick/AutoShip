//! DX11 null device hook — intercepts texture/buffer creation to minimize GPU memory.
//!
//! When `RenderMode::NullRender` is active, this module vtable-hooks the
//! `ID3D11Device` to replace texture allocations with 1×1 stubs. This cuts
//! per-client GPU memory from ~500 MB of textures down to nearly zero for
//! headless/background clients.
//!
//! ## Pointer chain (from MQ2 eqlib headers)
//!
//! ```text
//! pinstSGraphicsEngine → SGraphicsEngine+0x18 → CRender*
//!   CRender+0x0F00 → DeviceImpl* (DX9 wrapper alias)
//!     DeviceImpl+0x28 → Device*
//!       Device+0x18 → SwapChain (inline struct, 0x1448 bytes)
//!         SwapChain+0x00 → ID3D11Device*
//!         SwapChain+0x08 → ID3D11DeviceContext*
//! ```
//!
//! EQ uses a DX9→DX11 wrapper layer (`DX9Wrapper::DX11` namespace in eqlib).
//! The actual rendering goes through `EQGraphics.DLL` which holds the real
//! DX11 device and context.

use std::sync::atomic::{AtomicPtr, Ordering};

/// Original `ID3D11Device::CreateTexture2D` function pointer (vtable slot 5).
static ORIG_CREATE_TEXTURE2D: AtomicPtr<core::ffi::c_void> =
    AtomicPtr::new(core::ptr::null_mut());

/// Original `ID3D11Device::CreateBuffer` function pointer (vtable slot 3).
static ORIG_CREATE_BUFFER: AtomicPtr<core::ffi::c_void> =
    AtomicPtr::new(core::ptr::null_mut());

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

// ID3D11Device vtable indices (IUnknown has 3: QI, AddRef, Release).
const VTABLE_CREATE_BUFFER: usize = 3;
const VTABLE_CREATE_TEXTURE2D: usize = 5;

// HRESULT success.
#[allow(dead_code)]
const S_OK: i32 = 0;

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

// ─── Hook implementations ───

/// Hooked `CreateTexture2D` — returns 1×1 textures in NullRender mode.
unsafe extern "system" fn hooked_create_texture2d(
    this: *mut core::ffi::c_void,
    desc: *const D3D11_TEXTURE2D_DESC,
    initial_data: *const core::ffi::c_void,
    texture: *mut *mut core::ffi::c_void,
) -> i32 {
    // SAFETY: ORIG_CREATE_TEXTURE2D was stored during install() from a valid vtable entry.
    // transmute converts the stored void pointer back to the original COM method signature.
    let original: CreateTexture2DFn =
        unsafe { core::mem::transmute(ORIG_CREATE_TEXTURE2D.load(Ordering::Acquire)) };

    if super::render::mode() == dmft_common::ipc::RenderMode::NullRender && !desc.is_null() {
        // SAFETY: desc is a valid pointer passed by the DX11 runtime / EQ engine.
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
            // SAFETY: Calling the original COM method with a modified desc.
            // Pass null initial data — the 1×1 texture gets zero-initialized.
            return unsafe { original(this, &mini, core::ptr::null(), texture) };
        }
    }

    // SAFETY: Forwarding all arguments unchanged to the original COM method.
    unsafe { original(this, desc, initial_data, texture) }
}

/// Hooked `CreateBuffer` — returns 256-byte buffers in NullRender mode.
unsafe extern "system" fn hooked_create_buffer(
    this: *mut core::ffi::c_void,
    desc: *const D3D11_BUFFER_DESC,
    initial_data: *const core::ffi::c_void,
    buffer: *mut *mut core::ffi::c_void,
) -> i32 {
    // SAFETY: ORIG_CREATE_BUFFER was stored during install() from a valid vtable entry.
    let original: CreateBufferFn =
        unsafe { core::mem::transmute(ORIG_CREATE_BUFFER.load(Ordering::Acquire)) };

    if super::render::mode() == dmft_common::ipc::RenderMode::NullRender && !desc.is_null() {
        // SAFETY: desc is a valid pointer passed by the DX11 runtime / EQ engine.
        let d = unsafe { &*desc };

        // Only shrink large buffers (>4 KB). Small buffers are often constant
        // buffers or uniform blocks that must keep their exact size.
        if d.ByteWidth > 4096 {
            let mut mini = d.clone();
            mini.ByteWidth = 256;
            // SAFETY: Calling the original COM method with a modified desc.
            return unsafe { original(this, &mini, core::ptr::null(), buffer) };
        }
    }

    // SAFETY: Forwarding all arguments unchanged to the original COM method.
    unsafe { original(this, desc, initial_data, buffer) }
}

// ─── Platform-specific installation ───

#[cfg(windows)]
mod inner {
    use super::*;

    /// Resolve the `ID3D11Device*` from EQ's graphics engine pointer chain.
    ///
    /// Returns `None` if any pointer in the chain is null.
    unsafe fn resolve_d3d11_device(eq_base: u64) -> Option<*mut core::ffi::c_void> {
        let gfx_engine_ptr = dmft_common::offsets::rebase(
            dmft_common::offsets::PINST_SGRAPHICSENGINE,
            eq_base,
        )?;

        // SAFETY: All pointer dereferences in this chain follow the MQ2 eqlib
        // struct layout. Each step is null-checked before advancing.

        // pinstSGraphicsEngine → SGraphicsEngine*
        let gfx_engine: *const u64 = gfx_engine_ptr as *const u64;
        let sgraphics = unsafe { *gfx_engine } as *const u8;
        if sgraphics.is_null() {
            tracing::warn!("SGraphicsEngine pointer is null");
            return None;
        }

        // SGraphicsEngine+0x18 → CRender*
        let crender = unsafe { *(sgraphics.add(0x18) as *const *const u8) };
        if crender.is_null() {
            tracing::warn!("CRender pointer is null");
            return None;
        }

        // CRender+0x0F00 → DeviceImpl* (DX9 wrapper)
        let device_impl = unsafe { *(crender.add(0x0F00) as *const *const u8) };
        if device_impl.is_null() {
            tracing::warn!("DeviceImpl pointer is null");
            return None;
        }

        // DeviceImpl+0x28 → Device*
        let device = unsafe { *(device_impl.add(0x28) as *const *const u8) };
        if device.is_null() {
            tracing::warn!("Device pointer is null");
            return None;
        }

        // Device+0x18 → SwapChain (inline). SwapChain+0x00 → ID3D11Device*
        let d3d11_device = unsafe { *(device.add(0x18) as *const *mut core::ffi::c_void) };
        if d3d11_device.is_null() {
            tracing::warn!("ID3D11Device pointer is null");
            return None;
        }

        tracing::info!(
            device = format!("{:#x}", d3d11_device as usize),
            "Resolved ID3D11Device"
        );
        Some(d3d11_device)
    }

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
            VirtualProtect, PAGE_PROTECTION_FLAGS, PAGE_READWRITE,
        };

        // SAFETY: COM object layout guarantees first field is vtable pointer.
        // VirtualProtect is needed because vtable memory is read-only.
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

    /// Install DX11 vtable hooks on the device's `CreateTexture2D` and `CreateBuffer`.
    pub fn install(eq_base: u64) -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            let device = resolve_d3d11_device(eq_base)
                .ok_or("Failed to resolve ID3D11Device pointer chain")?;

            // Hook CreateTexture2D (vtable index 5)
            let orig_tex = vtable_hook(
                device,
                VTABLE_CREATE_TEXTURE2D,
                hooked_create_texture2d as *const core::ffi::c_void,
            )
            .ok_or("Failed to vtable-hook CreateTexture2D")?;
            ORIG_CREATE_TEXTURE2D.store(orig_tex, Ordering::Release);

            // Hook CreateBuffer (vtable index 3)
            let orig_buf = vtable_hook(
                device,
                VTABLE_CREATE_BUFFER,
                hooked_create_buffer as *const core::ffi::c_void,
            )
            .ok_or("Failed to vtable-hook CreateBuffer")?;
            ORIG_CREATE_BUFFER.store(orig_buf, Ordering::Release);

            tracing::info!("DX11 null device hooks installed (CreateTexture2D + CreateBuffer)");
        }
        Ok(())
    }

    /// Remove DX11 vtable hooks by restoring original function pointers.
    pub fn remove(eq_base: u64) {
        unsafe {
            let device = match resolve_d3d11_device(eq_base) {
                Some(d) => d,
                None => {
                    tracing::warn!("Cannot remove DX11 hooks — device not resolved");
                    return;
                }
            };

            let orig_tex = ORIG_CREATE_TEXTURE2D.load(Ordering::Acquire);
            let orig_buf = ORIG_CREATE_BUFFER.load(Ordering::Acquire);

            if !orig_tex.is_null() {
                let _ = vtable_hook(device, VTABLE_CREATE_TEXTURE2D, orig_tex);
            }
            if !orig_buf.is_null() {
                let _ = vtable_hook(device, VTABLE_CREATE_BUFFER, orig_buf);
            }
        }
        tracing::info!("DX11 null device hooks removed");
    }
}

#[cfg(not(windows))]
mod inner {
    /// Stub — DX11 hooks are only functional on Windows.
    pub fn install(_eq_base: u64) -> Result<(), Box<dyn std::error::Error>> {
        tracing::warn!("DX11 null device hooks not available on this platform (stub)");
        Ok(())
    }

    /// Stub — nothing to remove on non-Windows.
    pub fn remove(_eq_base: u64) {
        tracing::warn!("DX11 null device hook removal not available (stub)");
    }
}

#[allow(unused_imports)]
pub use inner::{install, remove};
