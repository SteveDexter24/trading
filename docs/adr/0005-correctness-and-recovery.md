# ADR 0005: Correctness guards and recovery semantics

Status: Accepted

## Context

An audit of the paper-trading foundation found several correctness gaps:
early event claims that could poison retries, side-blind risk exposure,
incomplete fill completion checks, and a kill switch that was not shared
between the API and engine.

## Decision

1. Market-event claims are released only when preparation fails before a
   durable `Submitted` order exists. After submission persistence, the claim
   remains so restarts cannot create a second order for the same event.
2. Paper-broker idempotency uses `entry(...).or_insert(...)` after latency
   simulation so concurrent retries converge on one receipt.
3. Partial fill planning allocates a lot-valid first slice and a remainder
   second slice; it never assumes `qty/2` twice.
4. Risk is side-aware: buys use ask/add exposure/cash; sells use bid/reduce
   exposure/require an open position.
5. Quote books must be uncrossed, and future observations are rejected.
6. Receipts cannot complete as filled unless cumulative fill quantity equals
   the order quantity.
7. The kill switch is stored through `StoragePort`, so the API and engine
   observe the same durable flag.
8. `Price` and `Quantity` keep private decimal fields and validate on
   deserialization.

## Consequences

Paper recovery is safer under crash/retry. Remaining work includes
transactional multi-step persistence and building `RiskContext` from live
portfolio state rather than caller-supplied fixtures.
