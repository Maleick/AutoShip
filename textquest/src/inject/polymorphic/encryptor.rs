//! Sub-issue #736 — per-injection encryption with unique keys.
//!
//! Encrypts the DLL payload with AES-256-GCM under a freshly generated
//! 32-byte key + 12-byte nonce. The key/nonce are emitted into the loader
//! stub as immediates by [`super::stub_gen`] and zeroized after use by the
//! self-erase routine in [`super::self_erase`].
//!
//! ## Status
//! Real AES-256-GCM encryption — this is the functional foundation. The
//! decryptor stub on the target side is part of #735.

use super::PolymorphicError;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};

/// Encrypt `dll_bytes` with AES-256-GCM.
pub fn encrypt_payload(
    dll_bytes: &[u8],
    key: &[u8; 32],
    nonce: &[u8; 12],
) -> Result<Vec<u8>, PolymorphicError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .encrypt(Nonce::from_slice(nonce), dll_bytes)
        .map_err(|e| PolymorphicError::Encrypt(e.to_string()))
}

/// Decrypt for round-trip testing. The on-target decryptor is emitted as
/// shellcode in #735; this helper is for host-side verification only.
pub fn decrypt_payload(
    ciphertext: &[u8],
    key: &[u8; 32],
    nonce: &[u8; 12],
) -> Result<Vec<u8>, PolymorphicError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|e| PolymorphicError::Encrypt(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_recovers_payload() {
        let dll = b"MZ\x90\x00fake-pe-payload-bytes";
        let key = [7u8; 32];
        let nonce = [3u8; 12];
        let ct = encrypt_payload(dll, &key, &nonce).unwrap();
        assert_ne!(&ct[..], &dll[..], "ciphertext must differ from plaintext");
        let pt = decrypt_payload(&ct, &key, &nonce).unwrap();
        assert_eq!(&pt[..], &dll[..]);
    }

    #[test]
    fn different_nonce_produces_different_ciphertext() {
        let dll = b"same-plaintext";
        let key = [9u8; 32];
        let n1 = [1u8; 12];
        let n2 = [2u8; 12];
        let a = encrypt_payload(dll, &key, &n1).unwrap();
        let b = encrypt_payload(dll, &key, &n2).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn wrong_key_fails_authentication() {
        let dll = b"sensitive";
        let key = [4u8; 32];
        let nonce = [5u8; 12];
        let ct = encrypt_payload(dll, &key, &nonce).unwrap();
        let bad_key = [0u8; 32];
        assert!(decrypt_payload(&ct, &bad_key, &nonce).is_err());
    }
}
