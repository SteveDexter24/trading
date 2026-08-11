use async_trait::async_trait;
use rust_decimal::Decimal;
use trading_domain::{DomainError, MarketEvent, Price, Side, Signal, StrategyId, StrategyPort};
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
        Ok(threshold_signal(
            Uuid::new_v4(),
            &self.id,
            event,
            self.maximum_entry_price,
            self.maximum_spread_fraction,
        ))
    }
}

#[must_use]
pub fn threshold_signal(
    signal_id: Uuid,
    strategy_id: &StrategyId,
    event: &MarketEvent,
    maximum_entry_price: Price,
    maximum_spread_fraction: Decimal,
) -> Option<Signal> {
    let MarketEvent::Quote(quote) = event else {
        return None;
    };
    (quote.ask <= maximum_entry_price && quote.spread_fraction() <= maximum_spread_fraction).then(
        || Signal {
            id: signal_id,
            strategy_id: strategy_id.clone(),
            instrument: quote.instrument.clone(),
            side: Side::Buy,
            generated_at: quote.observed_at,
            rationale: "deterministic price and spread threshold".to_owned(),
            prediction: None,
        },
    )
}
