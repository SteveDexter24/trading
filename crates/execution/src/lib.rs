//! Auditable imperative shell around pure domain transitions.

mod engine;
mod outcome;

pub use engine::TradingEngine;
pub use outcome::ExecutionOutcome;

#[cfg(test)]
mod tests;
