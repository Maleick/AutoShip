//! Offline reinforcement learning (CQL, IQL, AWR).
//! L-6: Offline RL algorithms for policy improvement.

/// Marker trait for offline RL solvers.
pub trait OfflineRL: Send + Sync {
    /// Run offline RL update step.
    fn update(&mut self, _batch: &[u8]) -> anyhow::Result<f32> {
        Ok(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_rl_marker_exists() {
        struct _DummyOffline;
        impl OfflineRL for _DummyOffline {}
    }
}
