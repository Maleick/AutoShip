//! Advisor loop: runtime policy monitoring and override heuristics.
//! L-9: Decision advisory system for policy corrections.

/// Marker trait for policy advisors.
pub trait Advisor: Send + Sync {
    /// Check if policy decision should be overridden.
    fn should_override(&self, _action: &[u8]) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advisor_marker_exists() {
        struct _DummyAdvisor;
        impl Advisor for _DummyAdvisor {}
    }
}
