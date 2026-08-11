use crate::{
    ApprovedOrder, BrokerReceipt, DomainError, Fill, Instrument, MarketEvent, Order, OrderIntent,
    Quote, RiskContext, RiskDecision, Signal,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

#[async_trait]
pub trait MarketDataPort: Send + Sync {
    async fn latest_quote(&self, instrument: &Instrument) -> Result<Quote, DomainError>;
}

#[async_trait]
pub trait StrategyPort: Send + Sync {
    async fn evaluate(&self, event: &MarketEvent) -> Result<Option<Signal>, DomainError>;
}

#[async_trait]
pub trait RiskEvaluator: Send + Sync {
    async fn evaluate(
        &self,
        intent: &OrderIntent,
        context: &RiskContext,
    ) -> Result<RiskDecision, DomainError>;
}

#[async_trait]
pub trait BrokerPort: Send + Sync {
    async fn submit(&self, order: &ApprovedOrder) -> Result<BrokerReceipt, DomainError>;
}

#[async_trait]
pub trait StoragePort: Send + Sync {
    async fn claim_event(&self, event_id: &str) -> Result<bool, DomainError>;
    async fn persist_intent(&self, intent: &OrderIntent) -> Result<(), DomainError>;
    async fn persist_risk_decision(&self, decision: &RiskDecision) -> Result<(), DomainError>;
    async fn persist_order(&self, order: &Order) -> Result<(), DomainError>;
    async fn persist_fill(&self, fill: &Fill) -> Result<(), DomainError>;
    async fn order_by_client_id(&self, client_order_id: Uuid)
        -> Result<Option<Order>, DomainError>;
    async fn orders(&self) -> Result<Vec<Order>, DomainError>;
    async fn fills(&self) -> Result<Vec<Fill>, DomainError>;
}
