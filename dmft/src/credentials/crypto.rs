use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit, OsRng},
};
use anyhow::Result;
use argon2::{Argon2, Algorithm, Version, Params};
use rand::RngCore;
use zeroize::Zeroizing;

fn argon2_instance() -> Result<Argon2<'static>> {
    let params = Params::new(65536, 3, 4, Some(32))
        .map_err(|e| anyhow::anyhow!("invalid argon2 params: {}", e))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

/// Derive a 32-byte encryption key from a master password and salt using Argon2id.
pub fn derive_key(master_password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let argon2 = argon2_instance()?;
    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(master_password.as_bytes(), salt, &mut *key)
        .map_err(|e| anyhow::anyhow!("argon2 key derivation failed: {}", e))?;
    Ok(key)
}

/// Derive a per-account encryption key from the master key and a per-account salt using Argon2id.
pub fn derive_key_from_master(master_key: &[u8; 32], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let argon2 = argon2_instance()?;
    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(master_key, salt, &mut *key)
        .map_err(|e| anyhow::anyhow!("argon2 per-account key derivation failed: {}", e))?;
    Ok(key)
}

/// Encrypt plaintext using AES-256-GCM. Returns (ciphertext, nonce).
pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<(Vec<u8>, Vec<u8>)> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| anyhow::anyhow!("AES-256-GCM encryption failed: {}", e))?;

    Ok((ciphertext, nonce_bytes.to_vec()))
}

/// Decrypt ciphertext using AES-256-GCM.
pub fn decrypt(ciphertext: &[u8], key: &[u8; 32], nonce: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))
}

/// Generate a random 32-byte salt.
pub fn generate_salt() -> [u8; 32] {
    let mut salt = [0u8; 32];
    OsRng.fill_bytes(&mut salt);
    salt
}
