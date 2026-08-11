use crate::{Currency, OrderStatus};
use thiserror::Error;

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
