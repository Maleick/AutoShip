use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::PolicyError;

const POLICY_SIG_KEY_ENV: &str = "TEXTQUEST_POLICY_SIG_KEY";

/// Verify the artifact at `artifact_dir` has a valid L-7 signature.
///
/// Layout expected:
///   artifact_dir/policy.bin   — raw policy data
///   artifact_dir/manifest.json — includes sha256 field
///   artifact_dir/signature     — hex keyed-BLAKE3(scope|version|sha256)
///
/// The signature key is loaded from `TEXTQUEST_POLICY_SIG_KEY`.
pub fn verify_artifact(artifact_dir: &Path, scope: &str, version: &str) -> Result<(), PolicyError> {
    let policy_path = artifact_dir.join("policy.bin");
    let manifest_path = artifact_dir.join("manifest.json");
    let sig_path = artifact_dir.join("signature");

    if !policy_path.exists() {
        return Err(PolicyError::MissingArtifact(format!(
            "{}/policy.bin missing",
            artifact_dir.display()
        )));
    }

    let manifest_bytes = std::fs::read(&manifest_path)?;
    let manifest: crate::bundle::PolicyManifest = serde_json::from_slice(&manifest_bytes)?;

    // Verify content hash matches policy.bin.
    let policy_bytes = std::fs::read(&policy_path)?;
    let computed = sha256_hex(&policy_bytes);
    if computed != manifest.sha256 {
        return Err(PolicyError::InvalidSignature {
            scope: scope.to_string(),
            version: version.to_string(),
        });
    }

    // Verify signature file exists and matches expected keyed digest.
    if !sig_path.exists() {
        return Err(PolicyError::InvalidSignature {
            scope: scope.to_string(),
            version: version.to_string(),
        });
    }

    let sig_bytes = std::fs::read(&sig_path)?;
    let provided_sig = std::str::from_utf8(&sig_bytes)
        .ok()
        .map(str::trim)
        .filter(|sig| !sig.is_empty())
        .ok_or_else(|| PolicyError::InvalidSignature {
            scope: scope.to_string(),
            version: version.to_string(),
        })?;
    let expected_sig = expected_signature(scope, version, &manifest.sha256).ok_or_else(|| {
        PolicyError::InvalidSignature {
            scope: scope.to_string(),
            version: version.to_string(),
        }
    })?;
    if provided_sig != expected_sig {
        return Err(PolicyError::InvalidSignature {
            scope: scope.to_string(),
            version: version.to_string(),
        });
    }

    Ok(())
}

fn expected_signature(scope: &str, version: &str, sha256: &str) -> Option<String> {
    let key = std::env::var(POLICY_SIG_KEY_ENV).ok()?;
    if key.is_empty() {
        return None;
    }
    Some(signature_hex(scope, version, sha256, &key))
}

/// Compute SHA-256(data) as lowercase hex.
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    hex_encode(&digest)
}

/// Build an artifact signature string using a keyed BLAKE3 MAC.
pub fn signature_hex(scope: &str, version: &str, sha256: &str, key: &str) -> String {
    let message = format!("{scope}|{version}|{sha256}");
    let key_material = blake3::hash(key.as_bytes());
    let mac = blake3::keyed_hash(key_material.as_bytes(), message.as_bytes());
    hex_encode(mac.as_bytes())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}
