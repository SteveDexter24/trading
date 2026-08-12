use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use trading_domain::ResearchPartition;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkForwardWindow {
    pub partition: ResearchPartition,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkForwardPlan {
    pub windows: Vec<WalkForwardWindow>,
}

/// Build contiguous train/validation/test partitions over an ordered timeline.
pub fn plan_walk_forward(
    timestamps: &[DateTime<Utc>],
    train_share: usize,
    validation_share: usize,
    test_share: usize,
) -> WalkForwardPlan {
    assert!(
        train_share + validation_share + test_share > 0,
        "partition shares must be positive"
    );
    if timestamps.len() < 2 {
        return WalkForwardPlan {
            windows: Vec::new(),
        };
    }

    let total = train_share + validation_share + test_share;
    let n = timestamps.len();
    let train_end = ((n * train_share) / total).max(1).min(n - 1);
    let validation_end = (train_end + ((n * validation_share) / total).max(1)).min(n - 1);

    let mut windows = vec![WalkForwardWindow {
        partition: ResearchPartition::Train,
        start: timestamps[0],
        end: timestamps[train_end],
    }];
    if validation_share > 0 && validation_end > train_end {
        windows.push(WalkForwardWindow {
            partition: ResearchPartition::Validation,
            start: timestamps[train_end],
            end: timestamps[validation_end],
        });
    }
    if test_share > 0 && n - 1 > validation_end {
        windows.push(WalkForwardWindow {
            partition: ResearchPartition::Test,
            start: timestamps[validation_end],
            end: timestamps[n - 1],
        });
    }
    WalkForwardPlan { windows }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn builds_three_partitions() {
        let stamps = (0..10)
            .map(|day| Utc.with_ymd_and_hms(2026, 1, day + 1, 0, 0, 0).unwrap())
            .collect::<Vec<_>>();
        let plan = plan_walk_forward(&stamps, 6, 2, 2);
        assert_eq!(plan.windows.len(), 3);
        assert_eq!(plan.windows[0].partition, ResearchPartition::Train);
        assert_eq!(plan.windows[1].partition, ResearchPartition::Validation);
        assert_eq!(plan.windows[2].partition, ResearchPartition::Test);
    }
}
