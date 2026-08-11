use crate::{Instrument, Side};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StrategyId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelVersion(pub String);

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
