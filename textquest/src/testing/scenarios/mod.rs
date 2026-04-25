//! Ready-made [`TestScenario`](crate::testing::scenario::TestScenario)
//! implementations for harness and CI tests.

pub mod mocks;

pub use mocks::{CountdownScenario, FastFailScenario, InstantScenario, MetricTestScenario};
