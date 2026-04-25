use crate::dataset::FlaggedSegmentDataset;
use crate::{BehaviorCloningError, Result};
use ndarray::Array1;

#[derive(Debug)]
pub struct BehaviorCloningTrainer {
    learning_rate: f32,
    epochs: u32,
    hidden_units: usize,
}

impl BehaviorCloningTrainer {
    pub fn new(learning_rate: f32, epochs: u32, hidden_units: usize) -> Self {
        BehaviorCloningTrainer {
            learning_rate,
            epochs,
            hidden_units,
        }
    }

    pub fn train(
        &self,
        dataset: &FlaggedSegmentDataset,
    ) -> Result<TrainedModel> {
        let (train_pairs, _heldout_pairs) = dataset.split_train_heldout(0.2);

        let context_dim = dataset.context_dim();
        let num_actions = dataset.num_actions() as usize;

        // Build training matrices
        let mut contexts = Vec::new();
        let mut actions = Vec::new();

        for pair in &train_pairs {
            contexts.push(pair.context.clone());
            actions.push(pair.action);
        }

        if contexts.is_empty() {
            return Err(BehaviorCloningError::Training(
                "No training samples available".to_string(),
            )
            .into());
        }


        // Initialize random weights for small MLP (2 layers, hidden_units per layer)
        // W1: (context_dim, hidden_units), b1: (hidden_units,)
        // W2: (hidden_units, num_actions), b2: (num_actions,)
        let w1 = Self::random_matrix(context_dim, self.hidden_units);
        let b1 = Array1::zeros(self.hidden_units);
        let w2 = Self::random_matrix(self.hidden_units, num_actions);
        let b2 = Array1::zeros(num_actions);

        // Simple training loop: forward pass, compute loss, gradient descent
        let mut model_weights = ModelWeights { w1, b1, w2, b2 };

        for _epoch in 0..self.epochs {
            // Forward pass and update (simplified SGD, policy gradient style)
            for (context, &action) in train_pairs.iter().zip(actions.iter()) {
                let context_vec = Array1::from_vec(context.context.clone());
                let _logits = model_weights.forward(&context_vec);
                let target_action = action as usize;

                // Simple gradient update: move policy toward action
                let hidden = {
                    let temp = context_vec.dot(&model_weights.w1) + &model_weights.b1;
                    Self::relu(&temp)
                };

                // Update W2 to increase probability of target action
                if target_action < num_actions {
                    for i in 0..self.hidden_units {
                        for j in 0..num_actions {
                            if j == target_action {
                                model_weights.w2[[i, j]] += self.learning_rate * hidden[i];
                            } else {
                                model_weights.w2[[i, j]] -= self.learning_rate * hidden[i] * 0.1;
                            }
                        }
                    }
                }
            }
        }

        Ok(TrainedModel {
            class: dataset.class.clone(),
            context_dim,
            num_actions,
            weights: model_weights,
        })
    }

    fn random_matrix(rows: usize, cols: usize) -> ndarray::Array2<f32> {
        use ndarray::Array2;
        let mut data = vec![0.0; rows * cols];
        for (i, elem) in data.iter_mut().enumerate() {
            // Simple deterministic initialization based on position (for testing)
            *elem = ((i as f32 % 100.0) - 50.0) * 0.001;
        }
        Array2::from_shape_vec((rows, cols), data).unwrap()
    }

    fn relu(x: &ndarray::Array1<f32>) -> ndarray::Array1<f32> {
        x.mapv(|v| v.max(0.0))
    }
}

#[derive(Debug, Clone)]
pub struct ModelWeights {
    pub w1: ndarray::Array2<f32>,
    pub b1: ndarray::Array1<f32>,
    pub w2: ndarray::Array2<f32>,
    pub b2: ndarray::Array1<f32>,
}

impl ModelWeights {
    pub fn forward(&self, context: &ndarray::Array1<f32>) -> ndarray::Array1<f32> {
        let hidden = Self::relu_arr(&(context.dot(&self.w1) + &self.b1));
        hidden.dot(&self.w2) + &self.b2
    }

    fn relu_arr(x: &ndarray::Array1<f32>) -> ndarray::Array1<f32> {
        x.mapv(|v| v.max(0.0))
    }
}

#[derive(Debug)]
pub struct TrainedModel {
    pub class: String,
    pub context_dim: usize,
    pub num_actions: usize,
    pub weights: ModelWeights,
}

impl TrainedModel {
    pub fn predict(&self, context: &[f32]) -> Result<u32> {
        if context.len() != self.context_dim {
            return Err(BehaviorCloningError::Training(format!(
                "Context dimension mismatch: expected {}, got {}",
                self.context_dim,
                context.len()
            ))
            .into());
        }

        let context_vec = ndarray::Array1::from_vec(context.to_vec());
        let logits = self.weights.forward(&context_vec);

        let action = logits
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(idx, _)| idx as u32)
            .ok_or_else(|| anyhow::anyhow!(BehaviorCloningError::Training("No actions available".to_string())))?;

        Ok(action)
    }
}
