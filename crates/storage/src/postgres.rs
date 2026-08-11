use async_trait::async_trait;
use chrono::Utc;
use sqlx::{postgres::PgPoolOptions, PgPool};
use trading_domain::{DomainError, Fill, Order, OrderIntent, RiskDecision, StoragePort};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PostgresStorage {
    pool: PgPool,
}

impl PostgresStorage {
    pub async fn connect(database_url: &str) -> Result<Self, DomainError> {
        PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map(|pool| Self { pool })
            .map_err(storage_error)
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
        sqlx::query(
            "INSERT INTO system_events \
             (event_id, event_type, payload, occurred_at, ingested_at) \
             VALUES ($1, 'market_event', '{}'::jsonb, $2, $2) \
             ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(event_id)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(storage_error)
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
        .map(|_| ())
        .map_err(storage_error)
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
        .map(|_| ())
        .map_err(storage_error)
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
        .map(|_| ())
        .map_err(storage_error)
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
        .map(|_| ())
        .map_err(storage_error)
    }

    async fn order_by_client_id(
        &self,
        client_order_id: Uuid,
    ) -> Result<Option<Order>, DomainError> {
        sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT payload FROM orders WHERE client_order_id = $1",
        )
        .bind(client_order_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?
        .map(serde_json::from_value)
        .transpose()
        .map_err(storage_error)
    }

    async fn orders(&self) -> Result<Vec<Order>, DomainError> {
        deserialize_many(
            sqlx::query_scalar::<_, serde_json::Value>(
                "SELECT payload FROM orders ORDER BY updated_at",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(storage_error)?,
        )
    }

    async fn fills(&self) -> Result<Vec<Fill>, DomainError> {
        deserialize_many(
            sqlx::query_scalar::<_, serde_json::Value>(
                "SELECT payload FROM fills ORDER BY filled_at",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(storage_error)?,
        )
    }
}

fn deserialize_many<T: serde::de::DeserializeOwned>(
    payloads: Vec<serde_json::Value>,
) -> Result<Vec<T>, DomainError> {
    payloads
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()
        .map_err(storage_error)
}

fn storage_error(error: impl std::fmt::Display) -> DomainError {
    DomainError::Storage(error.to_string())
}
