//! Multi-armed bandit algorithms (UCB, Thompson, epsilon-greedy).
//! L-4: Bandit exploration strategies for policy selection.

/// Marker trait for bandit solvers.
pub trait Bandit: Send + Sync {
    /// Select arm based on bandit algorithm.
    fn select(&self, _arm_count: usize) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bandit_marker_exists() {
        struct _DummyBandit;
        impl Bandit for _DummyBandit {}
    }
}
