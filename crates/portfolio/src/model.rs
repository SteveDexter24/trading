use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use trading_domain::{Currency, DomainError, Instrument, Money, Price, Quantity, Side};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PortfolioConstructionError {
    #[error("invalid portfolio constraint: {0}")]
    InvalidConstraint(String),
    #[error("invalid alpha estimate for {symbol}: {reason}")]
    InvalidEstimate { symbol: String, reason: String },
    #[error("duplicate instrument in portfolio input: {0}")]
    DuplicateInstrument(String),
    #[error("missing reference price for current holding: {0}")]
    MissingReferencePrice(String),
    #[error("portfolio arithmetic failed: {0}")]
    Arithmetic(String),
    #[error(transparent)]
    Domain(#[from] DomainError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstructionMethod {
    EqualWeight,
    InverseVolatility,
    RiskAdjustedAlpha,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlphaEstimate {
    pub instrument: Instrument,
    pub reference_price: Price,
    #[serde(with = "rust_decimal::serde::str")]
    pub expected_excess_return: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub forecast_volatility: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub confidence: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub liquidity_score: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub maximum_weight: Decimal,
    pub risk_group: Option<String>,
    pub forecast_horizon_seconds: u64,
    pub source_version: String,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupConstraint {
    pub group: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub maximum_weight: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstructionConstraints {
    #[serde(with = "rust_decimal::serde::str")]
    pub target_invested_weight: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub maximum_position_weight: Decimal,
    pub group_constraints: Vec<GroupConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstructionRequest {
    pub as_of: DateTime<Utc>,
    pub total_equity: Money,
    pub method: ConstructionMethod,
    pub constraints: ConstructionConstraints,
    pub estimates: Vec<AlphaEstimate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetAllocation {
    pub instrument: Instrument,
    pub reference_price: Price,
    #[serde(with = "rust_decimal::serde::str")]
    pub target_weight: Decimal,
    pub target_value: Money,
    #[serde(with = "rust_decimal::serde::str")]
    pub construction_score: Decimal,
    pub source_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortfolioPlan {
    pub as_of: DateTime<Utc>,
    pub method: ConstructionMethod,
    pub total_equity: Money,
    #[serde(with = "rust_decimal::serde::str")]
    pub allocated_weight: Decimal,
    pub unallocated_cash: Money,
    pub allocations: Vec<TargetAllocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HoldingSnapshot {
    pub instrument: Instrument,
    pub quantity: Quantity,
    pub reference_price: Price,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebalanceConstraints {
    pub minimum_trade_notional: Money,
    #[serde(with = "rust_decimal::serde::str")]
    pub maximum_turnover_weight: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedTrade {
    pub instrument: Instrument,
    pub side: Side,
    pub quantity: Quantity,
    pub reference_price: Price,
    pub estimated_notional: Money,
    #[serde(with = "rust_decimal::serde::str")]
    pub target_weight: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebalancePlan {
    pub as_of: DateTime<Utc>,
    pub currency: Currency,
    #[serde(with = "rust_decimal::serde::str")]
    pub turnover_weight: Decimal,
    pub estimated_turnover: Money,
    pub trades: Vec<ProposedTrade>,
}

pub(crate) fn validate_unit_interval(
    value: Decimal,
    name: &str,
) -> Result<(), PortfolioConstructionError> {
    if value < Decimal::ZERO || value > Decimal::ONE {
        return Err(PortfolioConstructionError::InvalidConstraint(format!(
            "{name} must be between zero and one"
        )));
    }
    Ok(())
}

pub(crate) fn checked_mul(
    left: Decimal,
    right: Decimal,
    context: &str,
) -> Result<Decimal, PortfolioConstructionError> {
    left.checked_mul(right)
        .ok_or_else(|| PortfolioConstructionError::Arithmetic(context.to_owned()))
}

pub(crate) fn checked_add(
    left: Decimal,
    right: Decimal,
    context: &str,
) -> Result<Decimal, PortfolioConstructionError> {
    left.checked_add(right)
        .ok_or_else(|| PortfolioConstructionError::Arithmetic(context.to_owned()))
}
