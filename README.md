# Paper Trading System

A paper-only, asynchronous Rust trading-system foundation. It is a modular
monolith with pure domain types and ports for market data, strategies, risk,
brokers, storage, and time.

Business logic follows a functional-core/imperative-shell design. Immutable
domain transitions, risk rules, quote validation, signal generation, fill
planning, and accounting are pure functions. Tokio, clocks, UUID generation,
locks, PostgreSQL, and transports stay at the edges. Crate `lib.rs` files only
declare focused modules and re-export their public APIs.

## Safety state

- Trading mode is fixed to `Paper` by the risk policy and status API.
- The runtime contains only the deterministic paper broker.
- Production Webull submission has no constructible code path.
- The Webull sandbox boundary also refuses submission in this iteration.
- Every intent is persisted before broker submission.
- Source events, client order IDs, broker updates, and fills are idempotent.
- Money, prices, quantities, fees, and FX values use `rust_decimal`, never
  floating point.

## Workspace

| Path | Responsibility |
| --- | --- |
| `crates/domain` | Value types, state machine, and inward-facing ports |
| `crates/market-data` | Freshness and sequence validation |
| `crates/strategy` | Deterministic strategy baseline |
| `crates/risk` | Paper-only pre-trade risk checks |
| `crates/portfolio` | Exact accounting functions and property tests |
| `crates/execution` | Persist-risk-submit-reconcile orchestration |
| `crates/paper-broker` | Idempotent simulated fills |
| `crates/storage` | In-memory test adapter, PostgreSQL adapter, migrations |
| `crates/webull-adapter` | Fail-closed HTTP/MQTT/gRPC boundary |
| `apps/trading-engine` | Synthetic end-to-end paper flow |
| `apps/api` | Read-only status API and authenticated kill switch |

## Verify and run

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo run --bin trading-engine
```

The engine emits a structured JSON audit result for a synthetic quote, signal,
intent, risk approval, simulated order, and fill. Set `DATABASE_URL` to use
PostgreSQL; otherwise it uses the in-memory adapter.

Run the status API separately:

```sh
ADMIN_API_TOKEN='replace-me' cargo run --bin trading-api
```

Endpoints are `/health`, `/ready`, `/v1/status`, `/v1/portfolio`,
`/v1/positions`, `/v1/orders`, `/v1/fills`, `/v1/signals`, and
`/v1/risk-decisions`. There is no trading endpoint. The only mutation is
`POST /v1/system/kill-switch`, which requires the configured bearer token.

## Data correctness

PostgreSQL migrations cover instruments, bars, quotes, corporate actions,
fundamentals, news and entities, features, models, signals, intents, risk
decisions, orders, fills, positions, HKD/USD cash, FX, portfolio snapshots, and
system events. Research data records observation, effective, source, and
ingestion times so historical queries can enforce point-in-time visibility.

HK/US calendars, fees, settlement, reconciliation transports, and the
backtesting runner remain adapter work. Domain contracts already separate
strategies, risk, prediction versions, clocks, currencies, exchange timezones,
and trading dates. ML training is intentionally absent; future offline training
must export versioned ONNX artifacts, while ML may only propose signals.

See `docs/adr` for architectural decisions and `docs/operations.md` for
deployment, metrics, backup, and recovery guidance.