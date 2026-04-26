//! Parameter search and hyperparameter optimization.
//! L-3: Offline Bayesian optimization + CMA-ES over FSM knobs.
//!
//! This module defines the core interface for parameter space definition,
//! objective functions, and search algorithms (Bayesian opt, CMA-ES).
//! Random search serves as the baseline implementation.

use std::collections::BTreeMap;

/// Marker trait for parameter search strategies (legacy).
pub trait ParamSearch: Send + Sync {
    /// Generate next parameter candidate.
    fn next(&mut self) -> Option<Vec<f32>> {
        None
    }
}

/// A named parameter with a specified range.
#[derive(Debug, Clone)]
pub struct Parameter {
    /// Unique identifier for the parameter (e.g., "cleric.complete_heal.trigger_pct").
    pub id: String,
    /// Minimum value (inclusive).
    pub min: f32,
    /// Maximum value (inclusive).
    pub max: f32,
    /// Default value.
    pub default: f32,
}

impl Parameter {
    /// Create a new parameter with bounds and default.
    pub fn new(id: impl Into<String>, min: f32, max: f32, default: f32) -> Self {
        Self {
            id: id.into(),
            min,
            max,
            default,
        }
    }

    /// Clamp a value to the parameter's valid range.
    pub fn clamp(&self, value: f32) -> f32 {
        value.max(self.min).min(self.max)
    }
}

/// Parameter space: a collection of named float ranges.
#[derive(Debug, Clone)]
pub struct ParamSpace {
    parameters: BTreeMap<String, Parameter>,
}

impl ParamSpace {
    /// Create an empty parameter space.
    pub fn new() -> Self {
        Self {
            parameters: BTreeMap::new(),
        }
    }

    /// Register a parameter.
    pub fn add_parameter(&mut self, param: Parameter) {
        self.parameters.insert(param.id.clone(), param);
    }

    /// Get all parameters.
    pub fn parameters(&self) -> &BTreeMap<String, Parameter> {
        &self.parameters
    }

    /// Get the number of parameters.
    pub fn len(&self) -> usize {
        self.parameters.len()
    }

    /// Check if the space is empty.
    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }

    /// Get default parameter values as a vector (in registration order).
    pub fn defaults(&self) -> Vec<f32> {
        self.parameters.values().map(|p| p.default).collect()
    }

    /// Convert a vector of values back to a parameter map.
    pub fn to_map(&self, values: &[f32]) -> Option<BTreeMap<String, f32>> {
        if values.len() != self.parameters.len() {
            return None;
        }

        let mut result = BTreeMap::new();
        for (i, (id, _)) in self.parameters.iter().enumerate() {
            result.insert(id.clone(), values[i]);
        }
        Some(result)
    }
}

impl Default for ParamSpace {
    fn default() -> Self {
        Self::new()
    }
}

/// Objective function: maps parameter values to a scalar reward to optimize.
pub type ObjectiveFn = fn(&[f32]) -> f32;

/// Result of a parameter search.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The best parameter set found.
    pub best_params: Vec<f32>,
    /// The reward achieved by the best parameter set.
    pub best_reward: f32,
    /// Number of evaluations performed.
    pub evaluations: usize,
    /// All evaluated candidates (params, reward) in order.
    pub history: Vec<(Vec<f32>, f32)>,
}

impl SearchResult {
    /// Create a new search result.
    pub fn new(best_params: Vec<f32>, best_reward: f32, evaluations: usize) -> Self {
        Self {
            best_params,
            best_reward,
            evaluations,
            history: Vec::new(),
        }
    }

    /// Add a candidate to the search history.
    pub fn add_candidate(&mut self, params: Vec<f32>, reward: f32) {
        self.history.push((params, reward));
    }
}

/// Optimizer trait: abstract interface for parameter search algorithms.
pub trait Optimizer: Send + Sync {
    /// Run the search and return the best result found.
    fn search(&mut self, space: &ParamSpace, objective: ObjectiveFn, budget: usize) -> SearchResult;
}

/// Random search optimizer: baseline implementation.
///
/// Samples random parameter combinations and returns the best found.
/// Serves as a reference for convergence and a fallback when specialized
/// algorithms (Bayesian opt, CMA-ES) are not available.
pub struct RandomSearchOptimizer {
    seed: u64,
}

impl RandomSearchOptimizer {
    /// Create a new random search optimizer with optional seed.
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Simple LCG random number generator (reproducible with seed).
    fn next_random(&mut self) -> f32 {
        const A: u64 = 6364136223846793005;
        const C: u64 = 1442695040888963407;
        self.seed = A.wrapping_mul(self.seed).wrapping_add(C);
        ((self.seed >> 32) as f32) / (u32::MAX as f32)
    }
}

impl Optimizer for RandomSearchOptimizer {
    fn search(&mut self, space: &ParamSpace, objective: ObjectiveFn, budget: usize) -> SearchResult {
        if space.is_empty() {
            return SearchResult::new(Vec::new(), 0.0, 0);
        }

        let mut best_params = space.defaults();
        let mut best_reward = objective(&best_params);
        let mut result = SearchResult::new(best_params.clone(), best_reward, 0);
        result.add_candidate(best_params.clone(), best_reward);

        for _ in 1..budget {
            // Generate random parameters
            let mut candidate = vec![0.0; space.len()];
            for (i, (_, param)) in space.parameters().iter().enumerate() {
                let rand_val = self.next_random();
                candidate[i] = param.min + rand_val * (param.max - param.min);
            }

            let reward = objective(&candidate);
            result.add_candidate(candidate.clone(), reward);

            if reward > best_reward {
                best_reward = reward;
                best_params = candidate;
            }
        }

        result.best_params = best_params;
        result.best_reward = best_reward;
        result.evaluations = budget;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_creation() {
        let param = Parameter::new("test.param", 0.0, 1.0, 0.5);
        assert_eq!(param.id, "test.param");
        assert_eq!(param.min, 0.0);
        assert_eq!(param.max, 1.0);
        assert_eq!(param.default, 0.5);
    }

    #[test]
    fn test_param_space_registration() {
        let mut space = ParamSpace::new();
        space.add_parameter(Parameter::new("p1", 0.0, 1.0, 0.5));
        space.add_parameter(Parameter::new("p2", -1.0, 1.0, 0.0));

        assert_eq!(space.len(), 2);
        assert!(!space.is_empty());

        let defaults = space.defaults();
        assert_eq!(defaults.len(), 2);
        assert_eq!(defaults[0], 0.5);
        assert_eq!(defaults[1], 0.0);
    }

    #[test]
    fn test_param_space_to_map() {
        let mut space = ParamSpace::new();
        space.add_parameter(Parameter::new("p1", 0.0, 1.0, 0.5));
        space.add_parameter(Parameter::new("p2", -1.0, 1.0, 0.0));

        let values = vec![0.7, 0.3];
        let map = space.to_map(&values).unwrap();

        assert_eq!(map.get("p1"), Some(&0.7));
        assert_eq!(map.get("p2"), Some(&0.3));
    }

    #[test]
    fn test_random_search_baseline() {
        let mut space = ParamSpace::new();
        space.add_parameter(Parameter::new("x", -5.0, 5.0, 0.0));
        space.add_parameter(Parameter::new("y", -5.0, 5.0, 0.0));

        // Simple quadratic objective: minimize (x-2)^2 + (y-3)^2
        let objective: ObjectiveFn = |params| {
            let x = params[0];
            let y = params[1];
            -((x - 2.0).powi(2) + (y - 3.0).powi(2))
        };

        let mut optimizer = RandomSearchOptimizer::new(42);
        let result = optimizer.search(&space, objective, 100);

        assert_eq!(result.evaluations, 100);
        assert!(result.best_reward <= 0.0); // Best is 0 at (2, 3)
        assert_eq!(result.history.len(), 100);
    }

    #[test]
    fn test_search_result_history() {
        let mut result = SearchResult::new(vec![0.5], 1.0, 0);
        result.add_candidate(vec![0.6], 0.9);
        result.add_candidate(vec![0.7], 1.1);

        assert_eq!(result.history.len(), 2);
        assert_eq!(result.history[0].1, 0.9);
        assert_eq!(result.history[1].1, 1.1);
    }
}
