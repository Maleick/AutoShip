//! Config export/import via base64 strings — rgmercs `rg_config_share` parity (gap #5).
//!
//! Operators can share configurations by exporting a module's settings to a
//! compact base64 string, then pasting it into another instance via import.
//!
//! # Format
//! `<module>:<base64(json(config))>`
//!
//! The JSON payload is the TOML-equivalent structure serialized as JSON, then
//! base64-encoded. The module prefix prevents cross-module import accidents.
//!
//! # CLI surface
//! ```text
//! textquest config export --module <name>   # prints a share string
//! textquest config import <share_string>    # applies with confirmation
//! ```

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};

/// A portable, shareable config blob for one named module.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfigShareBlob {
    /// Module name this blob belongs to (e.g. "warrior", "pull", "camp").
    pub module: String,
    /// Arbitrary JSON-encoded settings for the module.
    pub payload: serde_json::Value,
}

impl ConfigShareBlob {
    /// Encode this blob to a share string: `<module>:<base64(json)>`.
    ///
    /// # Errors
    /// Returns an error if JSON serialization fails.
    pub fn encode(&self) -> Result<String> {
        validate_module_name(&self.module).context("invalid module name for share export")?;
        let json = serde_json::to_vec(self).context("serialize config blob")?;
        Ok(format!("{}:{}", self.module, B64.encode(&json)))
    }

    /// Decode a share string produced by [`encode`].
    ///
    /// # Errors
    /// Returns an error if the string is malformed, base64-invalid, or the
    /// JSON cannot be deserialized.
    pub fn decode(share_string: &str) -> Result<Self> {
        let (prefix, b64) = share_string
            .split_once(':')
            .context("share string must be '<module>:<base64>'")?;
        validate_module_name(prefix).context("invalid module prefix")?;

        let bytes = B64.decode(b64).context("base64 decode")?;
        let blob: Self = serde_json::from_slice(&bytes).context("JSON decode")?;
        validate_module_name(&blob.module).context("invalid module in payload")?;

        if blob.module != prefix {
            bail!(
                "module mismatch: prefix says '{}' but payload says '{}'",
                prefix,
                blob.module
            );
        }

        Ok(blob)
    }
}

fn validate_module_name(module: &str) -> Result<()> {
    if module.is_empty() {
        bail!("module name is empty");
    }

    if !module
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        bail!("invalid module name: {module:?}");
    }

    Ok(())
}

/// Export a TOML file as a share string.
///
/// Reads the TOML at `path`, converts to a JSON `Value`, and wraps it in a
/// [`ConfigShareBlob`] for the given `module` name.
///
/// # Errors
/// Returns an error if the file cannot be read or the TOML cannot be parsed.
pub fn export_toml_file(module: &str, path: &std::path::Path) -> Result<String> {
    let toml_str =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let value: toml::Value = toml::from_str(&toml_str).context("parse TOML")?;
    let json = toml_value_to_json(value);
    let blob = ConfigShareBlob {
        module: module.to_owned(),
        payload: json,
    };
    blob.encode()
}

/// Import a share string and return the decoded [`ConfigShareBlob`].
///
/// Callers are responsible for showing a confirmation prompt before applying.
///
/// # Errors
/// Returns an error if decoding fails (see [`ConfigShareBlob::decode`]).
pub fn import_share_string(share_string: &str) -> Result<ConfigShareBlob> {
    ConfigShareBlob::decode(share_string)
}

/// Apply an imported blob by writing its payload as TOML to `path`.
///
/// The file is overwritten atomically (write to a `.tmp` then rename).
///
/// # Errors
/// Returns an error if the payload cannot be serialized to TOML or the file
/// cannot be written.
pub fn apply_blob_to_file(blob: &ConfigShareBlob, path: &std::path::Path) -> Result<()> {
    let toml_value = json_value_to_toml(&blob.payload)?;
    let toml_str = toml::to_string_pretty(&toml_value).context("serialize to TOML")?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, toml_str).with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("rename {} → {}", tmp.display(), path.display()))?;
    Ok(())
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn toml_value_to_json(v: toml::Value) -> serde_json::Value {
    match v {
        toml::Value::String(s) => serde_json::Value::String(s),
        toml::Value::Integer(i) => serde_json::Value::Number(i.into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        toml::Value::Boolean(b) => serde_json::Value::Bool(b),
        toml::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(toml_value_to_json).collect())
        }
        toml::Value::Table(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, toml_value_to_json(v)))
                .collect(),
        ),
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
    }
}

fn json_value_to_toml(v: &serde_json::Value) -> Result<toml::Value> {
    match v {
        serde_json::Value::Null => bail!("TOML does not support null values"),
        serde_json::Value::Bool(b) => Ok(toml::Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(toml::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(toml::Value::Float(f))
            } else {
                bail!("numeric value out of TOML range")
            }
        }
        serde_json::Value::String(s) => Ok(toml::Value::String(s.clone())),
        serde_json::Value::Array(arr) => {
            let items: Result<Vec<_>> = arr.iter().map(json_value_to_toml).collect();
            Ok(toml::Value::Array(items?))
        }
        serde_json::Value::Object(map) => {
            let table: Result<toml::map::Map<_, _>> = map
                .iter()
                .map(|(k, v)| json_value_to_toml(v).map(|tv| (k.clone(), tv)))
                .collect();
            Ok(toml::Value::Table(table?))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_encode_decode() {
        let blob = ConfigShareBlob {
            module: "warrior".into(),
            payload: serde_json::json!({ "rest_mana_pct": 60, "pull_radius": 200.0 }),
        };
        let share = blob.encode().unwrap();
        assert!(share.starts_with("warrior:"));
        let decoded = ConfigShareBlob::decode(&share).unwrap();
        assert_eq!(decoded.module, "warrior");
        assert_eq!(decoded.payload["rest_mana_pct"], 60);
    }

    #[test]
    fn decode_rejects_module_mismatch() {
        // Manually craft a string with wrong prefix
        let blob = ConfigShareBlob {
            module: "warrior".into(),
            payload: serde_json::json!({}),
        };
        let share = blob.encode().unwrap();
        // Replace prefix
        let tampered = share.replacen("warrior:", "cleric:", 1);
        assert!(ConfigShareBlob::decode(&tampered).is_err());
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(ConfigShareBlob::decode("not_valid").is_err());
    }

    #[test]
    fn decode_rejects_path_traversal_module() {
        let blob = ConfigShareBlob {
            module: "../../evil".into(),
            payload: serde_json::json!({}),
        };
        let json = serde_json::to_vec(&blob).unwrap();
        let share = format!("../../evil:{}", B64.encode(json));
        assert!(ConfigShareBlob::decode(&share).is_err());
    }

    #[test]
    fn decode_rejects_absolute_path_module() {
        let blob = ConfigShareBlob {
            module: "/tmp/evil".into(),
            payload: serde_json::json!({}),
        };
        let json = serde_json::to_vec(&blob).unwrap();
        let share = format!("/tmp/evil:{}", B64.encode(json));
        assert!(ConfigShareBlob::decode(&share).is_err());
    }
}
