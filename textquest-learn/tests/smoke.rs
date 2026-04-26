//! Smoke test: verify core public types compile and link.

use textquest_learn::{
    bandit::runtime::BanditRuntime,
    bandit::shadow::PolicyMode,
    policy::{Policy, PolicyMetadata},
};

#[test]
fn public_types_link() {
    let _: Option<BanditRuntime> = None;
    let _ = PolicyMode::Shadow;
    let _: Option<PolicyMetadata> = None;
    let _: Option<Box<dyn Policy>> = None;
}
