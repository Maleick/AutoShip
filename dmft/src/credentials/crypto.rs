use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit, OsRng},
};
use argon2::{Argon2, Algorithm, Version, Params};
use rand::RngCore;

/// Derive a 32-byte encryption key from a master password and salt using Argon2id.
pub fn derive_key(master_password: &str, salt: &[u8]) -> [u8; 32] {
    let params = Params::new(65536, 3, 1, Some(32)).expect("valid argon2 params");
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; 32];
    argon2
        .hash_password_into(master_password.as_bytes(), salt, &mut key)
        .expect("argon2 key derivation failed");
    key
}

/// Encrypt plaintext using AES-256-GCM. Returns (ciphertext, nonce).
pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> (Vec<u8>, Vec<u8>) {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .expect("AES-256-GCM encryption failed");

    (ciphertext, nonce_bytes.to_vec())
}

/// Decrypt ciphertext using AES-256-GCM.
pub fn decrypt(ciphertext: &[u8], key: &[u8; 32], nonce: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))
}

/// Generate a random 16-byte salt.
pub fn generate_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    salt
}
