//! TextQuest learning harness: offline RL training pipeline for policy optimization.
//!
//! This crate scaffolds the L-0 through L-9 learning subsystem:
//! - L-1: Ledger (trajectory logging)
//! - L-2: Reward (signal shaping)
//! - L-3: ParamSearch (hyperparameter optimization)
//! - L-4: Bandits (exploration strategies)
//! - L-5: BC (behavior cloning baseline)
//! - L-6: Offline (CQL/IQL training)
//! - L-7: Canary (staged rollout monitoring)
//! - L-8: Policy (network representation)
//! - L-9: Advisor (runtime override heuristics)

pub mod ledger;
pub mod reward;
pub mod paramsearch;
pub mod bandits;
pub mod bc;
pub mod offline;
pub mod canary;
pub mod policy;
pub mod advisor;

pub use ledger::{ExperienceEntry, ExperienceLedger, ExperienceLedgerWriter};
pub use reward::RewardFn;
pub use paramsearch::ParamSearch;
pub use bandits::Bandit;
pub use bc::BehaviorCloner;
pub use offline::OfflineRL;
pub use canary::Canary;
pub use policy::Policy;
pub use advisor::Advisor;

pub type Result<T> = anyhow::Result<T>;
