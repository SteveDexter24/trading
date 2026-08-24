//! Research and prediction contracts.
//!
//! ML may propose signals through these types, but never sizes or approves
//! orders. Training stays offline; the runtime only scores versioned features.

use crate::{DomainError, Instrument, ModelVersion, Prediction};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FeatureVersion(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResearchPartition {
    Train,
    Validation,
    Test,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureSet {
    pub id: Uuid,
    pub instrument: Instrument,
    pub feature_version: FeatureVersion,
    pub values: BTreeMap<String, Decimal>,
    pub observed_at: DateTime<Utc>,
    pub effective_at: DateTime<Utc>,
    pub source: String,
}

impl FeatureSet {
    pub fn validate_point_in_time(&self, decision_time: DateTime<Utc>) -> Result<(), DomainError> {
        (self.observed_at <= decision_time && self.effective_at <= decision_time)
            .then_some(())
            .ok_or(DomainError::InvalidMarketDataOrder)
    }

    pub fn require(&self, name: &str) -> Result<Decimal, DomainError> {
        self.values
            .get(name)
            .copied()
            .ok_or_else(|| DomainError::Adapter(format!("missing feature: {name}")))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoredPrediction {
    pub id: Uuid,
    pub instrument: Instrument,
    pub prediction: Prediction,
    pub feature_set_id: Uuid,
    pub scored_at: DateTime<Utc>,
}

impl From<&ScoredPrediction> for Prediction {
    fn from(value: &ScoredPrediction) -> Self {
        value.prediction.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelArtifact {
    pub name: String,
    pub version: ModelVersion,
    pub feature_version: FeatureVersion,
    pub artifact_digest: String,
    pub training_cutoff_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[async_trait::async_trait]
pub trait Predictor: Send + Sync {
    async fn predict(
        &self,
        features: &FeatureSet,
        decision_time: DateTime<Utc>,
    ) -> Result<ScoredPrediction, DomainError>;
}
