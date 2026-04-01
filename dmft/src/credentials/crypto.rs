use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, OsRng},
};
use anyhow::Result;
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use zeroize::Zeroizing;

fn argon2_instance() -> Result<Argon2<'static>> {
    let params = Params::new(65536, 3, 4, Some(32))
        .map_err(|e| anyhow::anyhow!("invalid argon2 params: {e}"))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

/// Derive a 32-byte encryption key from a master password and salt using Argon2id.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn derive_key(master_password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let argon2 = argon2_instance()?;
    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(master_password.as_bytes(), salt, &mut *key)
        .map_err(|e| anyhow::anyhow!("argon2 key derivation failed: {e}"))?;
    Ok(key)
}

/// Derive a per-account encryption key from the master key and a per-account salt using Argon2id.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn derive_key_from_master(master_key: &[u8; 32], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let argon2 = argon2_instance()?;
    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(master_key, salt, &mut *key)
        .map_err(|e| anyhow::anyhow!("argon2 per-account key derivation failed: {e}"))?;
    Ok(key)
}

/// Encrypt plaintext using AES-256-GCM. Returns (ciphertext, nonce).
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<(Vec<u8>, Vec<u8>)> {
    let cipher = Aes256Gcm::new(key.into());

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = &Nonce::from(nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| anyhow::anyhow!("AES-256-GCM encryption failed: {e}"))?;

    Ok((ciphertext, nonce_bytes.to_vec()))
}

/// Decrypt ciphertext using AES-256-GCM.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn decrypt(ciphertext: &[u8], key: &[u8; 32], nonce: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(key.into());
    #[allow(deprecated)] // from_slice needed for runtime-length nonce slices
    let nonce = Nonce::from_slice(nonce);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed: {e}"))
}

/// Generate a random 32-byte salt.
#[must_use]
pub fn generate_salt() -> [u8; 32] {
    let mut salt = [0u8; 32];
    OsRng.fill_bytes(&mut salt);
    salt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_salt_returns_32_bytes() {
        let salt = generate_salt();
        assert_eq!(salt.len(), 32);
    }

    #[test]
    fn generate_salt_produces_unique_values() {
        let salt_a = generate_salt();
        let salt_b = generate_salt();
        assert_ne!(salt_a, salt_b, "two salts should not be identical");
    }

    #[test]
    fn derive_key_returns_32_byte_key() {
        let salt = generate_salt();
        let key = derive_key("test_password", &salt).unwrap();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn different_salts_produce_different_keys() {
        let salt_a = generate_salt();
        let salt_b = generate_salt();
        let key_a = derive_key("same_password", &salt_a).unwrap();
        let key_b = derive_key("same_password", &salt_b).unwrap();
        assert_ne!(
            *key_a, *key_b,
            "different salts should produce different keys"
        );
    }

    #[test]
    fn different_passwords_produce_different_keys() {
        let salt = generate_salt();
        let key_a = derive_key("password_one", &salt).unwrap();
        let key_b = derive_key("password_two", &salt).unwrap();
        assert_ne!(
            *key_a, *key_b,
            "different passwords should produce different keys"
        );
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let salt = generate_salt();
        let key = derive_key("roundtrip_password", &salt).unwrap();
        let plaintext = b"secret EQ credentials";

        let (ciphertext, nonce) = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&ciphertext, &key, &nonce).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encrypt_decrypt_roundtrip_empty_plaintext() {
        let salt = generate_salt();
        let key = derive_key("empty_test", &salt).unwrap();
        let plaintext = b"";

        let (ciphertext, nonce) = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&ciphertext, &key, &nonce).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let salt = generate_salt();
        let correct_key = derive_key("correct_password", &salt).unwrap();
        let wrong_key = derive_key("wrong_password", &salt).unwrap();
        let plaintext = b"secret data";

        let (ciphertext, nonce) = encrypt(plaintext, &correct_key).unwrap();
        let result = decrypt(&ciphertext, &wrong_key, &nonce);

        assert!(result.is_err(), "decryption with wrong key should fail");
    }

    #[test]
    fn derive_key_from_master_produces_32_byte_key() {
        let master_salt = generate_salt();
        let master_key = derive_key("master_pass", &master_salt).unwrap();
        let account_salt = generate_salt();

        let account_key = derive_key_from_master(&master_key, &account_salt).unwrap();
        assert_eq!(account_key.len(), 32);
    }
}
