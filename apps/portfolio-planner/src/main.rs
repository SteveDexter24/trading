use chrono::Utc;
use rust_decimal::Decimal;
use trading_domain::{Currency, Exchange, Instrument, Money, Price, Quantity};
use trading_portfolio::{
    construct_portfolio, plan_rebalance, AlphaEstimate, ConstructionConstraints,
    ConstructionMethod, ConstructionRequest, GroupConstraint, HoldingSnapshot,
    RebalanceConstraints,
};
use uuid::Uuid;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let as_of = Utc::now();
    let voo = listed_fund("VOO")?;
    let gld = listed_fund("GLD")?;
    let estimates = vec![
        alpha_estimate(
            voo.clone(),
            Price::new(Decimal::new(500, 0))?,
            Decimal::new(8, 2),
            Decimal::new(18, 2),
            Decimal::new(75, 2),
            Decimal::new(98, 2),
            "us-equity",
            as_of,
        ),
        alpha_estimate(
            gld.clone(),
            Price::new(Decimal::new(200, 0))?,
            Decimal::new(5, 2),
            Decimal::new(12, 2),
            Decimal::new(80, 2),
            Decimal::new(95, 2),
            "gold",
            as_of,
        ),
    ];
    let target = construct_portfolio(ConstructionRequest {
        as_of,
        total_equity: Money::new(Decimal::new(100_000, 0), Currency::Usd)?,
        method: ConstructionMethod::RiskAdjustedAlpha,
        constraints: ConstructionConstraints {
            target_invested_weight: Decimal::new(90, 2),
            maximum_position_weight: Decimal::new(60, 2),
            group_constraints: vec![
                GroupConstraint {
                    group: "us-equity".to_owned(),
                    maximum_weight: Decimal::new(65, 2),
                },
                GroupConstraint {
                    group: "gold".to_owned(),
                    maximum_weight: Decimal::new(50, 2),
                },
            ],
        },
        estimates,
    })?;
    let current = vec![
        HoldingSnapshot {
            instrument: voo,
            quantity: Quantity::new(Decimal::new(20, 0))?,
            reference_price: Price::new(Decimal::new(500, 0))?,
        },
        HoldingSnapshot {
            instrument: gld,
            quantity: Quantity::new(Decimal::new(10, 0))?,
            reference_price: Price::new(Decimal::new(200, 0))?,
        },
    ];
    let rebalance = plan_rebalance(
        &target,
        &current,
        RebalanceConstraints {
            minimum_trade_notional: Money::new(Decimal::new(100, 0), Currency::Usd)?,
            maximum_turnover_weight: Decimal::new(25, 2),
        },
    )?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "mode": "paper-proposal-only",
            "target": target,
            "rebalance": rebalance,
        }))?
    );
    Ok(())
}

fn listed_fund(symbol: &str) -> Result<Instrument, Box<dyn std::error::Error>> {
    Ok(Instrument {
        id: Uuid::new_v4(),
        symbol: symbol.to_owned(),
        exchange: Exchange::Nyse,
        currency: Currency::Usd,
        board_lot: Quantity::new(Decimal::ONE)?,
        tick_size: Price::new(Decimal::new(1, 2))?,
        fractional_supported: false,
    })
}

#[allow(clippy::too_many_arguments)]
fn alpha_estimate(
    instrument: Instrument,
    reference_price: Price,
    expected_excess_return: Decimal,
    forecast_volatility: Decimal,
    confidence: Decimal,
    liquidity_score: Decimal,
    risk_group: &str,
    observed_at: chrono::DateTime<Utc>,
) -> AlphaEstimate {
    AlphaEstimate {
        reference_price,
        instrument,
        expected_excess_return,
        forecast_volatility,
        confidence,
        liquidity_score,
        maximum_weight: Decimal::new(60, 2),
        risk_group: Some(risk_group.to_owned()),
        forecast_horizon_seconds: 86_400,
        source_version: "etf-gold-alpha-v1".to_owned(),
        observed_at,
    }
}
