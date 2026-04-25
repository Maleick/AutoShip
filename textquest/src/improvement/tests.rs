//! Integration tests for the Bayesian improvement engine.

#[cfg(test)]
mod integration_tests {
    use crate::improvement::{BayesianPosterior, BetaBinomial, Gaussian, PosteriorStore};

    #[test]
    fn suggestion_workflow_beta_binomial() {
        let mut posterior = BetaBinomial::new();

        // No suggestion with default uniform prior
        assert!(!posterior.should_suggest(0.5));

        // After 50 successes, suggest toward 1.0
        for _ in 0..50 {
            posterior.update(1.0);
        }
        assert!(posterior.should_suggest(0.2)); // far from mean
        assert!(!posterior.should_suggest(0.98)); // close to mean

        let suggestion = posterior.suggestion(0.2).expect("should suggest");
        assert!(suggestion.proposed > 0.9);
        assert!(suggestion.confidence > 0.5);
    }

    #[test]
    fn suggestion_workflow_gaussian() {
        let mut posterior = Gaussian::with_prior(0.0, 100.0, 1.0);

        // No suggestion initially
        assert!(!posterior.should_suggest(0.0));

        // After 50 observations at 50.0
        for _ in 0..50 {
            posterior.update(50.0);
        }
        assert!(posterior.should_suggest(0.0)); // far from mean
        assert!(!posterior.should_suggest(50.1)); // close to mean

        let suggestion = posterior.suggestion(0.0).expect("should suggest");
        assert!((suggestion.proposed - 50.0).abs() < 1.0);
    }

    #[test]
    fn persistence_roundtrip() -> anyhow::Result<()> {
        let store = PosteriorStore::open_memory()?;

        // Create and persist a Beta-Binomial
        let mut bb = BetaBinomial::new();
        for _ in 0..30 {
            bb.update(1.0);
        }
        store.save_beta_binomial("warrior_1", "heal_landing", &bb)?;

        // Load and verify
        let loaded_bb = store.load_beta_binomial("warrior_1", "heal_landing")?;
        assert!((loaded_bb.mean() - bb.mean()).abs() < 0.01);

        // Create and persist a Gaussian
        let mut gauss = Gaussian::with_prior(50.0, 10.0, 1.0);
        for _ in 0..20 {
            gauss.update(51.0);
        }
        store.save_gaussian("warrior_1", "mana_first_cast", &gauss)?;

        // Load and verify
        let loaded_gauss = store.load_gaussian("warrior_1", "mana_first_cast", 1.0)?;
        assert!((loaded_gauss.mean() - gauss.mean()).abs() < 0.1);

        Ok(())
    }

    #[test]
    fn convergence_under_different_observation_counts() {
        // Verify that posteriors converge correctly with varying sample sizes
        let mut p10 = BetaBinomial::new();
        let mut p100 = BetaBinomial::new();
        let mut p1000 = BetaBinomial::new();

        for _ in 0..10 {
            p10.update(1.0);
        }
        for _ in 0..100 {
            p100.update(1.0);
        }
        for _ in 0..1000 {
            p1000.update(1.0);
        }

        assert!(p10.std_dev() > p100.std_dev());
        assert!(p100.std_dev() > p1000.std_dev());
        assert!(p1000.std_dev() < 0.01);
    }
}
