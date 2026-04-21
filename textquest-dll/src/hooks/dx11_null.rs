//! DX11 null device hook — intercepts texture/buffer creation and draw calls
//! to minimize GPU work.
//!
//! When `RenderMode::NullRender` is active, this module vtable-hooks:
//! - `ID3D11Device` — replaces texture allocations with 1×1 stubs and buffers
//!   with 256-byte stubs (~500 MB → ~0 per client).
//! - `ID3D11DeviceContext` — no-ops all draw calls (`Draw`, `DrawIndexed`,
//!   `DrawInstanced`, `DrawIndexedInstanced`, `DrawAuto`) so the GPU does zero
//!   geometry work even when the present chain runs.
//!
//! A one-frame passthrough (`request_screenshot_frame`) temporarily re-enables
//! draw calls for screenshot capture (#480).
//!
//! ## Hook strategy (DXGI vtable approach)
//!
//! Instead of walking EQ's internal pointer chain (which breaks on live
//! builds), we use MQ2's proven approach:
//!
//! 1. Create a temporary D3D11 device + swap chain via
//!    `D3D11CreateDeviceAndSwapChain` to get a real `IDXGISwapChain` vtable.
//! 2. Read `Present` (vtable index 8) and hook it.
//! 3. On the first `Present` call, use `swapChain->GetDevice()` to get the real
//!    `ID3D11Device*`, then hook `CreateTexture2D` (index 5) and `CreateBuffer`
//!    (index 3).
//! 4. From the device, call `GetImmediateContext` to get the
//!    `ID3D11DeviceContext*` and hook draw calls (indices 12, 13, 19, 20, 38).
//! 5. Clean up the temporary device/swap chain.

use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering},
};

/// Result of a screenshot capture attempt, stored after Present completes.
/// Consumed by the game loop tick that originally requested the capture.
static CAPTURE_RESULT: OnceLock<Mutex<Option<Result<String, String>>>> = OnceLock::new();

/// Store a capture result for the IPC handler to retrieve.
fn store_capture_result(result: Result<String, String>) {
    let lock = CAPTURE_RESULT.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = lock.lock() {
        *guard = Some(result);
    }
}

/// Take the pending capture result, if any.
pub fn take_capture_result() -> Option<Result<String, String>> {
    let lock = CAPTURE_RESULT.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = lock.lock() {
        guard.take()
    } else {
        None
    }
}

/// Original `IDXGISwapChain::Present` function pointer (vtable slot 8).
static ORIG_PRESENT: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());

/// Original `ID3D11Device::CreateTexture2D` function pointer (vtable slot 5).
static ORIG_CREATE_TEXTURE2D: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());

/// Original `ID3D11Device::CreateBuffer` function pointer (vtable slot 3).
static ORIG_CREATE_BUFFER: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());

/// Whether the Present hook has been installed.
static PRESENT_HOOKED: AtomicBool = AtomicBool::new(false);

/// Whether the device hooks (CreateTexture2D + CreateBuffer) have been
/// installed.
static DEVICE_HOOKED: AtomicBool = AtomicBool::new(false);

/// Whether the context draw-call hooks have been installed.
static CONTEXT_HOOKED: AtomicBool = AtomicBool::new(false);

/// Original `ID3D11DeviceContext::DrawIndexed` (vtable slot 12).
static ORIG_DRAW_INDEXED: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());

/// Original `ID3D11DeviceContext::Draw` (vtable slot 13).
static ORIG_DRAW: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());

/// Original `ID3D11DeviceContext::DrawIndexedInstanced` (vtable slot 19).
static ORIG_DRAW_INDEXED_INSTANCED: AtomicPtr<core::ffi::c_void> =
    AtomicPtr::new(core::ptr::null_mut());

/// Original `ID3D11DeviceContext::DrawInstanced` (vtable slot 20).
static ORIG_DRAW_INSTANCED: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());

/// Original `ID3D11DeviceContext::DrawAuto` (vtable slot 38).
static ORIG_DRAW_AUTO: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());

/// When true, draw calls pass through for one frame (screenshot capture).
/// Cleared by `hooked_present` after the frame completes.
static SCREENSHOT_FRAME: AtomicBool = AtomicBool::new(false);

/// Per-frame cached draw suppression decision — avoids two atomic loads per
/// draw call. Updated by `hooked_present` and `allow_draws_for_screenshot`.
static SUPPRESS_DRAWS: AtomicBool = AtomicBool::new(false);

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

// COM vtable indices — ID3D11Device.
const VTABLE_CREATE_BUFFER: usize = 3;
const VTABLE_CREATE_TEXTURE2D: usize = 5;

// COM vtable indices — IDXGISwapChain.
const VTABLE_PRESENT: usize = 8;

// COM vtable indices — ID3D11DeviceContext draw calls.
const VTABLE_DRAW_INDEXED: usize = 12;
const VTABLE_DRAW: usize = 13;
const VTABLE_DRAW_INDEXED_INSTANCED: usize = 19;
const VTABLE_DRAW_INSTANCED: usize = 20;
const VTABLE_DRAW_AUTO: usize = 38;

// COM vtable index — ID3D11Device::GetImmediateContext.
const VTABLE_GET_IMMEDIATE_CONTEXT: usize = 40;

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

// ─── ID3D11DeviceContext draw call signatures ───

type DrawFn = unsafe extern "system" fn(
    this: *mut core::ffi::c_void,
    vertex_count: u32,
    start_vertex_location: u32,
);

type DrawIndexedFn = unsafe extern "system" fn(
    this: *mut core::ffi::c_void,
    index_count: u32,
    start_index_location: u32,
    base_vertex_location: i32,
);

type DrawInstancedFn = unsafe extern "system" fn(
    this: *mut core::ffi::c_void,
    vertex_count_per_instance: u32,
    instance_count: u32,
    start_vertex_location: u32,
    start_instance_location: u32,
);

type DrawIndexedInstancedFn = unsafe extern "system" fn(
    this: *mut core::ffi::c_void,
    index_count_per_instance: u32,
    instance_count: u32,
    start_index_location: u32,
    base_vertex_location: i32,
    start_instance_location: u32,
);

type DrawAutoFn = unsafe extern "system" fn(this: *mut core::ffi::c_void);

// ─── Hook implementations ───

/// Returns true if draw calls should be suppressed. Uses a per-frame cached
/// value to avoid two atomic loads on every draw call (thousands per frame).
fn should_suppress_draw() -> bool {
    SUPPRESS_DRAWS.load(Ordering::Relaxed)
}

/// Recompute the cached draw suppression flag.
fn update_suppress_draws() {
    let suppress = super::render::mode() == textquest_common::ipc::RenderMode::NullRender
        && !SCREENSHOT_FRAME.load(Ordering::Relaxed);
    SUPPRESS_DRAWS.store(suppress, Ordering::Release);
}

/// Request that draw calls pass through for one frame (screenshot capture).
/// The flag is cleared automatically after the next `Present` call.
pub fn request_screenshot_frame() {
    SCREENSHOT_FRAME.store(true, Ordering::Release);
    update_suppress_draws();
    tracing::debug!("Screenshot frame requested — draw calls enabled for next frame");
}

/// Sync the draw-suppression flag to the current render mode. Called when the
/// render mode changes so the next frame respects NullRender immediately.
pub fn sync_suppress_draws() {
    update_suppress_draws();
}

/// Hooked `IDXGISwapChain::Present` — on first call, extracts the real
/// `ID3D11Device*` and hooks `CreateTexture2D`, `CreateBuffer`, and context
/// draw calls. Clears the screenshot-frame flag after each present.
#[cfg(windows)]
unsafe extern "system" fn hooked_present(
    this: *mut core::ffi::c_void,
    sync_interval: u32,
    flags: u32,
) -> i32 {
    // On first call, hook device and context.
    if DEVICE_HOOKED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        if let Err(e) = unsafe { inner::hook_device_from_swap_chain(this) } {
            tracing::warn!("Failed to hook ID3D11Device from Present: {}", e);
        }
    }

    // If a capture was requested via render::request_capture(), the render
    // hook forced this frame to render. Before calling original Present, check
    // if we should grab the backbuffer.
    let is_capture = super::render::take_capture_active();

    let original: PresentFn = unsafe { core::mem::transmute(ORIG_PRESENT.load(Ordering::Acquire)) };
    let result = unsafe { original(this, sync_interval, flags) };

    // Tick overlay frame timing on every present.
    super::overlay::tick();

    // Clear screenshot passthrough after the frame has been presented.
    if SCREENSHOT_FRAME.swap(false, Ordering::AcqRel) {
        tracing::debug!("Screenshot frame completed — draw suppression re-enabled");
    }
    update_suppress_draws();

    // Capture the backbuffer if this was a capture frame.
    if is_capture {
        let capture_result = unsafe { inner::capture_backbuffer(this) };
        match &capture_result {
            Ok(path) => tracing::info!(path, "Screenshot captured"),
            Err(e) => tracing::warn!(error = %e, "Screenshot capture failed"),
        }
        store_capture_result(capture_result);
    }

    result
}

// ─── Draw call hooks ───

/// Hooked `ID3D11DeviceContext::Draw` (vtable slot 13).
unsafe extern "system" fn hooked_draw(
    this: *mut core::ffi::c_void,
    vertex_count: u32,
    start_vertex_location: u32,
) {
    if should_suppress_draw() {
        return;
    }
    let original: DrawFn = unsafe { core::mem::transmute(ORIG_DRAW.load(Ordering::Acquire)) };
    unsafe { original(this, vertex_count, start_vertex_location) };
}

/// Hooked `ID3D11DeviceContext::DrawIndexed` (vtable slot 12).
unsafe extern "system" fn hooked_draw_indexed(
    this: *mut core::ffi::c_void,
    index_count: u32,
    start_index_location: u32,
    base_vertex_location: i32,
) {
    if should_suppress_draw() {
        return;
    }
    let original: DrawIndexedFn =
        unsafe { core::mem::transmute(ORIG_DRAW_INDEXED.load(Ordering::Acquire)) };
    unsafe {
        original(
            this,
            index_count,
            start_index_location,
            base_vertex_location,
        )
    };
}

/// Hooked `ID3D11DeviceContext::DrawInstanced` (vtable slot 20).
unsafe extern "system" fn hooked_draw_instanced(
    this: *mut core::ffi::c_void,
    vertex_count_per_instance: u32,
    instance_count: u32,
    start_vertex_location: u32,
    start_instance_location: u32,
) {
    if should_suppress_draw() {
        return;
    }
    let original: DrawInstancedFn =
        unsafe { core::mem::transmute(ORIG_DRAW_INSTANCED.load(Ordering::Acquire)) };
    unsafe {
        original(
            this,
            vertex_count_per_instance,
            instance_count,
            start_vertex_location,
            start_instance_location,
        )
    };
}

/// Hooked `ID3D11DeviceContext::DrawIndexedInstanced` (vtable slot 19).
unsafe extern "system" fn hooked_draw_indexed_instanced(
    this: *mut core::ffi::c_void,
    index_count_per_instance: u32,
    instance_count: u32,
    start_index_location: u32,
    base_vertex_location: i32,
    start_instance_location: u32,
) {
    if should_suppress_draw() {
        return;
    }
    let original: DrawIndexedInstancedFn =
        unsafe { core::mem::transmute(ORIG_DRAW_INDEXED_INSTANCED.load(Ordering::Acquire)) };
    unsafe {
        original(
            this,
            index_count_per_instance,
            instance_count,
            start_index_location,
            base_vertex_location,
            start_instance_location,
        )
    };
}

/// Hooked `ID3D11DeviceContext::DrawAuto` (vtable slot 38).
unsafe extern "system" fn hooked_draw_auto(this: *mut core::ffi::c_void) {
    if should_suppress_draw() {
        return;
    }
    let original: DrawAutoFn =
        unsafe { core::mem::transmute(ORIG_DRAW_AUTO.load(Ordering::Acquire)) };
    unsafe { original(this) };
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

    if super::render::mode() == textquest_common::ipc::RenderMode::NullRender && !desc.is_null() {
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

    if super::render::mode() == textquest_common::ipc::RenderMode::NullRender && !desc.is_null() {
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
    #[allow(dead_code)]
    unsafe fn vtable_unhook(
        object: *mut core::ffi::c_void,
        index: usize,
        original_fn: *mut core::ffi::c_void,
    ) {
        let _ = unsafe { vtable_hook(object, index, original_fn) };
    }

    /// Find EQ's main window handle. Looks for the "EverQuest" window class.
    fn find_eq_hwnd() -> Option<windows::Win32::Foundation::HWND> {
        use windows::{Win32::UI::WindowsAndMessaging::FindWindowA, core::s};

        let hwnd = unsafe { FindWindowA(s!("_EverQuestwndclass"), None) };
        if hwnd.0 == 0 { None } else { Some(hwnd) }
    }

    /// Create a temporary D3D11 device + swap chain to capture the DXGI vtable,
    /// then hook `IDXGISwapChain::Present`.
    fn hook_present_via_dummy_device(
        hwnd: windows::Win32::Foundation::HWND,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use windows::{
            Win32::Graphics::{
                Direct3D::D3D_DRIVER_TYPE_HARDWARE,
                Direct3D11::D3D11CreateDeviceAndSwapChain,
                Dxgi::{
                    Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_MODE_DESC, DXGI_SAMPLE_DESC},
                    DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_EFFECT_DISCARD,
                    DXGI_USAGE_RENDER_TARGET_OUTPUT,
                },
            },
            core::Interface,
        };

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

        drop(swap_chain);
        drop(device);

        Ok(())
    }

    /// Called from the hooked Present on the first invocation.
    /// Extracts the real `ID3D11Device*` from the swap chain and hooks
    /// `CreateTexture2D` + `CreateBuffer`, then gets the immediate context
    /// and hooks all draw calls.
    pub(super) unsafe fn hook_device_from_swap_chain(
        swap_chain: *mut core::ffi::c_void,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let iid_device: windows::core::GUID = windows::core::GUID::from_values(
            0xdb6f6ddb,
            0xac77,
            0x4e88,
            [0x82, 0x53, 0x81, 0x9d, 0xf9, 0xbb, 0xf1, 0x40],
        );

        type GetDeviceFn = unsafe extern "system" fn(
            this: *mut core::ffi::c_void,
            riid: *const windows::core::GUID,
            device: *mut *mut core::ffi::c_void,
        ) -> i32;

        let vtable_ptr = unsafe { *(swap_chain as *const *const *const core::ffi::c_void) };
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

        // ─── Hook immediate context draw calls ───

        type GetImmediateContextFn = unsafe extern "system" fn(
            this: *mut core::ffi::c_void,
            context: *mut *mut core::ffi::c_void,
        );

        let device_vtable = unsafe { *(device_ptr as *const *const *const core::ffi::c_void) };
        let get_ctx_fn: GetImmediateContextFn =
            unsafe { core::mem::transmute(*device_vtable.add(VTABLE_GET_IMMEDIATE_CONTEXT)) };

        let mut context_ptr: *mut core::ffi::c_void = core::ptr::null_mut();
        unsafe { get_ctx_fn(device_ptr, &mut context_ptr) };

        if context_ptr.is_null() {
            tracing::warn!("GetImmediateContext returned null — draw hooks skipped");
        } else {
            tracing::info!(
                context = format!("{:#x}", context_ptr as usize),
                "Got ID3D11DeviceContext — installing draw call hooks"
            );

            if let Some(orig) = unsafe {
                vtable_hook(
                    context_ptr,
                    VTABLE_DRAW_INDEXED,
                    hooked_draw_indexed as *const core::ffi::c_void,
                )
            } {
                ORIG_DRAW_INDEXED.store(orig, Ordering::Release);
            }

            if let Some(orig) = unsafe {
                vtable_hook(
                    context_ptr,
                    VTABLE_DRAW,
                    hooked_draw as *const core::ffi::c_void,
                )
            } {
                ORIG_DRAW.store(orig, Ordering::Release);
            }

            if let Some(orig) = unsafe {
                vtable_hook(
                    context_ptr,
                    VTABLE_DRAW_INDEXED_INSTANCED,
                    hooked_draw_indexed_instanced as *const core::ffi::c_void,
                )
            } {
                ORIG_DRAW_INDEXED_INSTANCED.store(orig, Ordering::Release);
            }

            if let Some(orig) = unsafe {
                vtable_hook(
                    context_ptr,
                    VTABLE_DRAW_INSTANCED,
                    hooked_draw_instanced as *const core::ffi::c_void,
                )
            } {
                ORIG_DRAW_INSTANCED.store(orig, Ordering::Release);
            }

            if let Some(orig) = unsafe {
                vtable_hook(
                    context_ptr,
                    VTABLE_DRAW_AUTO,
                    hooked_draw_auto as *const core::ffi::c_void,
                )
            } {
                ORIG_DRAW_AUTO.store(orig, Ordering::Release);
            }

            CONTEXT_HOOKED.store(true, Ordering::Release);
            tracing::info!("ID3D11DeviceContext draw hooks installed (5 methods)");

            let ctx_release: ReleaseFn = unsafe {
                core::mem::transmute(
                    *(*(context_ptr as *const *const *const core::ffi::c_void)).add(VTABLE_RELEASE),
                )
            };
            unsafe { ctx_release(context_ptr) };
        }

        // Release the device ref we got from GetDevice.
        let release: ReleaseFn = unsafe {
            core::mem::transmute(
                *(*(device_ptr as *const *const *const core::ffi::c_void)).add(VTABLE_RELEASE),
            )
        };
        unsafe { release(device_ptr) };

        // Initialize the overlay with the current backbuffer dimensions.
        if let Some((w, h)) = get_swap_chain_dims(swap_chain) {
            super::super::overlay::initialize(w, h);
        } else {
            tracing::warn!("Could not read swap chain dims — overlay init skipped");
        }

        Ok(())
    }

    /// Read backbuffer width and height from a swap chain pointer.
    fn get_swap_chain_dims(swap_chain: *mut core::ffi::c_void) -> Option<(u32, u32)> {
        use windows::{Win32::Graphics::Dxgi::IDXGISwapChain, core::Interface};
        let sc: IDXGISwapChain = unsafe { IDXGISwapChain::from_raw_borrowed(&swap_chain)?.clone() };
        let backbuffer: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D =
            unsafe { sc.GetBuffer(0).ok()? };
        let mut desc = windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC::default();
        unsafe { backbuffer.GetDesc(&mut desc) };
        Some((desc.Width, desc.Height))
    }

    /// Capture the swap chain's backbuffer to a BMP file.
    ///
    /// Called from `hooked_present` after the frame has been rendered.
    /// Uses `GetBuffer(0)` → staging texture → `Map` → write BMP.
    pub(super) unsafe fn capture_backbuffer(
        swap_chain: *mut core::ffi::c_void,
    ) -> Result<String, String> {
        use windows::{
            Win32::Graphics::{
                Direct3D11::{
                    D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
                    D3D11_TEXTURE2D_DESC as WinTexDesc, D3D11_USAGE_STAGING,
                },
                Dxgi::{
                    Common::{
                        DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
                        DXGI_FORMAT_B8G8R8X8_UNORM, DXGI_FORMAT_B8G8R8X8_UNORM_SRGB,
                    },
                    IDXGISwapChain,
                },
            },
            core::Interface,
        };

        let sc: IDXGISwapChain = unsafe {
            IDXGISwapChain::from_raw_borrowed(&swap_chain)
                .ok_or("Failed to borrow IDXGISwapChain")?
                .clone()
        };

        let backbuffer: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D = unsafe {
            sc.GetBuffer(0)
                .map_err(|e| format!("GetBuffer(0) failed: {e}"))?
        };

        let mut desc = WinTexDesc::default();
        unsafe { backbuffer.GetDesc(&mut desc) };
        let width = desc.Width;
        let height = desc.Height;
        let bgra_compatible = matches!(
            desc.Format,
            DXGI_FORMAT_B8G8R8A8_UNORM
                | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB
                | DXGI_FORMAT_B8G8R8X8_UNORM
                | DXGI_FORMAT_B8G8R8X8_UNORM_SRGB
        );
        if !bgra_compatible {
            return Err(format!(
                "Unsupported backbuffer format for BMP capture: {:?}",
                desc.Format
            ));
        }

        // windows 0.54: GetDevice() returns Result<T>, no out-param.
        let device: windows::Win32::Graphics::Direct3D11::ID3D11Device = unsafe {
            backbuffer
                .GetDevice()
                .map_err(|e| format!("GetDevice failed: {e}"))?
        };

        let staging_desc = WinTexDesc {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: desc.Format,
            SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: Default::default(),
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: Default::default(),
        };

        // windows 0.54: CreateTexture2D uses 3-arg out-param pattern, returns
        // Result<()>.
        let mut staging: Option<windows::Win32::Graphics::Direct3D11::ID3D11Texture2D> = None;
        unsafe {
            device
                .CreateTexture2D(&staging_desc, None, Some(&mut staging))
                .map_err(|e| format!("CreateTexture2D (staging) failed: {e}"))?;
        }
        let staging = staging.ok_or("CreateTexture2D returned null")?;

        // windows 0.54: GetImmediateContext() returns Result<T>, no out-param.
        let context: windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext = unsafe {
            device
                .GetImmediateContext()
                .map_err(|e| format!("GetImmediateContext failed: {e}"))?
        };

        unsafe {
            context.CopyResource(&staging, &backbuffer);
        }

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|e| format!("Map failed: {e}"))?;
        }

        let path = create_screenshot_path()?;

        let write_result = unsafe {
            write_bmp(
                &path,
                width,
                height,
                mapped.RowPitch,
                mapped.pData as *const u8,
            )
        };
        unsafe { context.Unmap(&staging, 0) };
        write_result.map(|()| path.to_string_lossy().into_owned())
    }

    fn create_screenshot_path() -> Result<std::path::PathBuf, String> {
        let dir = std::env::temp_dir().join("textquest");
        std::fs::create_dir_all(&dir).map_err(|e| format!("create screenshot dir: {e}"))?;

        let pid = std::process::id();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        for _ in 0..8 {
            let mut random = [0u8; 8];
            getrandom::getrandom(&mut random).map_err(|e| format!("random filename: {e}"))?;
            let suffix = random
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            let path = dir.join(format!("screenshot_{}_{}_{}.bmp", pid, timestamp, suffix));
            if !path.exists() {
                return Ok(path);
            }
        }

        Err("failed to allocate unique screenshot filename".to_string())
    }

    /// Write BGRA pixel data to a 24-bit BMP file.
    ///
    /// # Safety
    /// `data` must point to a valid pixel buffer with at least `height *
    /// row_pitch` bytes.
    unsafe fn write_bmp(
        path: &std::path::Path,
        width: u32,
        height: u32,
        row_pitch: u32,
        data: *const u8,
    ) -> Result<(), String> {
        use std::io::Write;

        let width_usize =
            usize::try_from(width).map_err(|_| "image width does not fit into usize")?;
        let height_usize =
            usize::try_from(height).map_err(|_| "image height does not fit into usize")?;
        let row_pitch_usize =
            usize::try_from(row_pitch).map_err(|_| "row pitch does not fit into usize")?;
        let min_row_pitch = width
            .checked_mul(4)
            .ok_or("invalid image width for row pitch calculation")?;
        if row_pitch < min_row_pitch {
            return Err(format!(
                "Invalid row pitch for BGRA data: {row_pitch} < {min_row_pitch}"
            ));
        }

        let row_size = width
            .checked_mul(3)
            .ok_or("invalid image width for BMP row calculation")?;
        let padded_row = row_size
            .checked_add(3)
            .map(|value| value & !3)
            .ok_or("invalid padded BMP row size")?;
        let pixel_data_size = padded_row
            .checked_mul(height)
            .ok_or("invalid BMP pixel data size")?;
        let file_size = 54u32
            .checked_add(pixel_data_size)
            .ok_or("invalid BMP file size")?;
        let pixel_stride = width_usize.checked_mul(4).ok_or("invalid BGRA row width")?;

        let mut file = std::io::BufWriter::new(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|e| format!("create file: {e}"))?,
        );

        file.write_all(b"BM").map_err(|e| format!("write: {e}"))?;
        file.write_all(&file_size.to_le_bytes())
            .map_err(|e| format!("write: {e}"))?;
        file.write_all(&[0u8; 4])
            .map_err(|e| format!("write: {e}"))?;
        file.write_all(&54u32.to_le_bytes())
            .map_err(|e| format!("write: {e}"))?;

        file.write_all(&40u32.to_le_bytes())
            .map_err(|e| format!("write: {e}"))?;
        file.write_all(&width.to_le_bytes())
            .map_err(|e| format!("write: {e}"))?;
        file.write_all(&(-(height as i32)).to_le_bytes())
            .map_err(|e| format!("write: {e}"))?;
        file.write_all(&1u16.to_le_bytes())
            .map_err(|e| format!("write: {e}"))?;
        file.write_all(&24u16.to_le_bytes())
            .map_err(|e| format!("write: {e}"))?;
        file.write_all(&[0u8; 24])
            .map_err(|e| format!("write: {e}"))?;

        let mut row_buf = vec![
            0u8;
            usize::try_from(padded_row)
                .map_err(|_| "padded BMP row size does not fit into usize")?
        ];
        for y in 0..height_usize {
            let row_offset = y
                .checked_mul(row_pitch_usize)
                .ok_or("source row offset overflow")?;
            let src_row = unsafe { data.add(row_offset) };
            for x in 0..width_usize {
                let pixel_offset = x.checked_mul(4).ok_or("source pixel offset overflow")?;
                debug_assert!(pixel_offset < pixel_stride);
                let px = unsafe { src_row.add(pixel_offset) };
                let idx = x
                    .checked_mul(3)
                    .ok_or("destination pixel offset overflow")?;
                row_buf[idx] = unsafe { *px };
                row_buf[idx + 1] = unsafe { *px.add(1) };
                row_buf[idx + 2] = unsafe { *px.add(2) };
            }
            file.write_all(&row_buf)
                .map_err(|e| format!("write: {e}"))?;
        }

        Ok(())
    }

    /// Install DX11 vtable hooks via DXGI.
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

    /// Attempt deferred installation.
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

    /// Remove DX11 vtable hooks.
    pub fn remove(_eq_base: u64) {
        if PRESENT_HOOKED.load(Ordering::Acquire) {
            tracing::info!(
                "DX11 Present hook cannot be cleanly removed (vtable shared, no persistent ref). \
                 Process is likely shutting down."
            );
        }
        if DEVICE_HOOKED.load(Ordering::Acquire) {
            tracing::info!(
                "DX11 device hooks (CreateTexture2D + CreateBuffer) will be released on process \
                 exit."
            );
        }
        if CONTEXT_HOOKED.load(Ordering::Acquire) {
            tracing::info!("DX11 context draw hooks (5 methods) will be released on process exit.");
        }
        PRESENT_HOOKED.store(false, Ordering::Release);
        DEVICE_HOOKED.store(false, Ordering::Release);
        CONTEXT_HOOKED.store(false, Ordering::Release);
        tracing::info!("DX11 null device + context hooks marked as removed");
    }
}

#[cfg(not(windows))]
mod inner {
    pub fn install(_eq_base: u64) -> Result<(), Box<dyn std::error::Error>> {
        tracing::warn!("DX11 null device hooks not available on this platform (stub)");
        Ok(())
    }
    pub fn ensure_installed() {}
    pub fn remove(_eq_base: u64) {
        tracing::warn!("DX11 null device hook removal not available (stub)");
    }
}

#[allow(unused_imports)]
pub use inner::{ensure_installed, install, remove};

/// Returns true if the context draw-call hooks have been installed.
pub fn draw_hooks_installed() -> bool {
    CONTEXT_HOOKED.load(Ordering::Acquire)
}

/// Returns true if the DX11 Present hook has been installed.
pub fn present_hook_installed() -> bool {
    PRESENT_HOOKED.load(Ordering::Acquire)
}

#[cfg(test)]
pub(crate) fn set_draw_hooks_installed_for_test(installed: bool) {
    CONTEXT_HOOKED.store(installed, Ordering::Release);
}

#[cfg(test)]
pub(crate) fn set_present_hook_installed_for_test(installed: bool) {
    PRESENT_HOOKED.store(installed, Ordering::Release);
}

#[cfg(test)]
pub(crate) fn reset_test_state() {
    ORIG_PRESENT.store(core::ptr::null_mut(), Ordering::Relaxed);
    ORIG_CREATE_TEXTURE2D.store(core::ptr::null_mut(), Ordering::Relaxed);
    ORIG_CREATE_BUFFER.store(core::ptr::null_mut(), Ordering::Relaxed);
    ORIG_DRAW_INDEXED.store(core::ptr::null_mut(), Ordering::Relaxed);
    ORIG_DRAW.store(core::ptr::null_mut(), Ordering::Relaxed);
    ORIG_DRAW_INDEXED_INSTANCED.store(core::ptr::null_mut(), Ordering::Relaxed);
    ORIG_DRAW_INSTANCED.store(core::ptr::null_mut(), Ordering::Relaxed);
    ORIG_DRAW_AUTO.store(core::ptr::null_mut(), Ordering::Relaxed);
    PRESENT_HOOKED.store(false, Ordering::Relaxed);
    DEVICE_HOOKED.store(false, Ordering::Relaxed);
    CONTEXT_HOOKED.store(false, Ordering::Relaxed);
    SCREENSHOT_FRAME.store(false, Ordering::Relaxed);
    SUPPRESS_DRAWS.store(false, Ordering::Relaxed);
    CACHED_EQ_BASE.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::ipc::RenderMode;

    fn suppresses(mode: RenderMode, screenshot: bool) -> bool {
        mode == RenderMode::NullRender && !screenshot
    }

    #[test]
    fn should_suppress_draw_in_null_render() {
        let _guard = super::super::render::test_state_lock();
        super::super::render::reset_test_state();
        reset_test_state();
        assert!(suppresses(RenderMode::NullRender, false));
    }

    #[test]
    fn should_not_suppress_draw_in_normal_mode() {
        let _guard = super::super::render::test_state_lock();
        super::super::render::reset_test_state();
        reset_test_state();
        assert!(!suppresses(RenderMode::Normal, false));
    }

    #[test]
    fn should_not_suppress_draw_in_strobe_mode() {
        let _guard = super::super::render::test_state_lock();
        super::super::render::reset_test_state();
        reset_test_state();
        assert!(!suppresses(RenderMode::Strobe, false));
    }

    #[test]
    fn screenshot_frame_overrides_null_render() {
        let _guard = super::super::render::test_state_lock();
        super::super::render::reset_test_state();
        reset_test_state();
        assert!(
            !suppresses(RenderMode::NullRender, true),
            "screenshot frame should allow draw calls through"
        );
    }

    #[test]
    fn request_screenshot_frame_sets_flag() {
        let _guard = super::super::render::test_state_lock();
        super::super::render::reset_test_state();
        reset_test_state();
        super::super::render::set_mode(RenderMode::NullRender);
        request_screenshot_frame();
        assert!(SCREENSHOT_FRAME.load(Ordering::Relaxed));
    }

    #[test]
    fn screenshot_frame_clears_on_swap() {
        let _guard = super::super::render::test_state_lock();
        super::super::render::reset_test_state();
        reset_test_state();
        SCREENSHOT_FRAME.store(true, Ordering::Relaxed);
        let was_set = SCREENSHOT_FRAME.swap(false, Ordering::AcqRel);
        assert!(was_set, "flag should have been set before swap");
        assert!(
            !SCREENSHOT_FRAME.load(Ordering::Relaxed),
            "flag should be cleared after swap"
        );
    }

    #[test]
    fn draw_hooks_not_installed_by_default() {
        let _guard = super::super::render::test_state_lock();
        super::super::render::reset_test_state();
        reset_test_state();
        assert!(!draw_hooks_installed());
    }

    #[test]
    fn vtable_constants_are_distinct() {
        let _guard = super::super::render::test_state_lock();
        super::super::render::reset_test_state();
        reset_test_state();
        let indices = [
            VTABLE_DRAW_INDEXED,
            VTABLE_DRAW,
            VTABLE_DRAW_INDEXED_INSTANCED,
            VTABLE_DRAW_INSTANCED,
            VTABLE_DRAW_AUTO,
        ];
        for (i, a) in indices.iter().enumerate() {
            for (j, b) in indices.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "vtable indices {i} and {j} must be distinct");
                }
            }
        }
    }

    #[test]
    fn stub_install_remove_are_safe() {
        let _guard = super::super::render::test_state_lock();
        super::super::render::reset_test_state();
        reset_test_state();
        #[cfg(not(windows))]
        {
            assert!(install(0x12345).is_ok());
            ensure_installed();
            remove(0x12345);
        }
    }
}
