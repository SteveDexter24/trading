//! Strategy that turns versioned predictions into signals.
//!
//! This module never sizes or approves orders. Risk and execution remain
//! responsible for those decisions.

use async_trait::async_trait;
use rust_decimal::Decimal;
use trading_domain::{DomainError, MarketEvent, Predictor, Side, Signal, StrategyId, StrategyPort};
use uuid::Uuid;

#[derive(Clone)]
pub struct PredictionGatedStrategy {
    id: StrategyId,
    predictor: std::sync::Arc<dyn Predictor>,
    minimum_probability: Decimal,
    maximum_uncertainty: Decimal,
}

impl std::fmt::Debug for PredictionGatedStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PredictionGatedStrategy")
            .field("id", &self.id)
            .field("minimum_probability", &self.minimum_probability)
            .field("maximum_uncertainty", &self.maximum_uncertainty)
            .finish_non_exhaustive()
    }
}

impl PredictionGatedStrategy {
    #[must_use]
    pub fn new(
        id: StrategyId,
        predictor: std::sync::Arc<dyn Predictor>,
        minimum_probability: Decimal,
        maximum_uncertainty: Decimal,
    ) -> Self {
        Self {
            id,
            predictor,
            minimum_probability,
            maximum_uncertainty,
        }
    }
}

#[async_trait]
impl StrategyPort for PredictionGatedStrategy {
    async fn evaluate(&self, event: &MarketEvent) -> Result<Option<Signal>, DomainError> {
        let MarketEvent::Quote(quote) = event else {
            return Ok(None);
        };
        let features = crate::features::quote_momentum_features(quote)?;
        let scored = self.predictor.predict(&features, quote.observed_at).await?;
        let prediction = scored.prediction;
        if prediction.probability < self.minimum_probability
            || prediction.uncertainty > self.maximum_uncertainty
        {
            return Ok(None);
        }

        Ok(Some(Signal {
            id: Uuid::new_v4(),
            strategy_id: self.id.clone(),
            instrument: quote.instrument.clone(),
            side: Side::Buy,
            generated_at: quote.observed_at,
            rationale: "prediction above threshold with bounded uncertainty".to_owned(),
            prediction: Some(prediction),
        }))
    }
}
