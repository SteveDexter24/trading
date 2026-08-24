//! Pure portfolio accounting, construction, and rebalance functions.
//!
//! Construction is deliberately long-only in this iteration. It converts
//! point-in-time alpha estimates into capped target weights; rebalance planning
//! converts those targets into lot-valid proposed trades. Every proposal still
//! requires the execution engine's normal pre-trade risk approval.

mod accounting;
mod construction;
mod model;
mod rebalance;

pub use accounting::cash_debit;
pub use construction::construct_portfolio;
pub use model::{
    AlphaEstimate, ConstructionConstraints, ConstructionMethod, ConstructionRequest,
    GroupConstraint, HoldingSnapshot, PortfolioConstructionError, PortfolioPlan, ProposedTrade,
    RebalanceConstraints, RebalancePlan, TargetAllocation,
};
pub use rebalance::plan_rebalance;

#[cfg(test)]
mod tests;
