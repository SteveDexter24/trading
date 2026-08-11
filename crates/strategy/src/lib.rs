//! Deterministic strategy baselines.

use async_trait::async_trait;
use rust_decimal::Decimal;
use trading_domain::{
    DomainError, MarketEvent, Price, Side, Signal, StrategyId, StrategyPort,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PriceThresholdStrategy {
    id: StrategyId,
    maximum_entry_price: Price,
    maximum_spread_fraction: Decimal,
}

impl PriceThresholdStrategy {
    #[must_use]
    pub const fn new(
        id: StrategyId,
        maximum_entry_price: Price,
        maximum_spread_fraction: Decimal,
    ) -> Self {
        Self {
            id,
            maximum_entry_price,
            maximum_spread_fraction,
        }
    }
}

#[async_trait]
impl StrategyPort for PriceThresholdStrategy {
    async fn evaluate(&self, event: &MarketEvent) -> Result<Option<Signal>, DomainError> {
        let MarketEvent::Quote(quote) = event else {
            return Ok(None);
        };
        if quote.ask > self.maximum_entry_price
            || quote.spread_fraction() > self.maximum_spread_fraction
        {
            return Ok(None);
        }

        Ok(Some(Signal {
            id: Uuid::new_v4(),
            strategy_id: self.id.clone(),
            instrument: quote.instrument.clone(),
            side: Side::Buy,
            generated_at: quote.observed_at,
            rationale: "deterministic price and spread threshold".to_owned(),
            prediction: None,
        }))
    }
}
