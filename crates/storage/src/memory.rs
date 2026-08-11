use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use tokio::sync::Mutex;
use trading_domain::{DomainError, Fill, Order, OrderIntent, RiskDecision, StoragePort};
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct InMemoryStorage {
    events: Mutex<HashSet<String>>,
    intents: Mutex<HashMap<Uuid, OrderIntent>>,
    decisions: Mutex<HashMap<Uuid, RiskDecision>>,
    orders: Mutex<HashMap<Uuid, Order>>,
    fills: Mutex<HashMap<String, Fill>>,
}

impl InMemoryStorage {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl StoragePort for InMemoryStorage {
    async fn claim_event(&self, event_id: &str) -> Result<bool, DomainError> {
        Ok(self.events.lock().await.insert(event_id.to_owned()))
    }

    async fn persist_intent(&self, intent: &OrderIntent) -> Result<(), DomainError> {
        self.intents.lock().await.insert(intent.id, intent.clone());
        Ok(())
    }

    async fn persist_risk_decision(&self, decision: &RiskDecision) -> Result<(), DomainError> {
        self.decisions
            .lock()
            .await
            .insert(decision.id, decision.clone());
        Ok(())
    }

    async fn persist_order(&self, order: &Order) -> Result<(), DomainError> {
        self.orders.lock().await.insert(order.id, order.clone());
        Ok(())
    }

    async fn persist_fill(&self, fill: &Fill) -> Result<(), DomainError> {
        self.fills
            .lock()
            .await
            .entry(fill.event_id.clone())
            .or_insert_with(|| fill.clone());
        Ok(())
    }

    async fn order_by_client_id(
        &self,
        client_order_id: Uuid,
    ) -> Result<Option<Order>, DomainError> {
        Ok(self
            .orders
            .lock()
            .await
            .values()
            .find(|order| order.intent.client_order_id == client_order_id)
            .cloned())
    }

    async fn orders(&self) -> Result<Vec<Order>, DomainError> {
        Ok(self.orders.lock().await.values().cloned().collect())
    }

    async fn fills(&self) -> Result<Vec<Fill>, DomainError> {
        Ok(self.fills.lock().await.values().cloned().collect())
    }
}
