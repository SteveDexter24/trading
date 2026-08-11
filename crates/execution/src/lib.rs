//! Auditable order orchestration.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use trading_domain::{
    ApprovedOrder, BrokerPort, Clock, DomainError, MarketEvent, Order, OrderIntent, OrderStatus,
    Quantity, RiskContext, RiskEvaluator, RiskOutcome, StoragePort, StrategyPort,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ExecutionOutcome {
    NoSignal,
    DuplicateEvent,
    RiskRejected {
        intent_id: Uuid,
        reasons: Vec<String>,
    },
    BrokerRejected {
        order_id: Uuid,
        reason: String,
    },
    Filled {
        order: Box<Order>,
    },
}

pub struct TradingEngine {
    strategy: Arc<dyn StrategyPort>,
    risk: Arc<dyn RiskEvaluator>,
    broker: Arc<dyn BrokerPort>,
    storage: Arc<dyn StoragePort>,
    clock: Arc<dyn Clock>,
    order_quantity: Quantity,
}

impl TradingEngine {
    #[must_use]
    pub fn new(
        strategy: Arc<dyn StrategyPort>,
        risk: Arc<dyn RiskEvaluator>,
        broker: Arc<dyn BrokerPort>,
        storage: Arc<dyn StoragePort>,
        clock: Arc<dyn Clock>,
        order_quantity: Quantity,
    ) -> Self {
        Self {
            strategy,
            risk,
            broker,
            storage,
            clock,
            order_quantity,
        }
    }

    pub async fn process(
        &self,
        event: &MarketEvent,
        context: &RiskContext,
    ) -> Result<ExecutionOutcome, DomainError> {
        let event_id = match event {
            MarketEvent::Quote(quote) => &quote.event_id,
            MarketEvent::Bar(bar) => &bar.event_id,
        };
        if !self.storage.claim_event(event_id).await? {
            return Ok(ExecutionOutcome::DuplicateEvent);
        }
        let Some(signal) = self.strategy.evaluate(event).await? else {
            return Ok(ExecutionOutcome::NoSignal);
        };

        let intent = OrderIntent::from_signal(&signal, self.order_quantity, self.clock.now());
        self.storage.persist_intent(&intent).await?;

        let decision = self.risk.evaluate(&intent, context).await?;
        self.storage.persist_risk_decision(&decision).await?;
        if let RiskOutcome::Rejected { reasons } = decision.outcome {
            return Ok(ExecutionOutcome::RiskRejected {
                intent_id: intent.id,
                reasons,
            });
        }

        let approved = ApprovedOrder::new(intent, &decision, self.clock.now())?;
        let mut order = approved.order().clone();
        self.storage.persist_order(&order).await?;

        order.transition(OrderStatus::Submitted, self.clock.now())?;
        order.submitted_at = Some(self.clock.now());
        self.storage.persist_order(&order).await?;

        let receipt = match self.broker.submit(&approved).await {
            Ok(receipt) => receipt,
            Err(error) => {
                order.transition(OrderStatus::Rejected, self.clock.now())?;
                self.storage.persist_order(&order).await?;
                return Ok(ExecutionOutcome::BrokerRejected {
                    order_id: order.id,
                    reason: error.to_string(),
                });
            }
        };
        order.broker_order_id = Some(receipt.broker_order_id);
        order.transition(OrderStatus::Acknowledged, receipt.acknowledged_at)?;
        self.storage.persist_order(&order).await?;

        let fill_count = receipt.fills.len();
        for (index, fill) in receipt.fills.into_iter().enumerate() {
            self.storage.persist_fill(&fill).await?;
            let status = if index + 1 == fill_count {
                OrderStatus::Filled
            } else {
                OrderStatus::PartiallyFilled
            };
            order.transition(status, fill.filled_at)?;
            self.storage.persist_order(&order).await?;
        }

        Ok(ExecutionOutcome::Filled {
            order: Box::new(order),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
    use rust_decimal::Decimal;
    use std::time::Duration;
    use trading_domain::{
        Currency, Exchange, Instrument, Money, Price, Quote, StrategyId, TradingMode,
    };
    use trading_paper_broker::{ExecutionModel, FillPolicy, PaperBroker};
    use trading_risk::{RiskLimits, RuleBasedRisk, SystemClock};
    use trading_storage::InMemoryStorage;
    use trading_strategy::PriceThresholdStrategy;

    fn fixture() -> (MarketEvent, RiskContext, Instrument) {
        let now = Utc::now();
        let instrument = Instrument {
            id: Uuid::new_v4(),
            symbol: "AAPL".to_owned(),
            exchange: Exchange::Nasdaq,
            currency: Currency::Usd,
            board_lot: Quantity::new(Decimal::ONE).expect("lot"),
            tick_size: Price::new(Decimal::new(1, 2)).expect("tick"),
            fractional_supported: false,
        };
        let quote = Quote {
            event_id: "synthetic-quote-1".to_owned(),
            instrument: instrument.clone(),
            bid: Price::new(Decimal::new(19_995, 2)).expect("bid"),
            ask: Price::new(Decimal::new(20_000, 2)).expect("ask"),
            sequence: 1,
            occurred_at: now,
            observed_at: now,
            trading_date: NaiveDate::from_ymd_opt(2026, 8, 11).expect("date"),
            source: "synthetic".to_owned(),
        };
        let context = RiskContext {
            mode: TradingMode::Paper,
            quote: quote.clone(),
            market_session_open: true,
            settled_cash: Money::new(Decimal::new(100_000, 0), Currency::Usd).expect("cash"),
            position_exposure: Money::zero(Currency::Usd),
            strategy_exposure: Money::zero(Currency::Usd),
            portfolio_exposure: Money::zero(Currency::Usd),
            daily_loss: Money::zero(Currency::Usd),
            drawdown: Money::zero(Currency::Usd),
            duplicate_order: false,
            kill_switch_active: false,
        };
        (MarketEvent::Quote(quote), context, instrument)
    }

    fn engine(
        storage: Arc<InMemoryStorage>,
        instrument: &Instrument,
        broker: Arc<PaperBroker>,
    ) -> TradingEngine {
        let strategy = PriceThresholdStrategy::new(
            StrategyId("short-term-baseline".to_owned()),
            Price::new(Decimal::new(25_000, 2)).expect("maximum price"),
            Decimal::new(1, 2),
        );
        let limits = RiskLimits {
            maximum_quote_age: ChronoDuration::seconds(5),
            maximum_order_value: Money::new(Decimal::new(5_000, 0), Currency::Usd).expect("limit"),
            maximum_position_exposure: Money::new(Decimal::new(10_000, 0), Currency::Usd)
                .expect("limit"),
            maximum_strategy_exposure: Money::new(Decimal::new(25_000, 0), Currency::Usd)
                .expect("limit"),
            maximum_portfolio_exposure: Money::new(Decimal::new(100_000, 0), Currency::Usd)
                .expect("limit"),
            maximum_daily_loss: Money::new(Decimal::new(5_000, 0), Currency::Usd).expect("limit"),
            maximum_drawdown: Money::new(Decimal::new(10_000, 0), Currency::Usd).expect("limit"),
            maximum_spread_fraction: Decimal::new(1, 2),
        };
        TradingEngine::new(
            Arc::new(strategy),
            Arc::new(RuleBasedRisk::new(
                [instrument.id],
                limits,
                Box::new(SystemClock),
            )),
            broker,
            storage,
            Arc::new(SystemClock),
            Quantity::new(Decimal::ONE).expect("quantity"),
        )
    }

    #[tokio::test]
    async fn synthetic_event_completes_auditable_paper_flow() {
        let (event, context, instrument) = fixture();
        let storage = Arc::new(InMemoryStorage::new());
        let broker = Arc::new(PaperBroker::new(ExecutionModel {
            price: context.quote.ask,
            currency: Currency::Usd,
            fee: Money::new(Decimal::new(1, 0), Currency::Usd).expect("fee"),
            latency: Duration::ZERO,
            fill_policy: FillPolicy::PartialThenFull,
        }));

        let result = engine(Arc::clone(&storage), &instrument, broker)
            .process(&event, &context)
            .await
            .expect("flow");
        let ExecutionOutcome::Filled { order } = result else {
            panic!("expected filled order");
        };
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(storage.orders().await.expect("orders").len(), 1);
        assert_eq!(storage.fills().await.expect("fills").len(), 2);
    }

    #[tokio::test]
    async fn restart_does_not_duplicate_simulated_order() {
        let (event, context, instrument) = fixture();
        let storage = Arc::new(InMemoryStorage::new());
        let broker = Arc::new(PaperBroker::new(ExecutionModel {
            price: context.quote.ask,
            currency: Currency::Usd,
            fee: Money::zero(Currency::Usd),
            latency: Duration::ZERO,
            fill_policy: FillPolicy::Full,
        }));
        let first = engine(Arc::clone(&storage), &instrument, Arc::clone(&broker));
        assert!(matches!(
            first.process(&event, &context).await.expect("first"),
            ExecutionOutcome::Filled { .. }
        ));
        drop(first);

        let restarted = engine(Arc::clone(&storage), &instrument, broker);
        assert_eq!(
            restarted.process(&event, &context).await.expect("replay"),
            ExecutionOutcome::DuplicateEvent
        );
        assert_eq!(storage.orders().await.expect("orders").len(), 1);
    }

    #[tokio::test]
    async fn risk_rejection_never_reaches_broker() {
        let (event, mut context, instrument) = fixture();
        context.mode = TradingMode::Live;
        let storage = Arc::new(InMemoryStorage::new());
        let broker = Arc::new(PaperBroker::new(ExecutionModel {
            price: context.quote.ask,
            currency: Currency::Usd,
            fee: Money::zero(Currency::Usd),
            latency: Duration::ZERO,
            fill_policy: FillPolicy::Full,
        }));
        let result = engine(Arc::clone(&storage), &instrument, broker)
            .process(&event, &context)
            .await
            .expect("rejection");
        assert!(matches!(result, ExecutionOutcome::RiskRejected { .. }));
        assert!(storage.orders().await.expect("orders").is_empty());
    }
}
