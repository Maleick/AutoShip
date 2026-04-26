pub mod bandit;
pub mod bandits;
pub mod evaluator;
pub mod policy;
pub mod py_trainer;
pub mod reward;

pub use bandit::runtime::{BanditError, BanditRuntime};
pub use bandit::shadow::PolicyMode;

pub type Result<T> = anyhow::Result<T>;
