use crate::ExecutionOutcome;
use std::sync::Arc;
use trading_domain::{
    ApprovedOrder, BrokerPort, BrokerReceipt, Clock, DomainError, MarketEvent, OrderIntent,
    OrderStatus, Quantity, RiskContext, RiskEvaluator, RiskOutcome, StoragePort, StrategyPort,
};

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
        if !self.storage.claim_event(event.event_id()).await? {
            return Ok(ExecutionOutcome::DuplicateEvent);
        }
        let Some(signal) = self.strategy.evaluate(event).await? else {
            return Ok(ExecutionOutcome::NoSignal);
        };

        let intent = OrderIntent::from_signal(&signal, self.order_quantity, self.clock.now());
        self.storage.persist_intent(&intent).await?;

        let decision = self.risk.evaluate(&intent, context).await?;
        self.storage.persist_risk_decision(&decision).await?;
        if let RiskOutcome::Rejected { reasons } = &decision.outcome {
            return Ok(ExecutionOutcome::RiskRejected {
                intent_id: intent.id,
                reasons: reasons.clone(),
            });
        }

        let approved = ApprovedOrder::new(intent, &decision, self.clock.now())?;
        self.storage.persist_order(approved.order()).await?;

        let order = approved.order().clone().submitted(self.clock.now())?;
        self.storage.persist_order(&order).await?;

        let receipt = match self.broker.submit(&approved).await {
            Ok(receipt) => receipt,
            Err(error) => {
                let rejected = order.transition(OrderStatus::Rejected, self.clock.now())?;
                self.storage.persist_order(&rejected).await?;
                return Ok(ExecutionOutcome::BrokerRejected {
                    order_id: rejected.id,
                    reason: error.to_string(),
                });
            }
        };

        self.persist_receipt(order, receipt).await
    }

    async fn persist_receipt(
        &self,
        order: trading_domain::Order,
        receipt: BrokerReceipt,
    ) -> Result<ExecutionOutcome, DomainError> {
        let BrokerReceipt {
            broker_order_id,
            acknowledged_at,
            fills,
        } = receipt;
        let mut current = order.acknowledged(broker_order_id, acknowledged_at)?;
        self.storage.persist_order(&current).await?;

        let fill_count = fills.len();
        for (index, fill) in fills.into_iter().enumerate() {
            self.storage.persist_fill(&fill).await?;
            let status = if index + 1 == fill_count {
                OrderStatus::Filled
            } else {
                OrderStatus::PartiallyFilled
            };
            current = current.transition(status, fill.filled_at)?;
            self.storage.persist_order(&current).await?;
        }

        Ok(ExecutionOutcome::Filled {
            order: Box::new(current),
        })
    }
}
