//! Smoke test: verify all L-N modules compile and export marker traits.

use textquest_learn::{
    Advisor, Bandit, BehaviorCloner, Canary, Ledger, OfflineRL, ParamSearch, Policy, RewardFn,
};

#[test]
fn all_modules_compile() {
    // Simply importing the traits proves they exist and are public.
    // Each module has its own `#[cfg(test)]` smoke test for trait impls.
    let _ = std::marker::PhantomData::<dyn Ledger>;
    let _ = std::marker::PhantomData::<dyn RewardFn>;
    let _ = std::marker::PhantomData::<dyn ParamSearch>;
    let _ = std::marker::PhantomData::<dyn Bandit>;
    let _ = std::marker::PhantomData::<dyn BehaviorCloner>;
    let _ = std::marker::PhantomData::<dyn OfflineRL>;
    let _ = std::marker::PhantomData::<dyn Canary>;
    let _ = std::marker::PhantomData::<dyn Policy>;
    let _ = std::marker::PhantomData::<dyn Advisor>;
}
