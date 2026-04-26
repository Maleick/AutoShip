//! Reward signal modeling and transformation.
//! L-2: Reward shaping for behavioral learning.

/// Marker trait for reward functions.
pub trait RewardFn: Send + Sync {
    /// Compute reward signal.
    fn reward(&self, _state: &[u8]) -> f32 {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reward_fn_marker_exists() {
        struct _DummyReward;
        impl RewardFn for _DummyReward {}
    }
}
