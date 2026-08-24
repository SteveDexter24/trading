# ADR 0007: Deterministic portfolio construction

Status: Accepted

## Context

Individual strategy signals are not a portfolio. A portfolio layer must compare
competing forecasts, reserve cash, control concentration, and turn target
weights into lot-valid rebalance proposals. It must not bypass pre-trade risk or
hide look-ahead data inside an optimizer.

## Decision

1. `trading-portfolio` owns pure construction and rebalance functions.
2. Inputs are point-in-time `AlphaEstimate` values with forecast horizon,
   expected excess return, volatility, confidence, liquidity, source version,
   and observation time.
3. A construction request may use equal weight, inverse volatility, or
   risk-adjusted alpha:

   ```text
   risk-adjusted score =
     positive expected excess return × confidence × liquidity / volatility
   ```

4. The initial implementation is explicitly long-only. Negative alpha is
   excluded by risk-adjusted construction rather than translated into a short.
5. Deterministic water filling allocates scores subject to invested-weight,
   position, instrument, and risk-group caps. Unallocated capacity remains cash.
6. Rebalance planning compares target and current quantities, scales gross
   trades to a turnover limit, rounds down to exchange board lots, filters small
   trades, and proposes sells before buys.
7. Portfolio output is a proposal, not an order. Each resulting trade must still
   pass the execution engine's paper-only risk evaluator independently.
8. Money, weights, estimates, and quantities use decimal arithmetic. Statistical
   estimates may be produced by offline models, but binary floating-point values
   do not enter money calculations.

## Consequences

The platform can now construct constrained ETF/gold or broader long-only target
portfolios and produce auditable rebalance proposals. Allocation remains
reproducible and independent of I/O.

This is not yet an institutional optimizer. Covariance-aware optimization,
factor exposure bounds, transaction-cost curves, tax lots, short borrow,
multi-currency optimization, robust estimation, and portfolio-aware historical
simulation remain future work.
