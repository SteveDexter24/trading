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
    orders_by_client: Mutex<HashMap<Uuid, Uuid>>,
    fills: Mutex<HashMap<String, Fill>>,
    kill_switch: Mutex<bool>,
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

    async fn release_event(&self, event_id: &str) -> Result<(), DomainError> {
        self.events.lock().await.remove(event_id);
        Ok(())
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
        self.orders_by_client
            .lock()
            .await
            .insert(order.intent.client_order_id, order.id);
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
        let Some(order_id) = self
            .orders_by_client
            .lock()
            .await
            .get(&client_order_id)
            .copied()
        else {
            return Ok(None);
        };
        Ok(self.orders.lock().await.get(&order_id).cloned())
    }

    async fn orders(&self) -> Result<Vec<Order>, DomainError> {
        Ok(self.orders.lock().await.values().cloned().collect())
    }

    async fn fills(&self) -> Result<Vec<Fill>, DomainError> {
        Ok(self.fills.lock().await.values().cloned().collect())
    }

    async fn risk_decisions(&self) -> Result<Vec<RiskDecision>, DomainError> {
        Ok(self.decisions.lock().await.values().cloned().collect())
    }

    async fn set_kill_switch(&self, active: bool) -> Result<(), DomainError> {
        *self.kill_switch.lock().await = active;
        Ok(())
    }

    async fn kill_switch_active(&self) -> Result<bool, DomainError> {
        Ok(*self.kill_switch.lock().await)
    }
}
