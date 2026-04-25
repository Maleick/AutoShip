//! Versioned policy store with hot-swap and rollback for TextQuest learned strategies.
//!
//! # Store layout
//!
//! ```text
//! ~/.textquest/policies/
//! ├── registry.sqlite
//! ├── artifacts/
//! │   ├── cleric.heal_picker.bandits.v3/
//! │   │   ├── policy.bin
//! │   │   ├── manifest.json
//! │   │   ├── canary_report.html
//! │   │   └── signature
//! │   └── ...
//! └── rollback_history.jsonl
//! ```

pub mod bundle;
pub mod error;
pub mod hot_swap;
pub mod registry;
pub mod rollback;
pub mod signature;
pub mod store;

pub use bundle::{PolicyBundle, PolicyManifest};
pub use error::PolicyError;
pub use hot_swap::PolicySwapRegistry;
pub use registry::RegistryEntry;
pub use rollback::{RollbackEvent, RollbackHistory};
pub use store::PolicyStore;

/// Returns the default policy store root: `~/.textquest/policies/`.
pub fn default_store_root() -> std::path::PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    std::path::Path::new(&home)
        .join(".textquest")
        .join("policies")
}
