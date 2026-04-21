//! Zone-entry integrity reporter hook — intercepts `FUN_1402827C0` (opcode `0xe4b3`).
//!
//! On every zone connect EQ hashes three data regions:
//!   1. Player name (32 bytes)
//!   2. Spell data
//!   3. UI string data
//!
//! The resulting report is sent to the server via opcode `0xe4b3`. If any of our
//! hooks or data-layer modifications touch spell data or UI strings the hash will
//! drift and the server will detect us.
//!
//! This module installs a `retour` byte-patch detour (not HWBP — DR3 is claimed
//! by the memcheck responder in `hooks::memcheck`) that:
//!
//! 1. Captures the three hashed region specs at call time and logs them in debug
//!    builds via `tracing::debug!`.
//! 2. Passes through to the original function unchanged by default.
//! 3. When [`SPOOF_ENABLED`] is set, substitutes cached clean hashes for any
//!    region we have modified (future use — no regions currently modified).
//!
//! # Usage
//!
//! ```text
//! // During DLL init, after rebasing offsets:
//! zone_entry_integrity::install(rebase(offsets::ZONE_ENTRY_INTEGRITY))?;
//!
//! // Optionally enable hash substitution:
//! zone_entry_integrity::set_spoof_enabled(true);
//!
//! // Shutdown:
//! zone_entry_integrity::remove();
//! ```
//!
//! # Evidence basis
//!
//! `docs/wiki/Research-Anti-Detection.md` §Ghidra-Verified Findings §Zone entry integrity.
//! Issue: #2178. Parent: #2173.

#![allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]

use std::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// Config flag
// ---------------------------------------------------------------------------

/// When `true`, the hook substitutes cached clean hashes for any modified
/// region instead of passing through the live (potentially drifted) hashes.
///
/// Disabled by default — currently a no-op because no regions are modified.
/// Flip via [`set_spoof_enabled`] when a future feature modifies spell data
/// or UI strings.
pub static SPOOF_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enable or disable hash substitution in the zone-entry integrity hook.
pub fn set_spoof_enabled(enabled: bool) {
    SPOOF_ENABLED.store(enabled, Ordering::Release);
}

/// Returns `true` if hash substitution is currently enabled.
pub fn is_spoof_enabled() -> bool {
    SPOOF_ENABLED.load(Ordering::Acquire)
}

// ---------------------------------------------------------------------------
// Telemetry — captured region specs
// ---------------------------------------------------------------------------

/// Describes one of the three hashed data regions captured at hook invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionSpec {
    /// Label used for logging / telemetry.
    pub label: &'static str,
    /// Address of the region start (as captured from the call frame).
    pub address: usize,
    /// Length of the hashed region in bytes.
    pub length: usize,
}

/// The three region specs captured on the most-recent zone-entry integrity
/// report. Protected by a `Mutex` so tests can read them without data races.
static LAST_REGIONS: std::sync::Mutex<Option<[RegionSpec; 3]>> = std::sync::Mutex::new(None);

/// Return a snapshot of the three region specs captured during the last hook
/// invocation, or `None` if the hook has not yet fired.
pub fn last_regions() -> Option<[RegionSpec; 3]> {
    LAST_REGIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

// ---------------------------------------------------------------------------
// Platform-specific hook (Windows: retour detour; non-Windows: stub)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod inner {
    use super::*;
    use retour::static_detour;

    // `FUN_1402827C0` signature inferred from Ghidra: called as a thiscall-
    // style void function. We treat the context pointer as opaque `*mut c_void`
    // until the struct layout is confirmed (tracked in #2178).
    type ZoneEntryIntegrityFn = unsafe extern "system" fn(*mut core::ffi::c_void);

    static_detour! {
        static ZoneEntryIntegrityHook: unsafe extern "system" fn(*mut core::ffi::c_void);
    }

    /// Detour for `FUN_1402827C0`.
    ///
    /// Strategy:
    /// 1. Read the three region specs from the context pointer at the Ghidra-
    ///    derived offsets (placeholders below — update when RE is complete).
    /// 2. Log the specs in debug builds.
    /// 3. If [`SPOOF_ENABLED`] is set, substitute clean hashes (future).
    /// 4. Always call the original function — we only observe, never skip.
    fn zone_entry_integrity_detour(this: *mut core::ffi::c_void) {
        // ---------------------------------------------------------------
        // Step 1: capture region specs
        //
        // Ghidra RE for #2178 identified three hashed regions in the context
        // struct. Offsets are provisional — the struct layout is not yet fully
        // confirmed. We use a safe guard: if `this` is null we skip capture.
        // ---------------------------------------------------------------
        let regions_opt = if this.is_null() {
            None
        } else {
            // SAFETY: `this` is non-null and was passed to us by EQ — it is
            // a valid pointer for the duration of this call. Reads are
            // bounded to known struct offsets. If offsets are wrong the
            // worst case is reading garbage values that get logged, not a
            // crash (the function itself would crash before us if `this`
            // were bad).
            //
            // Struct layout (provisional, from Ghidra analysis in #2178):
            //   this+0x00: vtable
            //   this+0x08: player_name_ptr (*const u8, 32 bytes)
            //   this+0x10: spell_data_ptr  (*const u8)
            //   this+0x14: spell_data_len  (u32)
            //   this+0x18: ui_str_ptr      (*const u8)
            //   this+0x1c: ui_str_len      (u32)
            unsafe {
                let base = this as *const u8;
                let player_name_addr = *(base.add(0x08) as *const usize);
                let spell_data_addr = *(base.add(0x10) as *const usize);
                let spell_data_len = *(base.add(0x14) as *const u32) as usize;
                let ui_str_addr = *(base.add(0x18) as *const usize);
                let ui_str_len = *(base.add(0x1c) as *const u32) as usize;

                Some([
                    RegionSpec {
                        label: "player_name",
                        address: player_name_addr,
                        length: 32,
                    },
                    RegionSpec {
                        label: "spell_data",
                        address: spell_data_addr,
                        length: spell_data_len,
                    },
                    RegionSpec {
                        label: "ui_strings",
                        address: ui_str_addr,
                        length: ui_str_len,
                    },
                ])
            }
        };

        // ---------------------------------------------------------------
        // Step 2: log in debug builds, store for telemetry
        // ---------------------------------------------------------------
        if let Some(ref regions) = regions_opt {
            tracing::debug!(
                player_name_addr = format!("{:#x}", regions[0].address),
                player_name_len = regions[0].length,
                spell_data_addr = format!("{:#x}", regions[1].address),
                spell_data_len = regions[1].length,
                ui_str_addr = format!("{:#x}", regions[2].address),
                ui_str_len = regions[2].length,
                "Zone-entry integrity report firing (opcode 0xe4b3)"
            );

            *LAST_REGIONS
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(regions.clone());
        } else {
            tracing::warn!("Zone-entry integrity hook fired with null context pointer");
        }

        // ---------------------------------------------------------------
        // Step 3: optional hash substitution (future — currently no-op)
        // ---------------------------------------------------------------
        if super::is_spoof_enabled() {
            // Future: for each region in `regions_opt` that overlaps a
            // modified block, temporarily restore clean bytes, let the
            // original hash them, then re-apply our patches. Same pattern
            // as `hooks::memcheck`. Not implemented until we actually modify
            // spell data or UI strings.
            tracing::debug!(
                "SPOOF_ENABLED but no modified regions registered — passthrough unchanged"
            );
        }

        // ---------------------------------------------------------------
        // Step 4: always call original — we only observe
        // ---------------------------------------------------------------
        // SAFETY: `this` is the same pointer EQ passed us. The original
        // function pointer is managed by retour and was captured before the
        // detour was installed.
        unsafe {
            ZoneEntryIntegrityHook.call(this);
        }
    }

    /// Install the zone-entry integrity hook.
    ///
    /// `handler_addr` must be the rebased address of `FUN_1402827C0`.
    pub fn install(handler_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
        // SAFETY: handler_addr is the rebased address of the target function.
        // The transmute converts it to a typed function pointer matching the
        // calling convention. retour saves original bytes and restores them on
        // `disable()`.
        unsafe {
            let target: ZoneEntryIntegrityFn = std::mem::transmute(handler_addr);
            ZoneEntryIntegrityHook.initialize(target, zone_entry_integrity_detour)?;
            ZoneEntryIntegrityHook.enable()?;
        }
        tracing::info!(
            addr = format!("{:#x}", handler_addr),
            "Zone-entry integrity hook installed (opcode 0xe4b3)"
        );
        Ok(())
    }

    /// Remove the zone-entry integrity hook, restoring original function bytes.
    pub fn remove() {
        // SAFETY: Disabling a retour hook restores the original function bytes.
        unsafe {
            if ZoneEntryIntegrityHook.is_enabled() {
                let _ = ZoneEntryIntegrityHook.disable();
            }
        }
        tracing::info!("Zone-entry integrity hook removed");
    }
}

#[cfg(not(windows))]
mod inner {
    pub fn install(_handler_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
        tracing::warn!("Zone-entry integrity hook not available on this platform (stub)");
        Ok(())
    }

    pub fn remove() {
        // no-op on non-Windows
    }
}

#[allow(unused_imports)]
pub use inner::{install, remove};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spoof_flag_default_disabled() {
        // Default state: passthrough only, no substitution.
        assert!(!is_spoof_enabled(), "spoof should default to disabled");
    }

    #[test]
    fn spoof_flag_roundtrip() {
        let original = is_spoof_enabled();

        set_spoof_enabled(true);
        assert!(is_spoof_enabled());

        set_spoof_enabled(false);
        assert!(!is_spoof_enabled());

        // Restore original state for test isolation.
        set_spoof_enabled(original);
    }

    #[test]
    fn last_regions_initially_none() {
        // Before the hook fires, there is no captured region snapshot.
        // Note: if another test triggered the hook this may already be Some.
        // We only assert the API doesn't panic.
        let _ = last_regions(); // must not panic
    }

    #[test]
    fn region_spec_debug_format() {
        let spec = RegionSpec {
            label: "player_name",
            address: 0x1234_5678,
            length: 32,
        };
        let s = format!("{spec:?}");
        // Debug derives print field names and values; address is printed as decimal.
        assert!(
            s.contains("player_name"),
            "label should appear in debug: {s}"
        );
        assert!(
            s.contains("length"),
            "length field should appear in debug: {s}"
        );
        assert!(s.contains("32"), "length value should appear in debug: {s}");
    }

    #[test]
    fn region_spec_equality() {
        let a = RegionSpec {
            label: "spell_data",
            address: 0xDEAD,
            length: 256,
        };
        let b = RegionSpec {
            label: "spell_data",
            address: 0xDEAD,
            length: 256,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn region_spec_inequality_on_address() {
        let a = RegionSpec {
            label: "ui_strings",
            address: 0x1000,
            length: 64,
        };
        let b = RegionSpec {
            label: "ui_strings",
            address: 0x2000,
            length: 64,
        };
        assert_ne!(a, b);
    }

    /// Verify the stub install/remove pair is safe on non-Windows.
    #[cfg(not(windows))]
    #[test]
    fn stub_install_remove_are_safe() {
        assert!(install(0x1402827c0).is_ok());
        remove();
    }

    /// Observe-only: hook fires once per zone transition.
    ///
    /// This test is advisory on non-Windows (the stub always succeeds).
    /// On Windows + live EQ it would fire once per zone connect.
    #[test]
    fn install_returns_ok_on_stub_platform() {
        #[cfg(not(windows))]
        {
            let result = install(0x1402827c0);
            assert!(
                result.is_ok(),
                "stub install should always succeed: {result:?}"
            );
            remove();
        }
    }
}
