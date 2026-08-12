//! Research and backtesting core.
//!
//! Reuses strategy and risk ports with an injected simulation clock. ML may
//! propose signals through predictors, but never sizes or approves orders.

mod backtest;
mod clock;
mod metrics;
mod walk_forward;

pub use backtest::{BacktestConfig, BacktestResult, Backtester};
pub use clock::{SharedClock, SimulationClock};
pub use metrics::{summarize_equity_curve, EquityPoint, PerformanceReport};
pub use walk_forward::{plan_walk_forward, WalkForwardPlan, WalkForwardWindow};
