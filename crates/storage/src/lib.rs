//! Storage adapters. PostgreSQL is the durable runtime adapter; the in-memory
//! implementation is used by deterministic tests and local demonstrations.

use async_trait::async_trait;
use chrono::Utc;
use sqlx::{postgres::PgPoolOptions, PgPool};
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

#[derive(Debug, Clone)]
pub struct PostgresStorage {
    pool: PgPool,
}

impl PostgresStorage {
    pub async fn connect(database_url: &str) -> Result<Self, DomainError> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(storage_error)?;
        Ok(Self { pool })
    }

    #[must_use]
    pub const fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn migrate(&self) -> Result<(), DomainError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(storage_error)
    }

    fn json<T: serde::Serialize>(value: &T) -> Result<serde_json::Value, DomainError> {
        serde_json::to_value(value).map_err(storage_error)
    }
}

#[async_trait]
impl StoragePort for PostgresStorage {
    async fn claim_event(&self, event_id: &str) -> Result<bool, DomainError> {
        let result = sqlx::query(
            "INSERT INTO system_events \
             (event_id, event_type, payload, occurred_at, ingested_at) \
             VALUES ($1, 'market_event', '{}'::jsonb, $2, $2) \
             ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(event_id)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(storage_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn persist_intent(&self, intent: &OrderIntent) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO order_intents \
             (id, client_order_id, strategy_id, instrument_id, created_at, payload) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(intent.id)
        .bind(intent.client_order_id)
        .bind(&intent.strategy_id.0)
        .bind(intent.instrument.id)
        .bind(intent.created_at)
        .bind(Self::json(intent)?)
        .execute(&self.pool)
        .await
        .map_err(storage_error)?;
        Ok(())
    }

    async fn persist_risk_decision(&self, decision: &RiskDecision) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO risk_decisions \
             (id, order_intent_id, approved, evaluated_at, payload) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(decision.id)
        .bind(decision.intent_id)
        .bind(decision.approved())
        .bind(decision.evaluated_at)
        .bind(Self::json(decision)?)
        .execute(&self.pool)
        .await
        .map_err(storage_error)?;
        Ok(())
    }

    async fn persist_order(&self, order: &Order) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO orders \
             (id, client_order_id, order_intent_id, risk_decision_id, status, \
              broker_order_id, submitted_at, updated_at, payload) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
             ON CONFLICT (id) DO UPDATE SET status = EXCLUDED.status, \
              broker_order_id = EXCLUDED.broker_order_id, \
              submitted_at = EXCLUDED.submitted_at, updated_at = EXCLUDED.updated_at, \
              payload = EXCLUDED.payload",
        )
        .bind(order.id)
        .bind(order.intent.client_order_id)
        .bind(order.intent.id)
        .bind(order.risk_decision_id)
        .bind(format!("{:?}", order.status))
        .bind(&order.broker_order_id)
        .bind(order.submitted_at)
        .bind(order.updated_at)
        .bind(Self::json(order)?)
        .execute(&self.pool)
        .await
        .map_err(storage_error)?;
        Ok(())
    }

    async fn persist_fill(&self, fill: &Fill) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO fills (id, event_id, order_id, filled_at, payload) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(fill.id)
        .bind(&fill.event_id)
        .bind(fill.order_id)
        .bind(fill.filled_at)
        .bind(Self::json(fill)?)
        .execute(&self.pool)
        .await
        .map_err(storage_error)?;
        Ok(())
    }

    async fn order_by_client_id(
        &self,
        client_order_id: Uuid,
    ) -> Result<Option<Order>, DomainError> {
        let payload = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT payload FROM orders WHERE client_order_id = $1",
        )
        .bind(client_order_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?;
        payload
            .map(serde_json::from_value)
            .transpose()
            .map_err(storage_error)
    }

    async fn orders(&self) -> Result<Vec<Order>, DomainError> {
        let payloads = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT payload FROM orders ORDER BY updated_at",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage_error)?;
        payloads
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<_, _>>()
            .map_err(storage_error)
    }

    async fn fills(&self) -> Result<Vec<Fill>, DomainError> {
        let payloads = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT payload FROM fills ORDER BY filled_at",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage_error)?;
        payloads
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<_, _>>()
            .map_err(storage_error)
    }
}

fn storage_error(error: impl std::fmt::Display) -> DomainError {
    DomainError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn claims_each_event_once_across_engine_instances() {
        let storage = InMemoryStorage::new();
        assert!(storage.claim_event("source-123").await.expect("claim"));
        assert!(!storage.claim_event("source-123").await.expect("duplicate"));
    }
}
