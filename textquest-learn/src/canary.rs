//! Canary deployments and A/B testing harness.
//! L-7: Staged rollout and online performance monitoring.

/// Marker trait for canary deployment controllers.
pub trait Canary: Send + Sync {
    /// Check if deployment should proceed.
    fn should_proceed(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canary_marker_exists() {
        struct _DummyCanary;
        impl Canary for _DummyCanary {}
    }
}
