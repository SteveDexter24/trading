# ADR 0002: Paper-only order safety

Status: Accepted

## Context

An accidental production order is a higher-severity failure than an unavailable
paper engine. Configuration checks alone are insufficient because a bad
deployment can change environment variables.

## Decision

Use defense in depth:

1. Risk rejects every mode except `TradingMode::Paper`.
2. `BrokerPort::submit` accepts `ApprovedOrder`, a domain type that can only be
   created from an approving decision for the same persisted intent.
3. The runtime composes only `PaperBroker`.
4. The Webull adapter exposes no production-broker constructor.
5. Its sandbox adapter returns a disabled error in this release and contains no
   order payload or submission implementation.
6. The API has no order-placement route.

The intent, decision, approved order, submission transition, broker receipt,
and fills are persisted in sequence. Source events and fill events have unique
idempotency keys.

## Consequences

Production submission requires a deliberate code and ADR change, an official
API-contract review, new safety tests, and a separately reviewed deployment.
Setting an environment variable cannot activate it.
