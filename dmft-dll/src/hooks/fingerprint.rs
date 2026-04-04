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

/// Per-client spoofed fingerprint values, generated once from the session token.
static SPOOFED: OnceLock<SpoofedFingerprint> = OnceLock::new();

/// The four hardware fingerprint fields that EQ's SystemFingerprint sends.
#[derive(Debug, Clone)]
pub struct SpoofedFingerprint {
    pub video_card_id: String,
    pub network_card_id: String,
    pub hard_drive_id: String,
    pub computer_name: String,
}

/// Initialize spoofed fingerprint values from the session token.
///
/// Must be called before the fingerprint hook fires (i.e., during DLL init).
/// Uses domain-separated hashing to derive plausible-looking unique values.
pub fn init(session_token: &[u8; 32]) {
    let fp = generate_fingerprint(session_token);
    tracing::info!(
        video = %fp.video_card_id,
        nic = %fp.network_card_id,
        disk = %fp.hard_drive_id,
        host = %fp.computer_name,
        "Spoofed fingerprint initialized"
    );
    let _ = SPOOFED.set(fp);
}

/// Get the spoofed fingerprint (None if init hasn't been called).
pub fn spoofed() -> Option<&'static SpoofedFingerprint> {
    SPOOFED.get()
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

#[cfg(windows)]
mod inner {
    use retour::static_detour;

    // SystemFingerprint signature from Ghidra.
    // void SystemFingerprint(void* this) — thiscall on x64.
    type FingerprintFn = unsafe extern "system" fn(*mut core::ffi::c_void);

    static_detour! {
        static FingerprintHook: unsafe extern "system" fn(*mut core::ffi::c_void);
    }

    /// The detour — replaces the fingerprint payload with spoofed values.
    ///
    /// Strategy: Let the original function run to populate the struct, then
    /// overwrite the four fields with our spoofed values. This is safer than
    /// skipping the original entirely, as it preserves any other side effects
    /// or state the function may set.
    fn fingerprint_detour(this: *mut core::ffi::c_void) {
        // Call original to let it initialize everything normally.
        // SAFETY: `this` is the same pointer EQ passed. The original function
        // was saved by retour during hook installation.
        unsafe {
            FingerprintHook.call(this);
        }

        // Now overwrite the fingerprint fields if we have spoofed values.
        if let Some(fp) = super::spoofed() {
            // The struct layout isn't fully reversed, so we use the CXStr
            // write helper to set string fields at known offsets.
            // These offsets are from Ghidra analysis of SystemFingerprint:
            //   this+0x00: vtable
            //   this+0x08: VideoCardId (CXStr, 0x10 bytes inline)
            //   this+0x18: NetworkCardId (CXStr)
            //   this+0x28: HardriveId (CXStr)
            //   this+0x38: ComputerName (CXStr)
            //
            // SAFETY: We're writing to the same struct the original function
            // just populated. The offsets are from Ghidra RE of the function
            // at SYSTEM_FINGERPRINT. If offsets are wrong, the server gets
            // garbage — but EQ won't crash (string writes are bounded).
            unsafe {
                write_cxstr(this, 0x08, &fp.video_card_id);
                write_cxstr(this, 0x18, &fp.network_card_id);
                write_cxstr(this, 0x28, &fp.hard_drive_id);
                write_cxstr(this, 0x38, &fp.computer_name);
            }
            tracing::trace!("Fingerprint spoofed successfully");
        } else {
            tracing::warn!(
                "Fingerprint hook fired but no spoofed values — sending real fingerprint"
            );
        }
    }

    /// Write a Rust string into a CXStr at the given offset from `base`.
    ///
    /// CXStr layout (from dmft-common/src/offsets.rs):
    ///   CXStr = single pointer to CStrRep (8 bytes)
    ///   CStrRep+0x04: alloc (u32, capacity)
    ///   CStrRep+0x08: length (u32)
    ///   CStrRep+0x18: data (char[])
    ///
    /// # Safety
    /// `base` must point to a valid struct with a CXStr at `offset`.
    /// The CStrRep buffer must have sufficient capacity for `value`.
    unsafe fn write_cxstr(base: *mut core::ffi::c_void, offset: usize, value: &str) {
        use dmft_common::offsets::eqmain::{CSTRREP_ALLOC, CSTRREP_DATA, CSTRREP_LENGTH};

        let field_ptr = (base as *mut u8).add(offset);
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
        // SAFETY: fingerprint_addr was rebased from SYSTEM_FINGERPRINT offset
        // against the live eqgame.exe base address. The transmute converts it
        // to a function pointer matching SystemFingerprint's calling convention.
        unsafe {
            let target: FingerprintFn = std::mem::transmute(fingerprint_addr);
            FingerprintHook.initialize(target, fingerprint_detour)?;
            FingerprintHook.enable()?;
        }
        tracing::info!(
            addr = format!("{:#x}", fingerprint_addr),
            "Fingerprint spoof hook installed"
        );
        Ok(())
    }

    /// Remove the fingerprint hook.
    pub fn remove() {
        // SAFETY: Disabling a retour hook restores the original function bytes.
        unsafe {
            if FingerprintHook.is_enabled() {
                let _ = FingerprintHook.disable();
            }
        }
        tracing::info!("Fingerprint spoof hook removed");
    }
}

#[cfg(not(windows))]
mod inner {
    pub fn install(_fingerprint_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
        tracing::warn!("Fingerprint spoof hook not available on this platform (stub)");
        Ok(())
    }

    pub fn remove() {
        tracing::warn!("Fingerprint spoof hook removal not available (stub)");
    }
}

#[allow(unused_imports)]
pub use inner::{install, remove};

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
    fn stub_install_remove_are_safe() {
        #[cfg(not(windows))]
        {
            assert!(install(0x12345).is_ok());
            remove();
        }
    }
}
