use serde::{Deserialize, Serialize};
use trading_domain::Order;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ExecutionOutcome {
    NoSignal,
    DuplicateEvent,
    RiskRejected {
        intent_id: Uuid,
        reasons: Vec<String>,
    },
    BrokerRejected {
        order_id: Uuid,
        reason: String,
    },
    Filled {
        order: Box<Order>,
    },
}
