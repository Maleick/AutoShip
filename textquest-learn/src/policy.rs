//! Policy representation, inference, and versioning.
//! L-8: Neural network policy models and serialization.

/// Marker trait for policy implementations.
pub trait Policy: Send + Sync {
    /// Infer action from state.
    fn infer(&self, _state: &[u8]) -> anyhow::Result<Vec<f32>> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_marker_exists() {
        struct _DummyPolicy;
        impl Policy for _DummyPolicy {}
    }
}
