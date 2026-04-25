use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use crate::{
    bundle::{PolicyBundle, PolicyManifest},
    error::PolicyError,
    hot_swap::PolicySwapRegistry,
    registry::PolicyRegistry,
    rollback::{RollbackEvent, RollbackHistory, now_iso8601},
    signature::verify_artifact,
};

/// Root of the on-disk policy store layout:
///
/// ```text
/// <root>/
///   registry.sqlite
///   artifacts/<scope>.<version>/
///     policy.bin
///     manifest.json
///     canary_report.html
///     signature
///   rollback_history.jsonl
/// ```
pub struct PolicyStore {
    root: PathBuf,
    registry: PolicyRegistry,
    history: RollbackHistory,
    swap: Arc<PolicySwapRegistry>,
}

impl PolicyStore {
    /// Open (or create) the policy store at `root`.
    pub fn open(root: &Path) -> Result<Self, PolicyError> {
        std::fs::create_dir_all(root)?;
        std::fs::create_dir_all(root.join("artifacts"))?;

        let registry = PolicyRegistry::open(&root.join("registry.sqlite"))?;
        let history = RollbackHistory::new(root.join("rollback_history.jsonl"));
        let swap = Arc::new(PolicySwapRegistry::new());

        let store = Self { root: root.to_path_buf(), registry, history, swap };

        // Restore active bundles from registry into swap slots on open.
        store.restore_active_bundles()?;

        Ok(store)
    }

    fn restore_active_bundles(&self) -> Result<(), PolicyError> {
        for entry in self.registry.all_active()? {
            if let Ok(bundle) = self.load_bundle_from_disk(&entry.scope, &entry.version) {
                self.swap.swap(&entry.scope, bundle);
            }
        }
        Ok(())
    }

    /// Promote an external artifact directory into the store after signature verification.
    ///
    /// `artifact_src` must contain policy.bin, manifest.json, and signature.
    /// The artifact is copied into the store's artifacts/ directory.
    /// Registry entry is created but NOT set active.
    pub fn promote(&self, artifact_src: &Path) -> Result<(), PolicyError> {
        let manifest = load_manifest(artifact_src)?;
        let scope = &manifest.scope;
        let version = &manifest.version;

        verify_artifact(artifact_src, scope, version)?;

        let dest = self.artifact_dir(scope, version);
        if dest != artifact_src {
            std::fs::create_dir_all(&dest)?;
            for entry in std::fs::read_dir(artifact_src)? {
                let entry = entry?;
                std::fs::copy(entry.path(), dest.join(entry.file_name()))?;
            }
        }

        self.registry.upsert(
            scope,
            version,
            &dest.to_string_lossy(),
            &manifest.source_bundle,
            &manifest.produced_at,
        )?;

        tracing::info!(scope, version, "policy artifact promoted");
        Ok(())
    }

    /// Activate a previously promoted version for `scope`.
    /// Hot-swaps the bundle into the in-process swap slot immediately.
    pub fn activate(&self, scope: &str, version: &str) -> Result<(), PolicyError> {
        let previous = self.registry.active_entry(scope)?.map(|e| e.version);

        self.registry.set_active(scope, version)?;

        let bundle = self.load_bundle_from_disk(scope, version)?;
        self.swap.swap(scope, bundle);

        self.history.append(&RollbackEvent::Activated {
            scope: scope.to_string(),
            version: version.to_string(),
            previous_version: previous,
            timestamp: now_iso8601(),
        })?;

        tracing::info!(scope, version, "policy activated");
        Ok(())
    }

    /// Roll back `scope` to the version that was active just before the current one.
    pub fn rollback(&self, scope: &str) -> Result<(), PolicyError> {
        let entries = self.registry.entries_for_scope(scope)?;

        let current_idx = entries.iter().position(|e| e.active);
        let current_version = match current_idx {
            Some(i) => entries[i].version.clone(),
            None => {
                // Nothing active — activate the last promoted entry.
                return entries.last().ok_or_else(|| PolicyError::NoPreviousVersion {
                    scope: scope.to_string(),
                }).and_then(|e| self.activate(scope, &e.version));
            }
        };

        // Find the entry immediately before the active one.
        let prev_version = match current_idx {
            Some(i) if i > 0 => entries[i - 1].version.clone(),
            _ => {
                return Err(PolicyError::NoPreviousVersion {
                    scope: scope.to_string(),
                });
            }
        };

        self.registry.set_active(scope, &prev_version)?;
        let bundle = self.load_bundle_from_disk(scope, &prev_version)?;
        self.swap.swap(scope, bundle);

        self.history.append(&RollbackEvent::RolledBack {
            scope: scope.to_string(),
            from_version: current_version,
            to_version: prev_version,
            timestamp: now_iso8601(),
        })?;

        tracing::info!(scope, "policy rolled back");
        Ok(())
    }

    /// Get the current live bundle for `scope` (for in-process decision use).
    pub fn bundle(&self, scope: &str) -> Arc<PolicyBundle> {
        self.swap.load(scope)
    }

    /// Get all currently active bundles across all scopes.
    pub fn all_active_bundles(&self) -> Vec<Arc<PolicyBundle>> {
        self.swap.all_active()
    }

    /// Return all active registry entries (for TUI display).
    pub fn active_registry_entries(&self) -> Result<Vec<crate::registry::RegistryEntry>, PolicyError> {
        self.registry.all_active()
    }

    fn artifact_dir(&self, scope: &str, version: &str) -> PathBuf {
        // Sanitise scope/version for use as directory names.
        let safe = format!("{}.{}", scope, version).replace('/', "_").replace('\\', "_");
        self.root.join("artifacts").join(safe)
    }

    fn load_bundle_from_disk(&self, scope: &str, version: &str) -> Result<PolicyBundle, PolicyError> {
        let dir = self.artifact_dir(scope, version);
        let manifest = load_manifest(&dir)?;
        let data = std::fs::read(dir.join("policy.bin")).unwrap_or_default();
        Ok(PolicyBundle {
            scope: manifest.scope,
            version: manifest.version,
            source_bundle: manifest.source_bundle,
            data,
            promoted_at: SystemTime::now(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn load_manifest(dir: &Path) -> Result<PolicyManifest, PolicyError> {
    let bytes = std::fs::read(dir.join("manifest.json"))?;
    Ok(serde_json::from_slice(&bytes)?)
}
