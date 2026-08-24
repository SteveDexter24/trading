//! Deterministic strategy baselines and prediction-gated research strategies.

mod features;
mod prediction_gated;
mod price_threshold;

pub use features::quote_momentum_features;
pub use prediction_gated::PredictionGatedStrategy;
pub use price_threshold::{threshold_signal, PriceThresholdStrategy};
