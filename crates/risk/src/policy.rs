use crate::{rules::evaluate_checks, RiskLimits};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashSet};
use trading_domain::{
    Clock, DomainError, OrderIntent, RiskContext, RiskDecision, RiskEvaluator, RiskOutcome,
};
use uuid::Uuid;

pub struct RuleBasedRisk {
    allowed_instruments: HashSet<Uuid>,
    limits: RiskLimits,
    clock: Box<dyn Clock>,
}

impl RuleBasedRisk {
    #[must_use]
    pub fn new(
        allowed_instruments: impl IntoIterator<Item = Uuid>,
        limits: RiskLimits,
        clock: Box<dyn Clock>,
    ) -> Self {
        Self {
            allowed_instruments: allowed_instruments.into_iter().collect(),
            limits,
            clock,
        }
    }
}

#[async_trait]
impl RiskEvaluator for RuleBasedRisk {
    async fn evaluate(
        &self,
        intent: &OrderIntent,
        context: &RiskContext,
    ) -> Result<RiskDecision, DomainError> {
        let now = self.clock.now();
        evaluate_checks(intent, context, self.limits, &self.allowed_instruments, now)
            .map(|checks| build_decision(Uuid::new_v4(), intent.id, now, checks))
    }
}

fn build_decision(
    id: Uuid,
    intent_id: Uuid,
    evaluated_at: DateTime<Utc>,
    checks: BTreeMap<String, bool>,
) -> RiskDecision {
    let reasons = checks
        .iter()
        .filter_map(|(name, passed)| (!passed).then_some(name.clone()))
        .collect::<Vec<_>>();
    let outcome = if reasons.is_empty() {
        RiskOutcome::Approved
    } else {
        RiskOutcome::Rejected { reasons }
    };

    RiskDecision {
        id,
        intent_id,
        outcome,
        evaluated_at,
        checks,
    }
}
