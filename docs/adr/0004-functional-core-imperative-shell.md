# ADR 0004: Functional core and imperative shell

Status: Accepted

## Context

Trading decisions and accounting must be deterministic, testable, and
replayable. Network, clock, random-ID, database, and broker operations are
inherently effectful, but those effects should not leak into business rules.

## Decision

Use a functional-core/imperative-shell style:

- Domain values are immutable from callers' perspective.
- State transitions consume a value and return `Result<NewValue, DomainError>`;
  they do not mutate shared state.
- Risk checks, market-data validation, strategy signal generation, fill
  planning, and accounting are pure functions.
- Time and generated IDs are passed into pure functions as values.
- Iterator transformations and `Option`/`Result` combinators express data flow.
- Tokio, locks, PostgreSQL, Webull transports, wall-clock reads, and random ID
  generation stay in adapter or orchestration modules.
- `lib.rs` files declare modules and re-export public APIs; implementations live
  in focused source files.

Rust is not treated as a purely functional language. Local mutation is allowed
inside an imperative adapter when it makes ownership and I/O sequencing clear,
but domain behavior remains referentially transparent.

## Consequences

Tests can call business rules without an async runtime or mocks. Backtests and
paper execution can share the same transformations. Effects remain visible at
composition boundaries, while allocation and ownership stay idiomatic Rust.
