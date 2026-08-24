use crate::{
    construct_portfolio, plan_rebalance, AlphaEstimate, ConstructionConstraints,
    ConstructionMethod, ConstructionRequest, GroupConstraint, HoldingSnapshot,
    PortfolioConstructionError, RebalanceConstraints,
};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use trading_domain::{Currency, Exchange, Instrument, Money, Price, Quantity, Side};
use uuid::Uuid;

fn instrument(symbol: &str, price: i64) -> (Instrument, Price) {
    (
        Instrument {
            id: Uuid::new_v4(),
            symbol: symbol.to_owned(),
            exchange: Exchange::Nyse,
            currency: Currency::Usd,
            board_lot: Quantity::new(Decimal::ONE).expect("lot"),
            tick_size: Price::new(Decimal::new(1, 2)).expect("tick"),
            fractional_supported: false,
        },
        Price::new(Decimal::new(price, 0)).expect("price"),
    )
}

#[allow(clippy::too_many_arguments)]
fn estimate(
    instrument: Instrument,
    price: Price,
    alpha: Decimal,
    volatility: Decimal,
    maximum_weight: Decimal,
    group: &str,
    observed_at: chrono::DateTime<Utc>,
) -> AlphaEstimate {
    AlphaEstimate {
        instrument,
        reference_price: price,
        expected_excess_return: alpha,
        forecast_volatility: volatility,
        confidence: Decimal::ONE,
        liquidity_score: Decimal::ONE,
        maximum_weight,
        risk_group: Some(group.to_owned()),
        forecast_horizon_seconds: 86_400,
        source_version: "alpha-v1".to_owned(),
        observed_at,
    }
}

#[test]
fn construction_respects_position_group_and_cash_constraints() {
    let as_of = Utc::now();
    let (equity_a, price_a) = instrument("AAA", 100);
    let (equity_b, price_b) = instrument("BBB", 50);
    let (gold, gold_price) = instrument("GLD", 200);
    let request = ConstructionRequest {
        as_of,
        total_equity: Money::new(Decimal::new(100_000, 0), Currency::Usd).expect("equity"),
        method: ConstructionMethod::RiskAdjustedAlpha,
        constraints: ConstructionConstraints {
            target_invested_weight: Decimal::new(9, 1),
            maximum_position_weight: Decimal::new(6, 1),
            group_constraints: vec![
                GroupConstraint {
                    group: "equity".to_owned(),
                    maximum_weight: Decimal::new(5, 1),
                },
                GroupConstraint {
                    group: "gold".to_owned(),
                    maximum_weight: Decimal::new(4, 1),
                },
            ],
        },
        estimates: vec![
            estimate(
                equity_a,
                price_a,
                Decimal::new(1, 1),
                Decimal::new(2, 1),
                Decimal::new(6, 1),
                "equity",
                as_of,
            ),
            estimate(
                equity_b,
                price_b,
                Decimal::new(5, 2),
                Decimal::new(1, 1),
                Decimal::new(6, 1),
                "equity",
                as_of,
            ),
            estimate(
                gold,
                gold_price,
                Decimal::new(1, 1),
                Decimal::new(1, 1),
                Decimal::new(6, 1),
                "gold",
                as_of,
            ),
        ],
    };

    let plan = construct_portfolio(request).expect("portfolio");

    assert_eq!(plan.allocated_weight, Decimal::new(9, 1));
    assert_eq!(plan.unallocated_cash.amount(), Decimal::new(10_000, 0));
    assert!(plan
        .allocations
        .iter()
        .all(|allocation| allocation.target_weight <= Decimal::new(6, 1)));
    let equity_weight = plan
        .allocations
        .iter()
        .filter(|allocation| allocation.instrument.symbol != "GLD")
        .map(|allocation| allocation.target_weight)
        .sum::<Decimal>();
    let gold_weight = plan
        .allocations
        .iter()
        .find(|allocation| allocation.instrument.symbol == "GLD")
        .expect("gold")
        .target_weight;
    assert_eq!(equity_weight, Decimal::new(5, 1));
    assert_eq!(gold_weight, Decimal::new(4, 1));
}

#[test]
fn risk_adjusted_construction_excludes_negative_alpha() {
    let as_of = Utc::now();
    let (positive, positive_price) = instrument("POS", 100);
    let (negative, negative_price) = instrument("NEG", 100);
    let request = ConstructionRequest {
        as_of,
        total_equity: Money::new(Decimal::new(10_000, 0), Currency::Usd).expect("equity"),
        method: ConstructionMethod::RiskAdjustedAlpha,
        constraints: ConstructionConstraints {
            target_invested_weight: Decimal::ONE,
            maximum_position_weight: Decimal::ONE,
            group_constraints: Vec::new(),
        },
        estimates: vec![
            estimate(
                positive,
                positive_price,
                Decimal::new(1, 2),
                Decimal::new(2, 1),
                Decimal::ONE,
                "equity",
                as_of,
            ),
            estimate(
                negative,
                negative_price,
                Decimal::new(-1, 2),
                Decimal::new(2, 1),
                Decimal::ONE,
                "equity",
                as_of,
            ),
        ],
    };

    let plan = construct_portfolio(request).expect("portfolio");

    assert_eq!(plan.allocations.len(), 1);
    assert_eq!(plan.allocations[0].instrument.symbol, "POS");
}

#[test]
fn construction_rejects_future_information() {
    let as_of = Utc::now();
    let (asset, price) = instrument("FUTURE", 100);
    let request = ConstructionRequest {
        as_of,
        total_equity: Money::new(Decimal::new(10_000, 0), Currency::Usd).expect("equity"),
        method: ConstructionMethod::EqualWeight,
        constraints: ConstructionConstraints {
            target_invested_weight: Decimal::ONE,
            maximum_position_weight: Decimal::ONE,
            group_constraints: Vec::new(),
        },
        estimates: vec![estimate(
            asset,
            price,
            Decimal::new(1, 2),
            Decimal::new(2, 1),
            Decimal::ONE,
            "equity",
            as_of + Duration::seconds(1),
        )],
    };

    assert!(matches!(
        construct_portfolio(request),
        Err(PortfolioConstructionError::InvalidEstimate { .. })
    ));
}

#[test]
fn rebalance_is_lot_valid_and_bounded_by_turnover() {
    let as_of = Utc::now();
    let (asset_a, price_a) = instrument("AAA", 100);
    let (asset_b, price_b) = instrument("BBB", 50);
    let (legacy, legacy_price) = instrument("OLD", 100);
    let target = construct_portfolio(ConstructionRequest {
        as_of,
        total_equity: Money::new(Decimal::new(1_000, 0), Currency::Usd).expect("equity"),
        method: ConstructionMethod::RiskAdjustedAlpha,
        constraints: ConstructionConstraints {
            target_invested_weight: Decimal::new(9, 1),
            maximum_position_weight: Decimal::new(6, 1),
            group_constraints: Vec::new(),
        },
        estimates: vec![
            estimate(
                asset_a.clone(),
                price_a,
                Decimal::new(2, 1),
                Decimal::new(1, 1),
                Decimal::new(6, 1),
                "equity",
                as_of,
            ),
            estimate(
                asset_b,
                price_b,
                Decimal::new(1, 1),
                Decimal::new(1, 1),
                Decimal::new(6, 1),
                "equity",
                as_of,
            ),
        ],
    })
    .expect("target");
    let holdings = vec![
        HoldingSnapshot {
            instrument: asset_a,
            quantity: Quantity::new(Decimal::new(2, 0)).expect("quantity"),
            reference_price: price_a,
        },
        HoldingSnapshot {
            instrument: legacy,
            quantity: Quantity::new(Decimal::ONE).expect("quantity"),
            reference_price: legacy_price,
        },
    ];

    let plan = plan_rebalance(
        &target,
        &holdings,
        RebalanceConstraints {
            minimum_trade_notional: Money::new(Decimal::new(25, 0), Currency::Usd)
                .expect("minimum"),
            maximum_turnover_weight: Decimal::new(4, 1),
        },
    )
    .expect("rebalance");

    assert!(plan.turnover_weight <= Decimal::new(4, 1));
    assert!(plan
        .trades
        .iter()
        .all(|trade| trade.quantity.value().fract() == Decimal::ZERO));
    assert!(plan
        .trades
        .iter()
        .all(|trade| trade.estimated_notional.amount() >= Decimal::new(25, 0)));
    assert!(plan.trades.iter().any(|trade| trade.side == Side::Buy));
}
