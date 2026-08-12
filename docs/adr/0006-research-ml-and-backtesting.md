# ADR 0006: Research, ML, and backtesting boundary

Status: Accepted

## Context

A paper execution engine is not enough for a quant workflow. Researchers need
point-in-time features, versioned predictions, walk-forward evaluation, and a
safe path from offline training to runtime inference. Model output must never
bypass risk.

## Decision

1. Domain owns `FeatureSet`, `Predictor`, `ScoredPrediction`, and
   `ResearchPartition`.
2. Offline training lives in `research/python` and exports ONNX + metadata.
3. Rust inference is an anti-corruption boundary. Training is disabled in-process.
4. Until an approved ONNX runtime/artifact is wired, research uses
   `DeterministicResearchModel` so pipelines remain testable.
5. `PredictionGatedStrategy` can emit signals that carry prediction metadata,
   but order sizing and approval remain outside ML.
6. `trading-research` backtests reuse the same strategy and risk ports with a
   shared `SimulationClock`, paper broker fills, and performance metrics
   (CAGR proxy, volatility, Sharpe/Sortino, drawdown, turnover, hit rate,
   exposure, costs, benchmark comparison).
7. Walk-forward helpers create train/validation/test windows over ordered
   timestamps.

## Consequences

Research and paper trading share contracts. Enabling live ONNX scoring requires
an explicit artifact review and runtime wiring; it cannot be activated by an
environment variable alone. Portfolio construction and richer feature stores
remain future work.
