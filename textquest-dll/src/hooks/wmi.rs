//! WMI query evasion for COM/DCOM-based fingerprinting.
//!
//! EQ can reach hardware and process inventory through WMI, bypassing the
//! direct Win32 hooks. This module hooks `CoCreateInstance` for
//! `CLSID_WbemLocator`, vtable-hooks the returned `IWbemLocator`, then
//! vtable-hooks `IWbemServices::ExecQuery` after namespace connection.

const TEXTQUEST_PROCESS_FILTER: &str = "(Name <> 'textquest.exe' AND Name <> \
                                        'textquest-web.exe' AND Name <> \
                                        'textquest-soul.exe' AND Name <> \
                                        'llm-client.exe' AND Name <> \
                                        'eqdiff.exe')";

const PROCESS_INVENTORY_CLASSES: &[&str] = &[
    "win32_process",
    "win32_perfrawdata_perfproc_process",
    "win32_perfformatteddata_perfproc_process",
];

const FINGERPRINTING_CLASSES: &[&str] = &[
    "cim_diskdrive",
    "cim_networkadapter",
    "cim_processor",
    "cim_videocontroller",
    "win32_baseboard",
    "win32_bios",
    "win32_computersystem",
    "win32_computersystemproduct",
    "win32_desktopmonitor",
    "win32_diskdrive",
    "win32_logicaldisk",
    "win32_networkadapter",
    "win32_networkadapterconfiguration",
    "win32_physicalmedia",
    "win32_pnpentity",
    "win32_portconnector",
    "win32_processor",
    "win32_systemenclosure",
    "win32_videocontroller",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WmiQueryPolicy {
    PassThrough,
    RewriteProcessInventory,
    BlockFingerprintingQuery,
}

pub(crate) fn classify_wmi_query(query: &str) -> WmiQueryPolicy {
    let normalized = normalize_wql(query);

    if contains_any_class(&normalized, FINGERPRINTING_CLASSES) {
        return WmiQueryPolicy::BlockFingerprintingQuery;
    }

    if contains_any_class(&normalized, PROCESS_INVENTORY_CLASSES) {
        return WmiQueryPolicy::RewriteProcessInventory;
    }

    WmiQueryPolicy::PassThrough
}

pub(crate) fn rewrite_process_inventory_query(query: &str) -> Option<String> {
    let normalized = normalize_wql(query);
    if !normalized.contains("win32_process") || !normalized.contains("select ") {
        return None;
    }

    let trimmed = query.trim().trim_end_matches(';').trim_end();
    if trimmed.is_empty() {
        return None;
    }

    if has_where_clause(trimmed) {
        Some(format!("{trimmed} AND {TEXTQUEST_PROCESS_FILTER}"))
    } else {
        Some(format!("{trimmed} WHERE {TEXTQUEST_PROCESS_FILTER}"))
    }
}

fn normalize_wql(query: &str) -> String {
    query
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_any_class(normalized_query: &str, classes: &[&str]) -> bool {
    classes.iter().any(|class| normalized_query.contains(class))
}

fn has_where_clause(query: &str) -> bool {
    query
        .split_whitespace()
        .any(|token| token.eq_ignore_ascii_case("where"))
}

#[cfg(not(windows))]
pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    tracing::warn!("WMI evasion hook not available on this platform (stub)");
    Ok(())
}

#[cfg(not(windows))]
pub fn remove() {
    tracing::warn!("WMI evasion hook removal not available (stub)");
}

#[cfg(windows)]
mod inner {
    use std::{
        ffi::c_void,
        ptr,
        sync::{
            OnceLock,
            atomic::{AtomicBool, AtomicPtr, Ordering},
        },
    };

    use retour::static_detour;
    use windows::{
        Win32::System::{
            LibraryLoader::{GetModuleHandleA, GetProcAddress, LoadLibraryA},
            Memory::{PAGE_PROTECTION_FLAGS, PAGE_READWRITE, VirtualProtect},
        },
        core::{GUID, HRESULT, s},
    };

    use super::{WmiQueryPolicy, classify_wmi_query, rewrite_process_inventory_query};

    const VTABLE_IWBEM_LOCATOR_CONNECT_SERVER: usize = 3;
    const VTABLE_IWBEM_SERVICES_EXEC_QUERY: usize = 20;

    const E_FAIL: HRESULT = HRESULT(0x8000_4005_u32 as i32);
    const WBEM_E_ACCESS_DENIED: HRESULT = HRESULT(0x8004_1003_u32 as i32);

    type CoCreateInstanceFn = unsafe extern "system" fn(
        rclsid: *const GUID,
        p_unk_outer: *mut c_void,
        cls_context: u32,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT;

    type ConnectServerFn = unsafe extern "system" fn(
        this: *mut c_void,
        network_resource: *const u16,
        user: *const u16,
        password: *const u16,
        locale: *const u16,
        security_flags: i32,
        authority: *const u16,
        ctx: *mut c_void,
        services: *mut *mut c_void,
    ) -> HRESULT;

    type ExecQueryFn = unsafe extern "system" fn(
        this: *mut c_void,
        query_language: *const u16,
        query: *const u16,
        flags: i32,
        ctx: *mut c_void,
        enum_result: *mut *mut c_void,
    ) -> HRESULT;

    type SysAllocStringLenFn = unsafe extern "system" fn(*const u16, u32) -> *mut u16;
    type SysFreeStringFn = unsafe extern "system" fn(*mut u16);

    static INSTALLED: AtomicBool = AtomicBool::new(false);
    static ORIGINAL_CONNECT_SERVER: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
    static LOCATOR_CONNECT_ENTRY: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
    static ORIGINAL_EXEC_QUERY: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
    static SERVICES_EXEC_ENTRY: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
    static OLEAUT32: OnceLock<windows::Win32::Foundation::HMODULE> = OnceLock::new();

    static_detour! {
        static CoCreateInstanceHook: unsafe extern "system" fn(
            *const GUID,
            *mut c_void,
            u32,
            *const GUID,
            *mut *mut c_void,
        ) -> HRESULT;
    }

    fn clsid_wbem_locator() -> GUID {
        GUID::from_values(
            0x4590_f811,
            0x1d3a,
            0x11d0,
            [0x89, 0x1f, 0x00, 0xaa, 0x00, 0x4b, 0x2e, 0x24],
        )
    }

    fn iid_iwbem_locator() -> GUID {
        GUID::from_values(
            0xdc12_a687,
            0x737f,
            0x11cf,
            [0x88, 0x4d, 0x00, 0xaa, 0x00, 0x4b, 0x2e, 0x24],
        )
    }

    fn guid_matches(actual: *const GUID, expected: GUID) -> bool {
        if actual.is_null() {
            return false;
        }

        unsafe { *actual == expected }
    }

    unsafe extern "system" fn co_create_instance_detour(
        rclsid: *const GUID,
        p_unk_outer: *mut c_void,
        cls_context: u32,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT {
        let hr = unsafe { CoCreateInstanceHook.call(rclsid, p_unk_outer, cls_context, riid, ppv) };

        if hr.is_ok()
            && guid_matches(rclsid, clsid_wbem_locator())
            && guid_matches(riid, iid_iwbem_locator())
            && !ppv.is_null()
        {
            let locator = unsafe { *ppv };
            unsafe { hook_locator_vtable(locator) };
        }

        hr
    }

    unsafe extern "system" fn connect_server_detour(
        this: *mut c_void,
        network_resource: *const u16,
        user: *const u16,
        password: *const u16,
        locale: *const u16,
        security_flags: i32,
        authority: *const u16,
        ctx: *mut c_void,
        services: *mut *mut c_void,
    ) -> HRESULT {
        let original = ORIGINAL_CONNECT_SERVER.load(Ordering::Acquire);
        if original.is_null() {
            return E_FAIL;
        }

        let original: ConnectServerFn = unsafe { std::mem::transmute(original) };
        let hr = unsafe {
            original(
                this,
                network_resource,
                user,
                password,
                locale,
                security_flags,
                authority,
                ctx,
                services,
            )
        };

        if hr.is_ok() && !services.is_null() {
            let namespace = unsafe { bstr_to_string(network_resource) }
                .unwrap_or_else(|| "<unknown>".to_string());
            tracing::debug!(namespace = %namespace, "WMI namespace connected");

            let service_ptr = unsafe { *services };
            unsafe { hook_services_vtable(service_ptr) };
        }

        hr
    }

    unsafe extern "system" fn exec_query_detour(
        this: *mut c_void,
        query_language: *const u16,
        query: *const u16,
        flags: i32,
        ctx: *mut c_void,
        enum_result: *mut *mut c_void,
    ) -> HRESULT {
        let original = ORIGINAL_EXEC_QUERY.load(Ordering::Acquire);
        if original.is_null() {
            return E_FAIL;
        }
        let original: ExecQueryFn = unsafe { std::mem::transmute(original) };

        let language = unsafe { bstr_to_string(query_language) }.unwrap_or_default();
        if !language.is_empty() && !language.eq_ignore_ascii_case("WQL") {
            return unsafe { original(this, query_language, query, flags, ctx, enum_result) };
        }

        let Some(query_text) = (unsafe { bstr_to_string(query) }) else {
            return unsafe { original(this, query_language, query, flags, ctx, enum_result) };
        };

        match classify_wmi_query(&query_text) {
            WmiQueryPolicy::PassThrough => unsafe {
                original(this, query_language, query, flags, ctx, enum_result)
            },
            WmiQueryPolicy::RewriteProcessInventory => {
                if let Some(rewritten) = rewrite_process_inventory_query(&query_text) {
                    if let Some(rewritten_bstr) = OwnedBstr::new(&rewritten) {
                        tracing::debug!("Rewriting WMI process inventory query");
                        return unsafe {
                            original(
                                this,
                                query_language,
                                rewritten_bstr.as_ptr(),
                                flags,
                                ctx,
                                enum_result,
                            )
                        };
                    }
                }

                tracing::warn!("Blocking WMI process inventory query after rewrite failure");
                clear_enum_result(enum_result);
                WBEM_E_ACCESS_DENIED
            }
            WmiQueryPolicy::BlockFingerprintingQuery => {
                tracing::debug!("Blocking WMI hardware fingerprinting query");
                clear_enum_result(enum_result);
                WBEM_E_ACCESS_DENIED
            }
        }
    }

    fn clear_enum_result(enum_result: *mut *mut c_void) {
        if !enum_result.is_null() {
            unsafe {
                *enum_result = ptr::null_mut();
            }
        }
    }

    unsafe fn hook_locator_vtable(locator: *mut c_void) {
        unsafe {
            hook_vtable_once(
                locator,
                VTABLE_IWBEM_LOCATOR_CONNECT_SERVER,
                connect_server_detour as *mut c_void,
                &ORIGINAL_CONNECT_SERVER,
                &LOCATOR_CONNECT_ENTRY,
                "IWbemLocator::ConnectServer",
            );
        }
    }

    unsafe fn hook_services_vtable(services: *mut c_void) {
        unsafe {
            hook_vtable_once(
                services,
                VTABLE_IWBEM_SERVICES_EXEC_QUERY,
                exec_query_detour as *mut c_void,
                &ORIGINAL_EXEC_QUERY,
                &SERVICES_EXEC_ENTRY,
                "IWbemServices::ExecQuery",
            );
        }
    }

    unsafe fn hook_vtable_once(
        object: *mut c_void,
        index: usize,
        hook_fn: *mut c_void,
        original_slot: &AtomicPtr<c_void>,
        entry_slot: &AtomicPtr<c_void>,
        label: &str,
    ) {
        if object.is_null() || !original_slot.load(Ordering::Acquire).is_null() {
            return;
        }

        let vtable = unsafe { *(object as *const *mut *mut c_void) };
        if vtable.is_null() {
            return;
        }

        let entry = unsafe { vtable.add(index) };
        let original = unsafe { *entry };
        if original.is_null() || original == hook_fn {
            return;
        }

        let mut old_protect = PAGE_PROTECTION_FLAGS(0);
        let entry_size = core::mem::size_of::<*mut c_void>();
        let protected = unsafe {
            VirtualProtect(
                entry as *const c_void,
                entry_size,
                PAGE_READWRITE,
                &mut old_protect,
            )
        };
        if protected.is_err() {
            tracing::warn!(method = label, "Failed to make WMI vtable writable");
            return;
        }

        unsafe {
            *entry = hook_fn;
            let _ = VirtualProtect(
                entry as *const c_void,
                entry_size,
                old_protect,
                &mut old_protect,
            );
        }

        original_slot.store(original, Ordering::Release);
        entry_slot.store(entry as *mut c_void, Ordering::Release);
        tracing::info!(method = label, "WMI COM vtable hook installed");
    }

    unsafe fn restore_vtable_entry(
        entry_slot: &AtomicPtr<c_void>,
        original_slot: &AtomicPtr<c_void>,
        hook_fn: *mut c_void,
    ) {
        let entry_ptr = entry_slot.swap(ptr::null_mut(), Ordering::AcqRel);
        let original = original_slot.swap(ptr::null_mut(), Ordering::AcqRel);
        if entry_ptr.is_null() || original.is_null() {
            return;
        }

        let entry = entry_ptr as *mut *mut c_void;
        if unsafe { *entry } != hook_fn {
            return;
        }

        let mut old_protect = PAGE_PROTECTION_FLAGS(0);
        let entry_size = core::mem::size_of::<*mut c_void>();
        let protected = unsafe {
            VirtualProtect(
                entry as *const c_void,
                entry_size,
                PAGE_READWRITE,
                &mut old_protect,
            )
        };
        if protected.is_err() {
            return;
        }

        unsafe {
            *entry = original;
            let _ = VirtualProtect(
                entry as *const c_void,
                entry_size,
                old_protect,
                &mut old_protect,
            );
        }
    }

    unsafe fn bstr_to_string(bstr: *const u16) -> Option<String> {
        if bstr.is_null() {
            return None;
        }

        let byte_len_ptr = unsafe { (bstr as *const u8).sub(4) as *const u32 };
        let byte_len = unsafe { *byte_len_ptr } as usize;
        let char_len = byte_len / 2;
        let slice = unsafe { std::slice::from_raw_parts(bstr, char_len) };
        Some(String::from_utf16_lossy(slice))
    }

    struct OwnedBstr {
        ptr: *mut u16,
        free: SysFreeStringFn,
    }

    impl OwnedBstr {
        fn new(value: &str) -> Option<Self> {
            let alloc = resolve_oleaut32_proc::<SysAllocStringLenFn>(s!("SysAllocStringLen"))?;
            let free = resolve_oleaut32_proc::<SysFreeStringFn>(s!("SysFreeString"))?;
            let wide: Vec<u16> = value.encode_utf16().collect();
            let ptr = unsafe { alloc(wide.as_ptr(), wide.len() as u32) };
            if ptr.is_null() {
                return None;
            }

            Some(Self { ptr, free })
        }

        fn as_ptr(&self) -> *const u16 {
            self.ptr
        }
    }

    impl Drop for OwnedBstr {
        fn drop(&mut self) {
            unsafe {
                (self.free)(self.ptr);
            }
        }
    }

    fn resolve_oleaut32_proc<T>(name: windows::core::PCSTR) -> Option<T> {
        let module = *OLEAUT32.get_or_init(|| unsafe {
            GetModuleHandleA(s!("oleaut32.dll"))
                .or_else(|_| LoadLibraryA(s!("oleaut32.dll")))
                .unwrap_or_default()
        });

        if module.0 == 0 {
            return None;
        }

        let proc = unsafe { GetProcAddress(module, name) }?;
        Some(unsafe { std::mem::transmute_copy(&proc) })
    }

    pub fn install() -> Result<(), Box<dyn std::error::Error>> {
        if INSTALLED.swap(true, Ordering::AcqRel) {
            tracing::debug!("WMI evasion hook already installed, skipping");
            return Ok(());
        }

        let install_result = unsafe {
            let ole32 = match GetModuleHandleA(s!("ole32.dll")) {
                Ok(module) => module,
                Err(_) => LoadLibraryA(s!("ole32.dll"))?,
            };
            let co_create_addr = GetProcAddress(ole32, s!("CoCreateInstance"))
                .ok_or("GetProcAddress(CoCreateInstance) returned None")?;
            let co_create: CoCreateInstanceFn = std::mem::transmute(co_create_addr);

            CoCreateInstanceHook.initialize(co_create, co_create_instance_detour)?;
            CoCreateInstanceHook.enable()?;
            Ok::<(), Box<dyn std::error::Error>>(())
        };

        if install_result.is_err() {
            INSTALLED.store(false, Ordering::Release);
        } else {
            tracing::info!("WMI CoCreateInstance hook installed");
        }

        install_result
    }

    pub fn remove() {
        unsafe {
            if CoCreateInstanceHook.is_enabled() {
                let _ = CoCreateInstanceHook.disable();
            }

            restore_vtable_entry(
                &SERVICES_EXEC_ENTRY,
                &ORIGINAL_EXEC_QUERY,
                exec_query_detour as *mut c_void,
            );
            restore_vtable_entry(
                &LOCATOR_CONNECT_ENTRY,
                &ORIGINAL_CONNECT_SERVER,
                connect_server_detour as *mut c_void,
            );
        }
        INSTALLED.store(false, Ordering::Release);
        tracing::info!("WMI evasion hook removed");
    }
}

#[cfg(windows)]
pub use inner::{install, remove};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_inventory_queries_are_rewritten() {
        assert_eq!(
            classify_wmi_query("SELECT ProcessId, Name FROM Win32_Process"),
            WmiQueryPolicy::RewriteProcessInventory
        );
    }

    #[test]
    fn process_query_rewrite_adds_where_clause() {
        let rewritten =
            rewrite_process_inventory_query("SELECT ProcessId, Name FROM Win32_Process").unwrap();

        assert!(rewritten.contains(" FROM Win32_Process WHERE "));
        assert!(rewritten.contains("Name <> 'textquest.exe'"));
        assert!(rewritten.contains("Name <> 'textquest-web.exe'"));
    }

    #[test]
    fn process_query_rewrite_extends_existing_where_clause() {
        let rewritten =
            rewrite_process_inventory_query("SELECT * FROM Win32_Process WHERE SessionId = 1")
                .unwrap();

        assert!(rewritten.contains("WHERE SessionId = 1 AND "));
        assert!(rewritten.contains("Name <> 'textquest.exe'"));
    }

    #[test]
    fn hardware_fingerprint_queries_are_blocked() {
        assert_eq!(
            classify_wmi_query("select SerialNumber from Win32_BIOS"),
            WmiQueryPolicy::BlockFingerprintingQuery
        );
        assert_eq!(
            classify_wmi_query("SELECT MACAddress FROM Win32_NetworkAdapter"),
            WmiQueryPolicy::BlockFingerprintingQuery
        );
    }

    #[test]
    fn unrelated_queries_pass_through() {
        assert_eq!(
            classify_wmi_query("SELECT * FROM Win32_OperatingSystem"),
            WmiQueryPolicy::PassThrough
        );
    }
}
