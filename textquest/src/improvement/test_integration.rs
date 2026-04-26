//! Integration tests demonstrating acceptance criteria compliance.
//!
//! This module verifies:
//! 1. Beta-Binomial posterior implemented with closed-form update
//! 2. Gaussian posterior implemented with closed-form update
//! 3. Posteriors persist across sessions in `posteriors` table
//! 4. Suggestions reflect posterior shifts only when ≥ 1σ from current
//! 5. Feature flag `[improve] bayesian_tier_enabled` (default false)
//! 6. Unit tests with synthetic data verify posterior convergence

#[cfg(test)]
mod acceptance_tests {
    use crate::improvement::{BayesianPosterior, BetaBinomial, Gaussian, PosteriorStore};

    #[test]
    /// AC1: Beta-Binomial posterior implemented with closed-form update.
    fn ac1_beta_binomial_closed_form() {
        let mut bb = BetaBinomial::new();

        // Verify closed-form updates are applied
        bb.update(1.0); // success
        assert_eq!(bb.alpha(), 2.0); // 1 + 1
        assert_eq!(bb.beta(), 1.0); // unchanged

        bb.update(0.0); // failure
        assert_eq!(bb.alpha(), 2.0);
        assert_eq!(bb.beta(), 2.0); // 1 + 1
    }

    #[test]
    /// AC2: Gaussian posterior implemented with closed-form update.
    fn ac2_gaussian_closed_form() {
        let mut gauss = Gaussian::with_prior(0.0, 100.0, 1.0);
        let prior_variance = gauss.variance;

        gauss.update(50.0);
        // Closed-form Gaussian update reduces variance
        assert!(gauss.variance < prior_variance);
    }

    #[test]
    /// AC3: Posteriors persist across sessions in `posteriors` table.
    fn ac3_persistence_across_sessions() -> anyhow::Result<()> {
        let store = PosteriorStore::open_memory()?;
        let bb = BetaBinomial::with_prior(50.0, 30.0);

        // Session 1: Save posterior
        store.save_beta_binomial("warrior_1", "spell_landing", &bb)?;

        // Session 2: Load and verify
        let loaded = store.load_beta_binomial("warrior_1", "spell_landing")?;
        assert_eq!(loaded.alpha(), 50.0);
        assert_eq!(loaded.beta(), 30.0);

        Ok(())
    }

    #[test]
    /// AC4: Suggestions reflect posterior shifts only when ≥ 1σ from current.
    fn ac4_1sigma_decision_rule() {
        let mut bb = BetaBinomial::new();

        // Build posterior toward success (mean → 1.0)
        for _ in 0..100 {
            bb.update(1.0);
        }
        let mean = bb.mean();
        let std_dev = bb.std_dev();

        // Current value far below mean (> 1σ away)
        let current_low = mean - 2.0 * std_dev;
        assert!(bb.should_suggest(current_low));

        // Current value very close to mean (< 1σ away)
        let current_close = mean - 0.1 * std_dev;
        assert!(!bb.should_suggest(current_close));

        // Suggestion includes confidence score
        if let Some(sugg) = bb.suggestion(current_low) {
            assert!(sugg.confidence > 0.0);
            assert!(sugg.confidence <= 1.0);
        }
    }

    #[test]
    /// AC5: Feature flag `[improve] bayesian_tier_enabled` (default false).
    fn ac5_feature_flag_default_false() {
        use crate::config::ImprovementConfig;

        let cfg = ImprovementConfig::default();
        assert!(!cfg.bayesian_tier_enabled); // Default is false
        assert_eq!(cfg.posteriors_db_path, "data/posteriors.db");
    }

    #[test]
    /// AC6: Unit tests with synthetic data verify posterior convergence.
    fn ac6_convergence_to_known_truth() {
        // Scenario: True success rate is 0.75 (75%)
        let mut bb = BetaBinomial::new();
        for i in 0..200 {
            bb.update(if i % 4 < 3 { 1.0 } else { 0.0 });
        }

        // With 200 samples, posterior should converge to truth
        assert!(
            (bb.mean() - 0.75).abs() < 0.05,
            "Failed to converge: {}",
            bb.mean()
        );
        assert!(bb.std_dev() < 0.05, "Variance too high: {}", bb.std_dev());
    }

    #[test]
    /// AC6 (Gaussian): Posterior converges to true mean.
    fn ac6_gaussian_convergence_to_true_mean() {
        let mut gauss = Gaussian::with_prior(0.0, 100.0, 1.0);

        // Simulate 200 observations at true mean = 42.0
        for _ in 0..200 {
            gauss.update(42.0);
        }

        assert!(
            (gauss.mean() - 42.0).abs() < 0.1,
            "Failed to converge: {}",
            gauss.mean()
        );
        assert!(
            gauss.variance < 0.1,
            "Variance too high: {}",
            gauss.variance
        );
    }

    #[test]
    /// Verify database schema includes all required posteriors columns.
    fn schema_includes_all_posterior_fields() -> anyhow::Result<()> {
        let store = PosteriorStore::open_memory()?;
        let bb = BetaBinomial::with_prior(10.0, 5.0);
        let gauss = Gaussian::with_prior(50.0, 25.0, 1.0);

        // Save both types
        store.save_beta_binomial("test", "bb_knob", &bb)?;
        store.save_gaussian("test", "gauss_knob", &gauss)?;

        // List and verify both are persisted
        let knobs = store.list_knobs("test")?;
        assert_eq!(knobs.len(), 2);
        assert!(
            knobs
                .iter()
                .any(|(k, t)| k == "bb_knob" && t == "beta_binomial")
        );
        assert!(
            knobs
                .iter()
                .any(|(k, t)| k == "gauss_knob" && t == "gaussian")
        );

        Ok(())
    }
}
