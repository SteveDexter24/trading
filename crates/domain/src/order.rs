use crate::{
    Currency, DomainError, Instrument, Money, Price, Quantity, RiskDecision, Signal, StrategyId,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit(Price),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderIntent {
    pub id: Uuid,
    pub client_order_id: Uuid,
    pub strategy_id: StrategyId,
    pub instrument: Instrument,
    pub side: Side,
    pub quantity: Quantity,
    pub order_type: OrderType,
    pub created_at: DateTime<Utc>,
}

impl OrderIntent {
    #[must_use]
    pub fn from_signal(signal: &Signal, quantity: Quantity, created_at: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            client_order_id: Uuid::new_v4(),
            strategy_id: signal.strategy_id.clone(),
            instrument: signal.instrument.clone(),
            side: signal.side,
            quantity,
            order_type: OrderType::Market,
            created_at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Created,
    RiskApproved,
    Submitted,
    Acknowledged,
    PartiallyFilled,
    Filled,
    Rejected,
    Cancelled,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub intent: OrderIntent,
    pub risk_decision_id: Uuid,
    pub broker_order_id: Option<String>,
    pub status: OrderStatus,
    pub submitted_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedOrder(Order);

impl ApprovedOrder {
    pub fn new(
        intent: OrderIntent,
        decision: &RiskDecision,
        now: DateTime<Utc>,
    ) -> Result<Self, DomainError> {
        if decision.intent_id != intent.id || !decision.approved() {
            return Err(DomainError::InvalidRiskApproval);
        }
        let created = Order {
            id: Uuid::new_v4(),
            intent,
            risk_decision_id: decision.id,
            broker_order_id: None,
            status: OrderStatus::Created,
            submitted_at: None,
            updated_at: now,
        };
        created.transition(OrderStatus::RiskApproved, now).map(Self)
    }

    #[must_use]
    pub const fn order(&self) -> &Order {
        &self.0
    }

    #[must_use]
    pub fn into_order(self) -> Order {
        self.0
    }
}

impl Order {
    pub fn transition(self, status: OrderStatus, at: DateTime<Utc>) -> Result<Self, DomainError> {
        can_transition(self.status, status)
            .then_some(Self {
                status,
                updated_at: at,
                ..self
            })
            .ok_or(DomainError::InvalidOrderTransition {
                from: self.status,
                to: status,
            })
    }

    pub fn submitted(self, at: DateTime<Utc>) -> Result<Self, DomainError> {
        self.transition(OrderStatus::Submitted, at)
            .map(|order| Self {
                submitted_at: Some(at),
                ..order
            })
    }

    pub fn acknowledged(
        self,
        broker_order_id: String,
        at: DateTime<Utc>,
    ) -> Result<Self, DomainError> {
        self.transition(OrderStatus::Acknowledged, at)
            .map(|order| Self {
                broker_order_id: Some(broker_order_id),
                ..order
            })
    }
}

#[must_use]
pub const fn can_transition(from: OrderStatus, to: OrderStatus) -> bool {
    matches!(
        (from, to),
        (OrderStatus::Created, OrderStatus::RiskApproved)
            | (OrderStatus::RiskApproved, OrderStatus::Submitted)
            | (OrderStatus::Submitted, OrderStatus::Acknowledged)
            | (
                OrderStatus::Acknowledged | OrderStatus::PartiallyFilled,
                OrderStatus::PartiallyFilled | OrderStatus::Filled
            )
            | (
                OrderStatus::Created
                    | OrderStatus::RiskApproved
                    | OrderStatus::Submitted
                    | OrderStatus::Acknowledged
                    | OrderStatus::PartiallyFilled,
                OrderStatus::Rejected | OrderStatus::Cancelled | OrderStatus::Expired
            )
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fill {
    pub id: Uuid,
    pub event_id: String,
    pub order_id: Uuid,
    pub quantity: Quantity,
    pub price: Price,
    pub fee: Money,
    pub currency: Currency,
    pub filled_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrokerReceipt {
    pub broker_order_id: String,
    pub acknowledged_at: DateTime<Utc>,
    pub fills: Vec<Fill>,
}
