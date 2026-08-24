//! ONNX inference boundary.
//!
//! Offline Python training exports versioned ONNX artifacts. This runtime
//! adapter intentionally fails closed until an artifact and runtime are
//! configured. Training never happens inside the trading binary.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use trading_domain::{
    DomainError, FeatureSet, FeatureVersion, ModelArtifact, ModelVersion, Predictor,
    ScoredPrediction,
};

#[derive(Debug, Clone)]
pub struct OnnxModelConfig {
    pub artifact_path: PathBuf,
    pub artifact: ModelArtifact,
}

#[derive(Debug, Clone)]
pub struct OnnxPredictor {
    config: OnnxModelConfig,
}

impl OnnxPredictor {
    pub fn new(config: OnnxModelConfig) -> Result<Self, DomainError> {
        if !config.artifact_path.exists() {
            return Err(DomainError::Adapter(format!(
                "ONNX artifact missing: {}",
                config.artifact_path.display()
            )));
        }
        Ok(Self { config })
    }

    #[must_use]
    pub fn model_version(&self) -> &ModelVersion {
        &self.config.artifact.version
    }

    #[must_use]
    pub fn feature_version(&self) -> &FeatureVersion {
        &self.config.artifact.feature_version
    }
}

#[async_trait]
impl Predictor for OnnxPredictor {
    async fn predict(
        &self,
        features: &FeatureSet,
        decision_time: DateTime<Utc>,
    ) -> Result<ScoredPrediction, DomainError> {
        features.validate_point_in_time(decision_time)?;
        if features.feature_version != self.config.artifact.feature_version {
            return Err(DomainError::Adapter(
                "feature version mismatch for ONNX model".to_owned(),
            ));
        }
        Err(DomainError::Adapter(
            "ONNX runtime execution is disabled until an approved artifact and runtime are wired; use DeterministicResearchModel for research scaffolding".to_owned(),
        ))
    }
}

pub fn production_training_disabled() -> Result<(), DomainError> {
    Err(DomainError::Adapter(
        "model training is offline-only and disabled inside the trading runtime".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn training_cannot_run_in_runtime() {
        assert!(production_training_disabled().is_err());
    }

    #[test]
    fn missing_artifact_fails_closed() {
        let config = OnnxModelConfig {
            artifact_path: PathBuf::from("/tmp/does-not-exist-trading.onnx"),
            artifact: ModelArtifact {
                name: "baseline".to_owned(),
                version: ModelVersion("v0".to_owned()),
                feature_version: FeatureVersion("quote-momentum-v1".to_owned()),
                artifact_digest: "sha256:none".to_owned(),
                training_cutoff_at: Utc::now(),
                created_at: Utc::now(),
            },
        };
        assert!(OnnxPredictor::new(config).is_err());
    }
}
