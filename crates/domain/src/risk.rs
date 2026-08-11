use crate::{Money, OrderIntent, Quote};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

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

impl RiskContext {
    #[must_use]
    pub const fn for_intent(&self, intent: &OrderIntent) -> bool {
        self.quote.instrument.id == intent.instrument.id
    }
}
