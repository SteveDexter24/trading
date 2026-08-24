//! ML inference boundary for versioned research models.
//!
//! Training is offline in Python. The trading runtime only scores frozen
//! artifacts and never sizes or approves orders from model outputs alone.

mod deterministic;
mod onnx;

pub use deterministic::DeterministicResearchModel;
pub use onnx::{production_training_disabled, OnnxModelConfig, OnnxPredictor};
