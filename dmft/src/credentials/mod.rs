//! Encrypted credential store — Argon2id key derivation + AES-256-GCM encryption.

/// Cryptographic primitives — key derivation and authenticated encryption.
pub mod crypto;
/// Interactive credential prompts for CLI-based credential management.
pub mod prompt;
/// SQLite-backed credential store with encrypted account entries.
pub mod store;
