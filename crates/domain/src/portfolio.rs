use crate::{Currency, Instrument, Money, Price, Quantity, StrategyId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
