//! Pure trading concepts and ports.
//!
//! This crate intentionally has no knowledge of HTTP, databases, Webull, or ML
//! runtimes. Modules expose immutable values and pure state transitions;
//! adapters translate external representations at the workspace boundary.

mod error;
mod instrument;
mod market;
mod numeric;
mod order;
mod portfolio;
mod ports;
mod research;
mod risk;
mod strategy;

pub use error::DomainError;
pub use instrument::{Currency, Exchange, Instrument};
pub use market::{Bar, MarketEvent, Quote};
pub use numeric::{Money, Price, Quantity};
pub use order::{
    can_transition, ApprovedOrder, BrokerReceipt, Fill, Order, OrderIntent, OrderStatus, OrderType,
    Side,
};
pub use portfolio::{CashBalance, Portfolio, Position};
pub use ports::{BrokerPort, Clock, MarketDataPort, RiskEvaluator, StoragePort, StrategyPort};
pub use research::{
    FeatureSet, FeatureVersion, ModelArtifact, Predictor, ResearchPartition, ScoredPrediction,
};
pub use risk::{RiskContext, RiskDecision, RiskOutcome, TradingMode};
pub use strategy::{ModelVersion, Prediction, Signal, StrategyId};

#[cfg(test)]
mod tests;
