//! Parameter search and hyperparameter optimization.
//! L-3: Grid/random search over algorithm hyperparameters.

/// Marker trait for parameter search strategies.
pub trait ParamSearch: Send + Sync {
    /// Generate next parameter candidate.
    fn next(&mut self) -> Option<Vec<f32>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paramsearch_marker_exists() {
        struct _DummySearch;
        impl ParamSearch for _DummySearch {}
    }
}
