//! Behavior cloning baseline and supervised training.
//! L-5: Behavioral cloning for policy initialization.

/// Marker trait for behavior cloning trainers.
pub trait BehaviorCloner: Send + Sync {
    /// Train on demonstration data.
    fn train(&mut self, _data: &[u8]) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bc_marker_exists() {
        struct _DummyBC;
        impl BehaviorCloner for _DummyBC {}
    }
}
