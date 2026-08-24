# Portfolio construction

The portfolio layer converts comparable, point-in-time alpha estimates into a
long-only target portfolio and lot-valid rebalance proposal.

## Run the ETF/gold example

```sh
cargo run --bin trading-portfolio-planner
```

The example combines VOO and GLD forecasts, keeps 10% target cash, applies
instrument and risk-group caps, and limits a rebalance to 25% gross turnover.
It prints the target allocations and proposed paper trades as JSON.

## Inputs

Each `AlphaEstimate` includes:

- instrument and reference price;
- expected excess return and forecast volatility;
- model confidence and liquidity score;
- instrument and risk-group caps;
- one common prediction horizon;
- source/model version and point-in-time observation timestamp.

`construct_portfolio` rejects duplicate instruments, mixed currencies, mixed
horizons, future observations, invalid ranges, and non-positive volatility.

## Construction methods

| Method | Score |
| --- | --- |
| `EqualWeight` | One per eligible instrument |
| `InverseVolatility` | Confidence × liquidity ÷ volatility |
| `RiskAdjustedAlpha` | Positive excess return × confidence × liquidity ÷ volatility |

Scores are allocations, not expected money returns. The water-filling allocator
normalizes them and redistributes capacity until it reaches the target invested
weight or encounters position/group caps. Any remainder stays in cash.

## Rebalancing

`plan_rebalance` compares target quantities with `HoldingSnapshot` values:

1. Convert target value to quantity using the reference price.
2. Round down to the instrument's board lot.
3. Calculate buys, reductions, and exits.
4. Scale all changes to the gross-turnover limit.
5. Round again and remove trades below the minimum notional.
6. Return sells before buys.

The result is a `RebalancePlan`; it does not create approved orders. The normal
execution path must convert each proposal to an intent and run the paper-only
risk gate.

## Current limitations

This first version is intentionally deterministic and long-only. It does not
yet estimate covariance, optimize factor exposures, model nonlinear market
impact, account for taxes or borrow, or perform multi-currency allocation.
Those capabilities should be added only with point-in-time data and
portfolio-level backtests.
