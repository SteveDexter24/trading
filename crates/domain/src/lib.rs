//! Pure trading concepts and ports.
//!
//! This crate intentionally has no knowledge of HTTP, databases, Webull, or ML
//! runtimes. Adapters translate external representations into these types.

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("currency mismatch: {left:?} and {right:?}")]
    CurrencyMismatch { left: Currency, right: Currency },
    #[error("amount must not be negative")]
    NegativeAmount,
    #[error("value must be greater than zero")]
    NonPositiveValue,
    #[error("invalid order transition from {from:?} to {to:?}")]
    InvalidOrderTransition { from: OrderStatus, to: OrderStatus },
    #[error("risk decision does not approve this order intent")]
    InvalidRiskApproval,
    #[error("duplicate event: {0}")]
    DuplicateEvent(String),
    #[error("stale or out-of-order market data")]
    InvalidMarketDataOrder,
    #[error("adapter error: {0}")]
    Adapter(String),
    #[error("storage error: {0}")]
    Storage(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Currency {
    Hkd,
    Usd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Exchange {
    Hkex,
    Nasdaq,
    Nyse,
}

impl Exchange {
    #[must_use]
    pub const fn timezone(self) -> &'static str {
        match self {
            Self::Hkex => "Asia/Hong_Kong",
            Self::Nasdaq | Self::Nyse => "America/New_York",
        }
    }

    #[must_use]
    pub const fn currency(self) -> Currency {
        match self {
            Self::Hkex => Currency::Hkd,
            Self::Nasdaq | Self::Nyse => Currency::Usd,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Instrument {
    pub id: Uuid,
    pub symbol: String,
    pub exchange: Exchange,
    pub currency: Currency,
    pub board_lot: Quantity,
    pub tick_size: Price,
    pub fractional_supported: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    #[serde(with = "rust_decimal::serde::str")]
    amount: Decimal,
    currency: Currency,
}

impl Money {
    pub fn new(amount: Decimal, currency: Currency) -> Result<Self, DomainError> {
        if amount.is_sign_negative() {
            Err(DomainError::NegativeAmount)
        } else {
            Ok(Self { amount, currency })
        }
    }

    #[must_use]
    pub const fn zero(currency: Currency) -> Self {
        Self {
            amount: Decimal::ZERO,
            currency,
        }
    }

    #[must_use]
    pub const fn amount(self) -> Decimal {
        self.amount
    }

    #[must_use]
    pub const fn currency(self) -> Currency {
        self.currency
    }

    pub fn checked_add(self, other: Self) -> Result<Self, DomainError> {
        self.ensure_same_currency(other)?;
        self.amount
            .checked_add(other.amount)
            .map(|amount| Self {
                amount,
                currency: self.currency,
            })
            .ok_or_else(|| DomainError::Adapter("money overflow".to_owned()))
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, DomainError> {
        self.ensure_same_currency(other)?;
        self.amount
            .checked_sub(other.amount)
            .filter(|amount| !amount.is_sign_negative())
            .map(|amount| Self {
                amount,
                currency: self.currency,
            })
            .ok_or(DomainError::NegativeAmount)
    }

    pub fn is_at_most(self, other: Self) -> Result<bool, DomainError> {
        self.ensure_same_currency(other)?;
        Ok(self.amount <= other.amount)
    }

    fn ensure_same_currency(self, other: Self) -> Result<(), DomainError> {
        if self.currency == other.currency {
            Ok(())
        } else {
            Err(DomainError::CurrencyMismatch {
                left: self.currency,
                right: other.currency,
            })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Price(#[serde(with = "rust_decimal::serde::str")] pub Decimal);

impl Price {
    pub fn new(value: Decimal) -> Result<Self, DomainError> {
        if value <= Decimal::ZERO {
            Err(DomainError::NonPositiveValue)
        } else {
            Ok(Self(value))
        }
    }

    #[must_use]
    pub const fn value(self) -> Decimal {
        self.0
    }

    pub fn times(self, quantity: Quantity, currency: Currency) -> Result<Money, DomainError> {
        self.0
            .checked_mul(quantity.0)
            .map(|amount| Money { amount, currency })
            .ok_or_else(|| DomainError::Adapter("notional overflow".to_owned()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Quantity(#[serde(with = "rust_decimal::serde::str")] pub Decimal);

impl Quantity {
    pub fn new(value: Decimal) -> Result<Self, DomainError> {
        if value <= Decimal::ZERO {
            Err(DomainError::NonPositiveValue)
        } else {
            Ok(Self(value))
        }
    }

    #[must_use]
    pub const fn value(self) -> Decimal {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    pub event_id: String,
    pub instrument: Instrument,
    pub bid: Price,
    pub ask: Price,
    pub sequence: u64,
    pub occurred_at: DateTime<Utc>,
    pub observed_at: DateTime<Utc>,
    pub trading_date: NaiveDate,
    pub source: String,
}

impl Quote {
    #[must_use]
    pub fn midpoint(&self) -> Price {
        Price((self.bid.0 + self.ask.0) / Decimal::TWO)
    }

    #[must_use]
    pub fn spread_fraction(&self) -> Decimal {
        (self.ask.0 - self.bid.0) / self.midpoint().0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bar {
    pub event_id: String,
    pub instrument: Instrument,
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub volume: Quantity,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub observed_at: DateTime<Utc>,
    pub trading_date: NaiveDate,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum MarketEvent {
    Quote(Quote),
    Bar(Bar),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StrategyId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelVersion(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prediction {
    #[serde(with = "rust_decimal::serde::str")]
    pub probability: Decimal,
    pub horizon_seconds: u64,
    #[serde(with = "rust_decimal::serde::str")]
    pub uncertainty: Decimal,
    pub model_version: ModelVersion,
    pub feature_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signal {
    pub id: Uuid,
    pub strategy_id: StrategyId,
    pub instrument: Instrument,
    pub side: Side,
    pub generated_at: DateTime<Utc>,
    pub rationale: String,
    pub prediction: Option<Prediction>,
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
pub enum TradingMode {
    Paper,
    Live,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskOutcome {
    Approved,
    Rejected { reasons: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskDecision {
    pub id: Uuid,
    pub intent_id: Uuid,
    pub outcome: RiskOutcome,
    pub evaluated_at: DateTime<Utc>,
    pub checks: BTreeMap<String, bool>,
}

impl RiskDecision {
    #[must_use]
    pub fn approved(&self) -> bool {
        matches!(self.outcome, RiskOutcome::Approved)
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
        Ok(Self(Order {
            id: Uuid::new_v4(),
            intent,
            risk_decision_id: decision.id,
            broker_order_id: None,
            status: OrderStatus::RiskApproved,
            submitted_at: None,
            updated_at: now,
        }))
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
    pub fn transition(
        &mut self,
        status: OrderStatus,
        at: DateTime<Utc>,
    ) -> Result<(), DomainError> {
        let valid = matches!(
            (self.status, status),
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
        );
        if !valid {
            return Err(DomainError::InvalidOrderTransition {
                from: self.status,
                to: status,
            });
        }
        self.status = status;
        self.updated_at = at;
        Ok(())
    }
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
pub struct Position {
    pub strategy_id: StrategyId,
    pub instrument: Instrument,
    pub quantity: Quantity,
    pub average_price: Price,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashBalance {
    pub currency: Currency,
    pub settled: Money,
    pub unsettled: Money,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Portfolio {
    pub positions: Vec<Position>,
    pub cash: Vec<CashBalance>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskContext {
    pub mode: TradingMode,
    pub quote: Quote,
    pub market_session_open: bool,
    pub settled_cash: Money,
    pub position_exposure: Money,
    pub strategy_exposure: Money,
    pub portfolio_exposure: Money,
    pub daily_loss: Money,
    pub drawdown: Money,
    pub duplicate_order: bool,
    pub kill_switch_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrokerReceipt {
    pub broker_order_id: String,
    pub acknowledged_at: DateTime<Utc>,
    pub fills: Vec<Fill>,
}

#[async_trait]
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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn rejects_negative_money() {
        assert_eq!(
            Money::new(Decimal::NEGATIVE_ONE, Currency::Usd),
            Err(DomainError::NegativeAmount)
        );
    }

    #[test]
    fn rejects_implicit_fx_arithmetic() {
        let usd = Money::new(Decimal::ONE, Currency::Usd).expect("USD");
        let hkd = Money::new(Decimal::ONE, Currency::Hkd).expect("HKD");
        assert_eq!(
            usd.checked_add(hkd),
            Err(DomainError::CurrencyMismatch {
                left: Currency::Usd,
                right: Currency::Hkd,
            })
        );
    }

    #[test]
    fn calculates_notional_without_floating_point() {
        let price = Price::new(Decimal::new(10_125, 2)).expect("valid price");
        let quantity = Quantity::new(Decimal::new(25, 1)).expect("valid quantity");
        assert_eq!(
            price
                .times(quantity, Currency::Usd)
                .expect("valid notional")
                .amount(),
            Decimal::new(25_3125, 3)
        );
    }

    #[test]
    fn rejects_skipped_order_transition() {
        let now = Utc::now();
        let instrument = Instrument {
            id: Uuid::new_v4(),
            symbol: "AAPL".to_owned(),
            exchange: Exchange::Nasdaq,
            currency: Currency::Usd,
            board_lot: Quantity::new(Decimal::ONE).expect("valid quantity"),
            tick_size: Price::new(Decimal::new(1, 2)).expect("valid price"),
            fractional_supported: false,
        };
        let signal = Signal {
            id: Uuid::new_v4(),
            strategy_id: StrategyId("baseline".to_owned()),
            instrument,
            side: Side::Buy,
            generated_at: now,
            rationale: "test".to_owned(),
            prediction: None,
        };
        let intent =
            OrderIntent::from_signal(&signal, Quantity::new(Decimal::ONE).expect("valid"), now);
        let decision = RiskDecision {
            id: Uuid::new_v4(),
            intent_id: intent.id,
            outcome: RiskOutcome::Approved,
            evaluated_at: now,
            checks: BTreeMap::new(),
        };
        let mut order = ApprovedOrder::new(intent, &decision, now)
            .expect("approved")
            .into_order();
        assert!(matches!(
            order.transition(OrderStatus::Filled, now),
            Err(DomainError::InvalidOrderTransition { .. })
        ));
    }
}
