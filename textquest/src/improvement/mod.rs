//! Self-improvement loop — Bayesian posterior tier for suggestion engine.
//!
//! This module provides conjugate prior posteriors (Beta-Binomial and Gaussian)
//! for tracking and suggesting optimal knob values across game sessions.

pub mod bayesian;
pub mod store;

pub use bayesian::{BayesianPosterior, BetaBinomial, Gaussian};
pub use store::PosteriorStore;

#[cfg(test)]
mod test_integration;
#[cfg(test)]
mod tests;
