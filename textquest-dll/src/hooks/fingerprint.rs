//! Hardware fingerprint spoofing — intercepts `SystemFingerprint` to return
//! unique per-client values for VideoCardId, NetworkCardId, HardriveId (sic),
//! and ComputerName.
//!
//! Without this hook, all 36 multibox clients send identical hardware
//! fingerprints — the most obvious server-side detection vector.
//!
//! Spoofed values are derived deterministically from the IPC session token
//! so they remain stable across client restarts for the same account slot.

use std::sync::OnceLock;

/// Per-client spoofed fingerprint values, generated once from the session
/// token.
static SPOOFED: OnceLock<SpoofedFingerprint> = OnceLock::new();
static FIRMWARE: OnceLock<SpoofedFirmwareTables> = OnceLock::new();

const RSMB_PROVIDER: u32 = fourcc(*b"RSMB");
const ACPI_PROVIDER: u32 = fourcc(*b"ACPI");
const RSMB_TABLE_ID: u32 = 0;

const fn fourcc(tag: [u8; 4]) -> u32 {
    (tag[0] as u32) | ((tag[1] as u32) << 8) | ((tag[2] as u32) << 16) | ((tag[3] as u32) << 24)
}

/// The four hardware fingerprint fields that EQ's SystemFingerprint sends.
#[derive(Debug, Clone)]
pub struct SpoofedFingerprint {
    pub video_card_id: String,
    pub network_card_id: String,
    pub hard_drive_id: String,
    pub computer_name: String,
}

#[derive(Debug, Clone)]
struct SpoofedFirmwareTables {
    rsmb: Vec<u8>,
    acpi: Vec<SpoofedAcpiTable>,
}

#[derive(Debug, Clone)]
struct SpoofedAcpiTable {
    id: u32,
    data: Vec<u8>,
}

/// Initialize spoofed fingerprint values from the session token.
///
/// Must be called before the fingerprint hook fires (i.e., during DLL init).
/// Uses domain-separated hashing to derive plausible-looking unique values.
pub fn init(session_token: &[u8; 32]) {
    let fp = generate_fingerprint(session_token);
    let firmware = generate_firmware_tables(session_token);
    tracing::info!(
        video = %fp.video_card_id,
        nic = %fp.network_card_id,
        disk = %fp.hard_drive_id,
        host = %fp.computer_name,
        "Spoofed fingerprint initialized"
    );
    let _ = SPOOFED.set(fp);
    let _ = FIRMWARE.set(firmware);
}

/// Get the spoofed fingerprint (None if init hasn't been called).
pub fn spoofed() -> Option<&'static SpoofedFingerprint> {
    SPOOFED.get()
}

fn spoofed_firmware() -> Option<&'static SpoofedFirmwareTables> {
    FIRMWARE.get()
}

/// Generate deterministic per-client fingerprint values from the session token.
///
/// Each field uses a different domain prefix hashed with the token to produce
/// unique values that look like real hardware IDs.
fn generate_fingerprint(token: &[u8; 32]) -> SpoofedFingerprint {
    SpoofedFingerprint {
        // GPU ID: PCI vendor:device format (e.g., "PCI\VEN_10DE&DEV_2684")
        video_card_id: generate_gpu_id(token),
        // NIC: MAC-like format (e.g., "00-1A-2B-3C-4D-5E")
        network_card_id: generate_mac_address(token),
        // Disk: volume serial (e.g., "A1B2-C3D4")
        hard_drive_id: generate_disk_serial(token),
        // Hostname: plausible Windows hostname (e.g., "DESKTOP-A1B2C3D")
        computer_name: generate_hostname(token),
    }
}

/// Derive a deterministic hash from the token with a domain separator.
/// Uses SipHash-like mixing (FNV-1a variant) for simplicity — we don't need
/// cryptographic strength, just uniqueness and determinism.
fn derive_bytes(token: &[u8; 32], domain: &[u8]) -> [u8; 16] {
    // FNV-1a 128-bit with domain separation
    let mut h: u128 = 0x6C62_272E_07BB_0142_62B8_2175_6295_C58D;
    let prime: u128 = 0x0000_0000_0000_0100_0000_0000_0000_013B;

    // Mix in domain
    for &b in domain {
        h ^= b as u128;
        h = h.wrapping_mul(prime);
    }
    // Separator
    h ^= 0xFF_u128;
    h = h.wrapping_mul(prime);
    // Mix in token
    for &b in token {
        h ^= b as u128;
        h = h.wrapping_mul(prime);
    }

    h.to_le_bytes()
}

fn generate_gpu_id(token: &[u8; 32]) -> String {
    let bytes = derive_bytes(token, b"gpu");
    // Use realistic NVIDIA vendor ID (10DE) with a derived device ID
    let device = u16::from_le_bytes([bytes[0], bytes[1]]);
    format!(
        "PCI\\VEN_10DE&DEV_{:04X}&SUBSYS_{:04X}{:04X}&REV_{:02X}",
        device,
        u16::from_le_bytes([bytes[2], bytes[3]]),
        u16::from_le_bytes([bytes[4], bytes[5]]),
        bytes[6] & 0x0F,
    )
}

fn generate_mac_address(token: &[u8; 32]) -> String {
    let bytes = derive_bytes(token, b"nic");
    // Set locally administered bit (bit 1 of first octet) to avoid
    // colliding with real OUI-assigned addresses
    let first = (bytes[0] | 0x02) & 0xFE; // locally administered, unicast
    format!(
        "{:02X}-{:02X}-{:02X}-{:02X}-{:02X}-{:02X}",
        first, bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]
    )
}

fn generate_disk_serial(token: &[u8; 32]) -> String {
    let bytes = derive_bytes(token, b"disk");
    let hi = u16::from_le_bytes([bytes[0], bytes[1]]);
    let lo = u16::from_le_bytes([bytes[2], bytes[3]]);
    format!("{:04X}-{:04X}", hi, lo)
}

fn generate_hostname(token: &[u8; 32]) -> String {
    let bytes = derive_bytes(token, b"host");
    // Windows-style hostname: DESKTOP-XXXXXXX (7 alphanumeric chars)
    let charset = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let suffix: String = bytes[..7]
        .iter()
        .map(|&b| charset[(b as usize) % charset.len()] as char)
        .collect();
    format!("DESKTOP-{suffix}")
}

fn generate_firmware_tables(token: &[u8; 32]) -> SpoofedFirmwareTables {
    let mut rsmb_table = Vec::new();
    append_smbios_bios_info(&mut rsmb_table, token);
    append_smbios_system_info(&mut rsmb_table, token);
    append_smbios_baseboard_info(&mut rsmb_table, token);
    append_smbios_chassis_info(&mut rsmb_table, token);
    append_smbios_end_of_table(&mut rsmb_table);

    let mut rsmb = Vec::with_capacity(8 + rsmb_table.len());
    rsmb.push(0);
    rsmb.push(3);
    rsmb.push(2);
    rsmb.push(0);
    rsmb.extend_from_slice(&(rsmb_table.len() as u32).to_le_bytes());
    rsmb.extend_from_slice(&rsmb_table);

    let acpi = [
        (*b"FACP", b"acpi-facp".as_slice()),
        (*b"APIC", b"acpi-apic".as_slice()),
        (*b"HPET", b"acpi-hpet".as_slice()),
        (*b"MCFG", b"acpi-mcfg".as_slice()),
        (*b"DSDT", b"acpi-dsdt".as_slice()),
    ]
    .into_iter()
    .map(|(signature, domain)| SpoofedAcpiTable {
        id: fourcc(signature),
        data: generate_acpi_header(token, signature, domain),
    })
    .collect();

    SpoofedFirmwareTables { rsmb, acpi }
}

fn append_smbios_bios_info(out: &mut Vec<u8>, token: &[u8; 32]) {
    let bytes = derive_bytes(token, b"smbios-bios");
    let mut formatted = vec![0u8; 0x18];
    formatted[0] = 0;
    formatted[1] = 0x18;
    formatted[2..4].copy_from_slice(&0x0000_u16.to_le_bytes());
    formatted[4] = 1;
    formatted[5] = 2;
    formatted[8] = 3;
    formatted[9] = 0x20;
    formatted[0x12] = 0x03;
    formatted[0x13] = 0x0D;
    formatted[0x14] = 3;
    formatted[0x15] = 2;
    formatted[0x16] = 0xFF;
    formatted[0x17] = 0xFF;
    out.extend_from_slice(&formatted);
    append_smbios_strings(
        out,
        &[
            "American Megatrends Inc.",
            &format!(
                "1.{:02}.{:04}",
                bytes[0] % 20,
                u16::from_le_bytes([bytes[1], bytes[2]])
            ),
            "05/18/2023",
        ],
    );
}

fn append_smbios_system_info(out: &mut Vec<u8>, token: &[u8; 32]) {
    let uuid = generate_smbios_uuid(token);
    let serial = generate_serial(token, b"smbios-system-serial", "SYS");
    let sku = generate_serial(token, b"smbios-sku", "SKU");

    let mut formatted = vec![0u8; 0x1B];
    formatted[0] = 1;
    formatted[1] = 0x1B;
    formatted[2..4].copy_from_slice(&0x0100_u16.to_le_bytes());
    formatted[4] = 1;
    formatted[5] = 2;
    formatted[6] = 3;
    formatted[7] = 4;
    formatted[8..24].copy_from_slice(&uuid);
    formatted[24] = 0x06;
    formatted[25] = 5;
    formatted[26] = 6;
    out.extend_from_slice(&formatted);
    append_smbios_strings(
        out,
        &[
            "Dell Inc.",
            "OptiPlex 7070",
            "A00",
            &serial,
            &sku,
            "OptiPlex",
        ],
    );
}

fn append_smbios_baseboard_info(out: &mut Vec<u8>, token: &[u8; 32]) {
    let serial = generate_serial(token, b"smbios-board-serial", "BRD");
    let asset = generate_serial(token, b"smbios-board-asset", "AST");

    let mut formatted = vec![0u8; 0x0F];
    formatted[0] = 2;
    formatted[1] = 0x0F;
    formatted[2..4].copy_from_slice(&0x0200_u16.to_le_bytes());
    formatted[4] = 1;
    formatted[5] = 2;
    formatted[6] = 3;
    formatted[7] = 4;
    formatted[8] = 5;
    formatted[9] = 0x0A;
    formatted[10..12].copy_from_slice(&0x0300_u16.to_le_bytes());
    formatted[12] = 0;
    formatted[13] = 0;
    formatted[14] = 0x0A;
    out.extend_from_slice(&formatted);
    append_smbios_strings(out, &["Dell Inc.", "0T2HR0", "A02", &serial, &asset]);
}

fn append_smbios_chassis_info(out: &mut Vec<u8>, token: &[u8; 32]) {
    let serial = generate_serial(token, b"smbios-chassis-serial", "CHS");
    let asset = generate_serial(token, b"smbios-chassis-asset", "TAG");

    let mut formatted = vec![0u8; 0x16];
    formatted[0] = 3;
    formatted[1] = 0x16;
    formatted[2..4].copy_from_slice(&0x0300_u16.to_le_bytes());
    formatted[4] = 1;
    formatted[5] = 0x03;
    formatted[6] = 2;
    formatted[7] = 3;
    formatted[0x11] = 0x01;
    formatted[0x12] = 0x03;
    formatted[0x13] = 0x03;
    formatted[0x14] = 0;
    out.extend_from_slice(&formatted);
    append_smbios_strings(out, &["Dell Inc.", &serial, &asset]);
}

fn append_smbios_end_of_table(out: &mut Vec<u8>) {
    out.extend_from_slice(&[127, 4, 0x7F, 0x00, 0, 0]);
}

fn append_smbios_strings(out: &mut Vec<u8>, strings: &[&str]) {
    for value in strings {
        out.extend_from_slice(value.as_bytes());
        out.push(0);
    }
    out.push(0);
}

fn generate_serial(token: &[u8; 32], domain: &[u8], prefix: &str) -> String {
    let bytes = derive_bytes(token, domain);
    format!(
        "{prefix}{:02X}{:02X}{:02X}{:02X}{:02X}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4]
    )
}

fn generate_smbios_uuid(token: &[u8; 32]) -> [u8; 16] {
    let mut uuid = derive_bytes(token, b"smbios-uuid");
    uuid[6] = (uuid[6] & 0x0F) | 0x40;
    uuid[8] = (uuid[8] & 0x3F) | 0x80;
    uuid
}

fn generate_acpi_header(token: &[u8; 32], signature: [u8; 4], domain: &[u8]) -> Vec<u8> {
    let bytes = derive_bytes(token, domain);
    let mut table = Vec::with_capacity(36);
    table.extend_from_slice(&signature);
    table.extend_from_slice(&36_u32.to_le_bytes());
    table.push(2);
    table.push(0);
    table.extend_from_slice(b"DELL  ");
    table.extend_from_slice(&[
        b'T',
        b'Q',
        b'F',
        b'W',
        hex_nibble(bytes[0] >> 4),
        hex_nibble(bytes[0]),
        hex_nibble(bytes[1] >> 4),
        hex_nibble(bytes[1]),
    ]);
    table.extend_from_slice(
        &u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]).to_le_bytes(),
    );
    table.extend_from_slice(b"MSFT");
    table.extend_from_slice(&0x0001_0013_u32.to_le_bytes());

    let checksum = 0_u8.wrapping_sub(table.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte)));
    table[9] = checksum;
    table
}

fn hex_nibble(value: u8) -> u8 {
    b"0123456789ABCDEF"[(value & 0x0F) as usize]
}

fn firmware_table_bytes(
    firmware: &SpoofedFirmwareTables,
    provider: u32,
    table_id: u32,
) -> Option<&[u8]> {
    match provider {
        RSMB_PROVIDER if table_id == RSMB_TABLE_ID => Some(&firmware.rsmb),
        ACPI_PROVIDER => firmware
            .acpi
            .iter()
            .find(|table| table.id == table_id)
            .map(|table| table.data.as_slice()),
        _ => None,
    }
}

fn firmware_table_enum_bytes(firmware: &SpoofedFirmwareTables, provider: u32) -> Option<Vec<u8>> {
    match provider {
        RSMB_PROVIDER => Some(RSMB_TABLE_ID.to_le_bytes().to_vec()),
        ACPI_PROVIDER => {
            let mut bytes = Vec::with_capacity(firmware.acpi.len() * 4);
            for table in &firmware.acpi {
                bytes.extend_from_slice(&table.id.to_le_bytes());
            }
            Some(bytes)
        }
        _ => None,
    }
}

#[cfg(windows)]
mod inner {
    use core::ffi::c_void;
    use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

    use super::hwbp::{self, HwbpSlot};
    use windows::{
        Win32::System::Diagnostics::Debug::CONTEXT,
        Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress},
        core::s,
    };

    // SystemFingerprint signature from Ghidra.
    type FingerprintFn = unsafe extern "system" fn(*mut core::ffi::c_void);
    type GetSystemFirmwareTableFn = unsafe extern "system" fn(u32, u32, *mut c_void, u32) -> u32;
    type EnumSystemFirmwareTablesFn = unsafe extern "system" fn(u32, *mut c_void, u32) -> u32;

    const UNSET_SLOT: u8 = 0xFF;

    static SYSTEM_FINGERPRINT_SLOT: AtomicU8 = AtomicU8::new(UNSET_SLOT);
    static GET_SYSTEM_FIRMWARE_SLOT: AtomicU8 = AtomicU8::new(UNSET_SLOT);
    static ENUM_SYSTEM_FIRMWARE_SLOT: AtomicU8 = AtomicU8::new(UNSET_SLOT);
    static SYSTEM_FINGERPRINT_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
    static GET_SYSTEM_FIRMWARE_TABLE_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
    static ENUM_SYSTEM_FIRMWARE_TABLES_ORIGINAL: AtomicUsize = AtomicUsize::new(0);

    fn system_fingerprint_slot() -> Option<HwbpSlot> {
        HwbpSlot::from_index(SYSTEM_FINGERPRINT_SLOT.load(Ordering::Acquire) as usize)
    }

    fn get_system_firmware_slot() -> Option<HwbpSlot> {
        HwbpSlot::from_index(GET_SYSTEM_FIRMWARE_SLOT.load(Ordering::Acquire) as usize)
    }

    fn enum_system_firmware_slot() -> Option<HwbpSlot> {
        HwbpSlot::from_index(ENUM_SYSTEM_FIRMWARE_SLOT.load(Ordering::Acquire) as usize)
    }

    fn set_system_fingerprint_slot(slot: HwbpSlot) {
        SYSTEM_FINGERPRINT_SLOT.store(slot as u8, Ordering::Release);
    }

    fn set_get_system_firmware_slot(slot: HwbpSlot) {
        GET_SYSTEM_FIRMWARE_SLOT.store(slot as u8, Ordering::Release);
    }

    fn set_enum_system_firmware_slot(slot: HwbpSlot) {
        ENUM_SYSTEM_FIRMWARE_SLOT.store(slot as u8, Ordering::Release);
    }

    fn set_system_fingerprint_original(address: usize) {
        SYSTEM_FINGERPRINT_ORIGINAL.store(address, Ordering::Release);
    }

    fn set_get_system_firmware_original(address: usize) {
        GET_SYSTEM_FIRMWARE_TABLE_ORIGINAL.store(address, Ordering::Release);
    }

    fn set_enum_system_firmware_original(address: usize) {
        ENUM_SYSTEM_FIRMWARE_TABLES_ORIGINAL.store(address, Ordering::Release);
    }

    fn system_fingerprint_original() -> Option<FingerprintFn> {
        let address = SYSTEM_FINGERPRINT_ORIGINAL.load(Ordering::Acquire);
        if address == 0 {
            None
        } else {
            Some(unsafe { std::mem::transmute::<usize, FingerprintFn>(address) })
        }
    }

    fn get_system_firmware_table_original() -> Option<GetSystemFirmwareTableFn> {
        let address = GET_SYSTEM_FIRMWARE_TABLE_ORIGINAL.load(Ordering::Acquire);
        if address == 0 {
            None
        } else {
            Some(unsafe { std::mem::transmute::<usize, GetSystemFirmwareTableFn>(address) })
        }
    }

    fn enum_system_firmware_tables_original() -> Option<EnumSystemFirmwareTablesFn> {
        let address = ENUM_SYSTEM_FIRMWARE_TABLES_ORIGINAL.load(Ordering::Acquire);
        if address == 0 {
            None
        } else {
            Some(unsafe {
                std::mem::transmute::<usize, EnumSystemFirmwareTablesFn>(address)
            })
        }
    }

    fn clear_slot_state() {
        SYSTEM_FINGERPRINT_SLOT.store(UNSET_SLOT, Ordering::Release);
        GET_SYSTEM_FIRMWARE_SLOT.store(UNSET_SLOT, Ordering::Release);
        ENUM_SYSTEM_FIRMWARE_SLOT.store(UNSET_SLOT, Ordering::Release);
        SYSTEM_FINGERPRINT_ORIGINAL.store(0, Ordering::Release);
        GET_SYSTEM_FIRMWARE_TABLE_ORIGINAL.store(0, Ordering::Release);
        ENUM_SYSTEM_FIRMWARE_TABLES_ORIGINAL.store(0, Ordering::Release);
    }

    fn jump_to_return(context: &mut CONTEXT) {
        if context.Rsp != 0 {
            let return_addr = unsafe { *(context.Rsp as *const usize) };
            context.Rip = return_addr as u64;
            // Emulate a real `ret`: pop the return address from the stack.
            context.Rsp = context.Rsp.saturating_add(std::mem::size_of::<usize>() as u64);
        }
    }

    /// The callback — replaces the fingerprint payload with spoofed values.
    ///
    /// Strategy: Let the original function run to populate the struct, then
    /// overwrite the four fields with our spoofed values. This is safer than
    /// skipping the original entirely, as it preserves any other side effects
    /// or state the function may set.
    #[cfg_attr(windows, unsafe(link_section = ".tq"))]
    fn fingerprint_callback(exception_info: *mut ()) -> bool {
        let context = unsafe {
            let exception_info = &mut *(exception_info
                as *mut windows::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS);
            &mut *exception_info.ContextRecord
        };
        let this = context.Rcx as *mut c_void;

        if let Some(slot) = system_fingerprint_slot() {
            if let Some(original) = system_fingerprint_original() {
                if let Err(error) = hwbp::disable_current_thread_breakpoint(slot) {
                    tracing::warn!(
                        "SystemFingerprint hook failed to disable current-thread HWBP: {}",
                        error
                    );
                } else {
                    unsafe {
                        original(this);
                    }
                    if let Err(error) = hwbp::enable_current_thread_breakpoint(slot) {
                        tracing::warn!(
                            "SystemFingerprint hook failed to re-enable current-thread HWBP: {}",
                            error
                        );
                    }
                }
            }
        }

        if let Some(fp) = super::spoofed() {
            if !this.is_null() {
                unsafe {
                    // The struct layout isn't fully reversed, so we use the CXStr
                    // write helper to set string fields at known offsets.
                    // These offsets are from Ghidra analysis of SystemFingerprint:
                    //   this+0x00: vtable
                    //   this+0x08: VideoCardId (CXStr, 0x10 bytes inline)
                    //   this+0x18: NetworkCardId (CXStr)
                    //   this+0x28: HardriveId (CXStr)
                    //   this+0x38: ComputerName (CXStr)
                    write_cxstr(this, 0x08, &fp.video_card_id);
                    write_cxstr(this, 0x18, &fp.network_card_id);
                    write_cxstr(this, 0x28, &fp.hard_drive_id);
                    write_cxstr(this, 0x38, &fp.computer_name);
                }
            }
            tracing::trace!("Fingerprint spoofed successfully");
        }

        jump_to_return(context);
        true
    }

    #[cfg_attr(windows, unsafe(link_section = ".tq"))]
    fn get_system_firmware_table_callback(
        exception_info: *mut (),
    ) -> bool {
        let context = unsafe {
            let exception_info = &mut *(exception_info
                as *mut windows::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS);
            &mut *exception_info.ContextRecord
        };
        let provider = context.Rcx as u32;
        let table_id = context.Rdx as u32;
        let buffer = context.R8 as *mut c_void;
        let buffer_size = context.R9 as u32;

        let result = if let Some(firmware) = super::spoofed_firmware() {
            super::firmware_table_bytes(firmware, provider, table_id)
                .map(|table| copy_firmware_response(table, buffer, buffer_size))
                .unwrap_or(0)
        } else if let Some(slot) = get_system_firmware_slot() {
            if let Some(original) = get_system_firmware_table_original() {
                if let Err(error) = hwbp::disable_current_thread_breakpoint(slot) {
                    tracing::warn!(
                        "GetSystemFirmwareTable hook failed to disable current-thread HWBP: {}",
                        error
                    );
                    0
                } else {
                    let returned = unsafe { original(provider, table_id, buffer, buffer_size) };
                    if let Err(error) = hwbp::enable_current_thread_breakpoint(slot) {
                        tracing::warn!(
                            "GetSystemFirmwareTable hook failed to re-enable current-thread \
                             HWBP: {}",
                            error
                        );
                    }
                    returned
                }
            } else {
                0
            }
        } else {
            0
        };

        context.Rax = result as u64;
        jump_to_return(context);
        true
    }

    #[cfg_attr(windows, unsafe(link_section = ".tq"))]
    fn enum_system_firmware_tables_callback(
        exception_info: *mut (),
    ) -> bool {
        let context = unsafe {
            let exception_info = &mut *(exception_info
                as *mut windows::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS);
            &mut *exception_info.ContextRecord
        };
        let provider = context.Rcx as u32;
        let buffer = context.Rdx as *mut c_void;
        let buffer_size = context.R8 as u32;

        let result = if let Some(firmware) = super::spoofed_firmware() {
            super::firmware_table_enum_bytes(firmware, provider)
                .map(|table_ids| copy_firmware_response(&table_ids, buffer, buffer_size))
                .unwrap_or(0)
        } else if let Some(slot) = enum_system_firmware_slot() {
            if let Some(original) = enum_system_firmware_tables_original() {
                if let Err(error) = hwbp::disable_current_thread_breakpoint(slot) {
                    tracing::warn!(
                        "EnumSystemFirmwareTables hook failed to disable current-thread HWBP: {}",
                        error
                    );
                    0
                } else {
                    let returned = unsafe { original(provider, buffer, buffer_size) };
                    if let Err(error) = hwbp::enable_current_thread_breakpoint(slot) {
                        tracing::warn!(
                            "EnumSystemFirmwareTables hook failed to re-enable current-thread \
                             HWBP: {}",
                            error
                        );
                    }
                    returned
                }
            } else {
                0
            }
        } else {
            0
        };

        context.Rax = result as u64;
        jump_to_return(context);
        true
    }

    fn copy_firmware_response(data: &[u8], buffer: *mut c_void, buffer_size: u32) -> u32 {
        let required = data.len() as u32;
        if buffer.is_null() || buffer_size < required {
            return required;
        }

        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), buffer.cast::<u8>(), data.len());
        }
        required
    }

    /// Write a Rust string into a CXStr at the given offset from `base`.
    ///
    /// CXStr layout (from textquest-common/src/offsets.rs):
    ///   CXStr = single pointer to CStrRep (8 bytes)
    ///   CStrRep+0x04: alloc (u32, capacity)
    ///   CStrRep+0x08: length (u32)
    ///   CStrRep+0x18: data (char[])
    ///
    /// # Safety
    /// `base` must point to a valid struct with a CXStr at `offset`.
    /// The CStrRep buffer must have sufficient capacity for `value`.
    unsafe fn write_cxstr(base: *mut core::ffi::c_void, offset: usize, value: &str) {
        use textquest_common::offsets::eqmain::{CSTRREP_ALLOC, CSTRREP_DATA, CSTRREP_LENGTH};

        let field_ptr = unsafe { (base as *mut u8).add(offset) };
        // CXStr is a pointer to CStrRep
        let rep_ptr = unsafe { *(field_ptr as *const *mut u8) };

        if rep_ptr.is_null() {
            return;
        }

        // Check CStrRep capacity before writing
        let capacity = unsafe { *(rep_ptr.add(CSTRREP_ALLOC) as *const u32) } as usize;
        let len = value.len().min(capacity.saturating_sub(1)); // leave room for null
        if len == 0 {
            return;
        }

        unsafe {
            let data_ptr = rep_ptr.add(CSTRREP_DATA);
            core::ptr::copy_nonoverlapping(value.as_ptr(), data_ptr, len);
            *data_ptr.add(len) = 0; // null terminate
            // Update length field in CStrRep
            *(rep_ptr.add(CSTRREP_LENGTH) as *mut u32) = len as u32;
        }
    }

    /// Install the fingerprint hook.
    pub fn install(fingerprint_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
        if fingerprint_addr == 0 {
            return Err("SystemFingerprint address is zero".into());
        }

        let target: FingerprintFn = unsafe { std::mem::transmute(fingerprint_addr) };
        let _ = target;

        let slot = match hwbp::register_available(fingerprint_addr, fingerprint_callback)? {
            hwbp::HwbpInstallOutcome::Installed(slot) => slot,
            hwbp::HwbpInstallOutcome::FallbackToDetour => {
                return Err(
                    "No HWBP slots available for SystemFingerprint hook; cannot install without JMP hooks"
                        .into(),
                );
            }
        };

        set_system_fingerprint_slot(slot);
        set_system_fingerprint_original(fingerprint_addr);
        tracing::info!(
            slot = slot as u8,
            addr = format!("{:#x}", fingerprint_addr),
            "Fingerprint spoof hook installed"
        );
        Ok(())
    }

    pub fn install_firmware_hooks() -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            let kernel32 = GetModuleHandleA(s!("kernel32.dll"))?;
            let get_addr = GetProcAddress(kernel32, s!("GetSystemFirmwareTable"))
                .ok_or("GetSystemFirmwareTable not exported by kernel32.dll")?;
            let enum_addr = GetProcAddress(kernel32, s!("EnumSystemFirmwareTables"))
                .ok_or("EnumSystemFirmwareTables not exported by kernel32.dll")?;

            let get_slot = match hwbp::register_available(
                get_addr as usize,
                get_system_firmware_table_callback,
            )? {
                hwbp::HwbpInstallOutcome::Installed(slot) => slot,
                hwbp::HwbpInstallOutcome::FallbackToDetour => {
                    return Err(
                        "No HWBP slots available for firmware hooks; cannot install without JMP hooks"
                            .into(),
                    );
                }
            };
            set_get_system_firmware_slot(get_slot);
            set_get_system_firmware_original(get_addr as usize);

            let enum_slot = match hwbp::register_available(
                enum_addr as usize,
                enum_system_firmware_tables_callback,
            )? {
                hwbp::HwbpInstallOutcome::Installed(slot) => slot,
                hwbp::HwbpInstallOutcome::FallbackToDetour => {
                    if let Err(unregister_error) = hwbp::unregister(get_slot) {
                        tracing::warn!(
                            "Failed to rollback GetSystemFirmwareTable during enum hook setup: {}",
                            unregister_error
                        );
                    }
                    clear_slot_state();
                    return Err(
                        "No HWBP slots available for firmware hooks; cannot install without JMP hooks"
                            .into(),
                    );
                }
            };
            set_enum_system_firmware_slot(enum_slot);
            set_enum_system_firmware_original(enum_addr as usize);
        }

        tracing::info!(
            get_slot = GET_SYSTEM_FIRMWARE_SLOT.load(Ordering::Acquire),
            enum_slot = ENUM_SYSTEM_FIRMWARE_SLOT.load(Ordering::Acquire),
            "Firmware table fingerprint HWBP hooks installed"
        );
        Ok(())
    }

    /// Remove the fingerprint hooks.
    pub fn remove() {
        if let Some(slot) = system_fingerprint_slot() {
            if hwbp::is_active(slot) {
                if let Err(e) = hwbp::unregister(slot) {
                    tracing::warn!("Failed to remove SystemFingerprint HWBP: {}", e);
                }
            }
        }
        if let Some(slot) = get_system_firmware_slot() {
            if hwbp::is_active(slot) {
                if let Err(e) = hwbp::unregister(slot) {
                    tracing::warn!("Failed to remove GetSystemFirmwareTable HWBP: {}", e);
                }
            }
        }
        if let Some(slot) = enum_system_firmware_slot() {
            if hwbp::is_active(slot) {
                if let Err(e) = hwbp::unregister(slot) {
                    tracing::warn!(
                        "Failed to remove EnumSystemFirmwareTables HWBP: {}",
                        e
                    );
                }
            }
        }
        clear_slot_state();
        tracing::info!("Fingerprint spoof HWBP hooks removed");
    }
}

#[cfg(not(windows))]
mod inner {
    pub fn install(_fingerprint_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
        tracing::warn!("Fingerprint spoof hook not available on this platform (stub)");
        Ok(())
    }

    pub fn install_firmware_hooks() -> Result<(), Box<dyn std::error::Error>> {
        tracing::warn!("Firmware table hooks not available on this platform (stub)");
        Ok(())
    }

    pub fn remove() {
        tracing::warn!("Fingerprint spoof hook removal not available (stub)");
    }
}

#[allow(unused_imports)]
pub use inner::{install, install_firmware_hooks, remove};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_fingerprint_is_deterministic() {
        let token = [0x42u8; 32];
        let fp1 = generate_fingerprint(&token);
        let fp2 = generate_fingerprint(&token);
        assert_eq!(fp1.video_card_id, fp2.video_card_id);
        assert_eq!(fp1.network_card_id, fp2.network_card_id);
        assert_eq!(fp1.hard_drive_id, fp2.hard_drive_id);
        assert_eq!(fp1.computer_name, fp2.computer_name);
    }

    #[test]
    fn different_tokens_produce_different_fingerprints() {
        let token_a = [0x01u8; 32];
        let token_b = [0x02u8; 32];
        let fp_a = generate_fingerprint(&token_a);
        let fp_b = generate_fingerprint(&token_b);
        assert_ne!(fp_a.video_card_id, fp_b.video_card_id);
        assert_ne!(fp_a.network_card_id, fp_b.network_card_id);
        assert_ne!(fp_a.hard_drive_id, fp_b.hard_drive_id);
        assert_ne!(fp_a.computer_name, fp_b.computer_name);
    }

    #[test]
    fn gpu_id_has_valid_format() {
        let token = [0xABu8; 32];
        let fp = generate_fingerprint(&token);
        assert!(fp.video_card_id.starts_with("PCI\\VEN_10DE&DEV_"));
        assert!(fp.video_card_id.contains("&SUBSYS_"));
        assert!(fp.video_card_id.contains("&REV_"));
    }

    #[test]
    fn mac_address_is_locally_administered() {
        let token = [0xCDu8; 32];
        let fp = generate_fingerprint(&token);
        let parts: Vec<&str> = fp.network_card_id.split('-').collect();
        assert_eq!(parts.len(), 6);
        let first_octet = u8::from_str_radix(parts[0], 16).unwrap();
        // Locally administered bit set, unicast
        assert_eq!(first_octet & 0x02, 0x02);
        assert_eq!(first_octet & 0x01, 0x00);
    }

    #[test]
    fn disk_serial_format() {
        let token = [0xEFu8; 32];
        let fp = generate_fingerprint(&token);
        assert_eq!(fp.hard_drive_id.len(), 9); // "XXXX-XXXX"
        assert_eq!(fp.hard_drive_id.chars().nth(4), Some('-'));
    }

    #[test]
    fn hostname_format() {
        let token = [0x99u8; 32];
        let fp = generate_fingerprint(&token);
        assert!(fp.computer_name.starts_with("DESKTOP-"));
        assert_eq!(fp.computer_name.len(), 15); // "DESKTOP-" + 7 chars
        // All chars after prefix are alphanumeric
        assert!(
            fp.computer_name[8..]
                .chars()
                .all(|c| c.is_ascii_alphanumeric())
        );
    }

    #[test]
    fn init_sets_spoofed_values() {
        // Note: OnceLock means this can only succeed once per process.
        // If another test called init first, this is still valid —
        // we just verify spoofed() returns Some.
        let token = [0x55u8; 32];
        init(&token);
        assert!(spoofed().is_some());
    }

    #[test]
    fn derive_bytes_domain_separation() {
        let token = [0x77u8; 32];
        let a = derive_bytes(&token, b"gpu");
        let b = derive_bytes(&token, b"nic");
        assert_ne!(a, b);
    }

    #[test]
    fn firmware_tables_are_deterministic_distinct_and_vm_free() {
        let token_a = [0x11u8; 32];
        let token_b = [0x22u8; 32];

        let a1 = generate_firmware_tables(&token_a);
        let a2 = generate_firmware_tables(&token_a);
        let b = generate_firmware_tables(&token_b);

        assert_eq!(a1.rsmb, a2.rsmb);
        assert_ne!(a1.rsmb, b.rsmb);

        let upper = String::from_utf8_lossy(&a1.rsmb).to_ascii_uppercase();
        for forbidden in ["VBOX", "VIRTUALBOX", "VMWARE", "QEMU", "BOCHS", "XEN"] {
            assert!(
                !upper.contains(forbidden),
                "spoofed SMBIOS data contained {forbidden}"
            );
        }
    }

    #[test]
    fn stub_install_remove_are_safe() {
        #[cfg(not(windows))]
        {
            assert!(install(0x12345).is_ok());
            remove();
        }
    }
}
