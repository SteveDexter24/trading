//! Deterministic broker simulator with pure fill planning.

mod simulator;

pub use simulator::{plan_fills, ExecutionModel, FillPolicy, PaperBroker, PlannedFill};
