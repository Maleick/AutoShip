use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use arc_swap::ArcSwap;

use crate::bundle::PolicyBundle;

/// Per-scope atomic policy slot.
/// `load()` is lock-free and safe to call from a high-frequency decision loop.
pub type PolicySlot = Arc<ArcSwap<PolicyBundle>>;

/// Global registry of per-scope hot-swap slots.
pub struct PolicySwapRegistry {
    slots: Mutex<HashMap<String, PolicySlot>>,
}

impl PolicySwapRegistry {
    pub fn new() -> Self {
        Self {
            slots: Mutex::new(HashMap::new()),
        }
    }

    /// Return (or create) the slot for `scope`, initialised to the rule-based bundle.
    pub fn slot_for(&self, scope: &str) -> PolicySlot {
        let mut guard = self.slots.lock().expect("policy swap registry poisoned");
        guard
            .entry(scope.to_string())
            .or_insert_with(|| Arc::new(ArcSwap::from_pointee(PolicyBundle::rule_based(scope))))
            .clone()
    }

    /// Atomically swap `scope` to `bundle`.
    /// In-flight `load()` calls against the old bundle complete normally;
    /// the next `load()` returns the new bundle.
    pub fn swap(&self, scope: &str, bundle: PolicyBundle) {
        let slot = self.slot_for(scope);
        slot.store(Arc::new(bundle));
    }

    /// Read the currently active bundle for `scope`.
    pub fn load(&self, scope: &str) -> Arc<PolicyBundle> {
        self.slot_for(scope).load_full()
    }

    /// Return all currently active bundles.
    pub fn all_active(&self) -> Vec<Arc<PolicyBundle>> {
        let guard = self.slots.lock().expect("policy swap registry poisoned");
        guard.values().map(|slot| slot.load_full()).collect()
    }
}

impl Default for PolicySwapRegistry {
    fn default() -> Self {
        Self::new()
    }
}
