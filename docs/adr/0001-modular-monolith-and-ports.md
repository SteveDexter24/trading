# ADR 0001: Modular monolith with ports and adapters

Status: Accepted

## Context

The engine needs deterministic backtests and paper execution without coupling
trading rules to Webull, PostgreSQL, HTTP, or an ML runtime. Operational
complexity must stay low enough for one ARM VM.

## Decision

Use one Rust workspace and process boundary. `trading-domain` owns value types
and the `MarketDataPort`, `StrategyPort`, `RiskEvaluator`, `BrokerPort`,
`StoragePort`, and `Clock` traits. Adapters depend inward on those traits.
Domain code has no transport or persistence dependencies.

The execution application composes these modules with Tokio. Calls across
module boundaries are asynchronous where they perform I/O. Transformations and
risk checks remain pure where possible.

## Consequences

- Paper execution and future backtests share strategy and risk contracts.
- Webull and PostgreSQL can be replaced in tests.
- Modules can be extracted later only if measured scaling or ownership needs
  justify a service boundary.
- In-process calls avoid distributed consistency and observability overhead.
