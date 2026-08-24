//! Deterministic research scorer used before ONNX artifacts exist.
//!
//! This is not a trained production model. It exists so research pipelines,
//! backtests, and signal wiring can be tested without an ML runtime.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use trading_domain::{
    DomainError, FeatureSet, FeatureVersion, ModelVersion, Prediction, Predictor, ScoredPrediction,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DeterministicResearchModel {
    model_version: ModelVersion,
    feature_version: FeatureVersion,
    bias: Decimal,
    spread_weight: Decimal,
}

impl DeterministicResearchModel {
    #[must_use]
    pub fn quote_momentum_v1() -> Self {
        Self {
            model_version: ModelVersion("deterministic-quote-momentum-v1".to_owned()),
            feature_version: FeatureVersion("quote-momentum-v1".to_owned()),
            bias: Decimal::new(65, 2),
            spread_weight: Decimal::new(8, 0),
        }
    }
}

#[async_trait]
impl Predictor for DeterministicResearchModel {
    async fn predict(
        &self,
        features: &FeatureSet,
        decision_time: DateTime<Utc>,
    ) -> Result<ScoredPrediction, DomainError> {
        features.validate_point_in_time(decision_time)?;
        if features.feature_version != self.feature_version {
            return Err(DomainError::Adapter(
                "feature version mismatch for research model".to_owned(),
            ));
        }

        let spread = features.require("spread_fraction")?;
        let raw = self.bias - (spread * self.spread_weight);
        let probability = raw.clamp(Decimal::ZERO, Decimal::ONE);
        let uncertainty = (Decimal::ONE - probability) / Decimal::TWO;

        Ok(ScoredPrediction {
            id: Uuid::new_v4(),
            instrument: features.instrument.clone(),
            prediction: Prediction {
                probability,
                horizon_seconds: 86_400,
                uncertainty,
                model_version: self.model_version.clone(),
                feature_version: self.feature_version.0.clone(),
            },
            feature_set_id: features.id,
            scored_at: decision_time,
        })
    }
}
