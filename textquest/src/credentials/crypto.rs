use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
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

/// Derive a 32-byte encryption key from a master password and salt using
/// Argon2id.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn derive_key<M: AsRef<str>>(master_password: M, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let argon2 = argon2_instance()?;
    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(master_password.as_ref().as_bytes(), salt, &mut *key)
        .map_err(|e| anyhow::anyhow!("argon2 key derivation failed: {e}"))?;
    Ok(key)
}

/// Derive a per-account encryption key from the master key and a per-account
/// salt using Argon2id.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn derive_key_from_master(
    master_key: impl AsRef<[u8]>,
    salt: &[u8],
) -> Result<Zeroizing<[u8; 32]>> {
    let master_key = master_key.as_ref();
    if master_key.len() != 32 {
        anyhow::bail!("master key must be 32 bytes");
    }

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
    rand::rng().fill_bytes(&mut nonce_bytes);
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
    rand::rng().fill_bytes(&mut salt);
    salt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_key_accepts_zeroizing_master_password() {
        let password = Zeroizing::new("test_password".to_string());
        let salt = generate_salt();
        let key_zeroized = derive_key(&password, &salt).unwrap();
        let key_plain = derive_key("test_password", &salt).unwrap();

        assert_eq!(*key_zeroized, *key_plain);
    }

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

    #[test]
    fn same_password_same_salt_produces_same_key() {
        let salt = generate_salt();
        let key_a = derive_key("test_password", &salt).unwrap();
        let key_b = derive_key("test_password", &salt).unwrap();
        assert_eq!(*key_a, *key_b);
    }

    #[test]
    fn encrypt_produces_12_byte_nonce() {
        let salt = generate_salt();
        let key = derive_key("test", &salt).unwrap();
        let (_, nonce) = encrypt(b"data", &key).unwrap();
        assert_eq!(nonce.len(), 12);
    }

    #[test]
    fn ciphertext_differs_from_plaintext() {
        let salt = generate_salt();
        let key = derive_key("test", &salt).unwrap();
        let plaintext = b"sensitive data here";
        let (ciphertext, _) = encrypt(plaintext, &key).unwrap();
        assert_ne!(&ciphertext[..], &plaintext[..]);
    }

    #[test]
    fn encrypt_large_plaintext() {
        let salt = generate_salt();
        let key = derive_key("large_test", &salt).unwrap();
        let plaintext = vec![0x42u8; 10_000];
        let (ciphertext, nonce) = encrypt(&plaintext, &key).unwrap();
        let decrypted = decrypt(&ciphertext, &key, &nonce).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_with_wrong_nonce_fails() {
        let salt = generate_salt();
        let key = derive_key("test_nonce", &salt).unwrap();
        let plaintext = b"test data";
        let (ciphertext, _) = encrypt(plaintext, &key).unwrap();
        let wrong_nonce = [0u8; 12];
        let result = decrypt(&ciphertext, &key, &wrong_nonce);
        assert!(result.is_err());
    }

    #[test]
    fn decrypt_tampered_ciphertext_fails() {
        let salt = generate_salt();
        let key = derive_key("tamper_test", &salt).unwrap();
        let plaintext = b"original data";
        let (mut ciphertext, nonce) = encrypt(plaintext, &key).unwrap();
        // Tamper with the ciphertext
        if !ciphertext.is_empty() {
            ciphertext[0] ^= 0xFF;
        }
        let result = decrypt(&ciphertext, &key, &nonce);
        assert!(result.is_err());
    }

    #[test]
    fn derive_key_from_master_different_salts_different_keys() {
        let master_salt = generate_salt();
        let master_key = derive_key("master", &master_salt).unwrap();
        let salt_a = generate_salt();
        let salt_b = generate_salt();
        let key_a = derive_key_from_master(&master_key, &salt_a).unwrap();
        let key_b = derive_key_from_master(&master_key, &salt_b).unwrap();
        assert_ne!(*key_a, *key_b);
    }

    #[test]
    fn each_encrypt_produces_different_nonce() {
        let salt = generate_salt();
        let key = derive_key("nonce_test", &salt).unwrap();
        let (_, nonce_a) = encrypt(b"data", &key).unwrap();
        let (_, nonce_b) = encrypt(b"data", &key).unwrap();
        assert_ne!(nonce_a, nonce_b);
    }
}
