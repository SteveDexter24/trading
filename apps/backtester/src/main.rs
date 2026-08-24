use chrono::{Duration, NaiveDate, Utc};
use rust_decimal::Decimal;
use std::{sync::Arc, time::Duration as StdDuration};
use tracing::info;
use trading_domain::{
    Currency, Exchange, Instrument, MarketEvent, Money, Price, Quantity, Quote, RiskContext,
    StrategyId, TradingMode,
};
use trading_ml_inference::DeterministicResearchModel;
use trading_research::{
    plan_walk_forward, BacktestConfig, Backtester, SharedClock, SimulationClock,
};
use trading_risk::{RiskLimits, RuleBasedRisk};
use trading_strategy::PredictionGatedStrategy;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let start = Utc::now() - Duration::days(20);
    let clock = Arc::new(SimulationClock::new(start));
    let instrument = Instrument {
        id: Uuid::new_v4(),
        symbol: "AAPL".to_owned(),
        exchange: Exchange::Nasdaq,
        currency: Currency::Usd,
        board_lot: Quantity::new(Decimal::ONE)?,
        tick_size: Price::new(Decimal::new(1, 2))?,
        fractional_supported: false,
    };

    let events = (0..12)
        .map(|index| {
            let at = start + Duration::days(index);
            MarketEvent::Quote(Quote {
                event_id: format!("research-quote-{index}"),
                instrument: instrument.clone(),
                bid: Price::new(Decimal::new(99, 0)).expect("bid"),
                ask: Price::new(Decimal::new(100, 0)).expect("ask"),
                sequence: (index + 1) as u64,
                occurred_at: at,
                observed_at: at,
                trading_date: NaiveDate::from_ymd_opt(2026, 1, 1)
                    .expect("date")
                    .checked_add_days(chrono::Days::new(index as u64))
                    .expect("date"),
                source: "synthetic-research".to_owned(),
            })
        })
        .collect::<Vec<_>>();

    let stamps = events
        .iter()
        .map(|event| match event {
            MarketEvent::Quote(quote) => quote.observed_at,
            MarketEvent::Bar(bar) => bar.observed_at,
        })
        .collect::<Vec<_>>();
    let plan = plan_walk_forward(&stamps, 6, 3, 3);
    info!(
        windows = %serde_json::to_string(&plan)?,
        "walk-forward partitions"
    );

    let strategy = PredictionGatedStrategy::new(
        StrategyId("research-prediction".to_owned()),
        Arc::new(DeterministicResearchModel::quote_momentum_v1()),
        Decimal::new(50, 2),
        Decimal::new(50, 2),
    );
    let limits = RiskLimits {
        maximum_quote_age: Duration::days(40),
        maximum_order_value: Money::new(Decimal::new(10_000, 0), Currency::Usd)?,
        maximum_position_exposure: Money::new(Decimal::new(50_000, 0), Currency::Usd)?,
        maximum_strategy_exposure: Money::new(Decimal::new(50_000, 0), Currency::Usd)?,
        maximum_portfolio_exposure: Money::new(Decimal::new(100_000, 0), Currency::Usd)?,
        maximum_daily_loss: Money::new(Decimal::new(5_000, 0), Currency::Usd)?,
        maximum_drawdown: Money::new(Decimal::new(10_000, 0), Currency::Usd)?,
        maximum_spread_fraction: Decimal::new(5, 2),
    };
    let risk = RuleBasedRisk::new(
        [instrument.id],
        limits,
        Box::new(SharedClock(Arc::clone(&clock))),
    );
    let first_quote = match &events[0] {
        MarketEvent::Quote(quote) => quote.clone(),
        MarketEvent::Bar(_) => return Err("expected quote".into()),
    };
    let context = RiskContext {
        mode: TradingMode::Paper,
        quote: first_quote,
        market_session_open: true,
        settled_cash: Money::new(Decimal::new(100_000, 0), Currency::Usd)?,
        position_exposure: Money::zero(Currency::Usd),
        strategy_exposure: Money::zero(Currency::Usd),
        portfolio_exposure: Money::zero(Currency::Usd),
        daily_loss: Money::zero(Currency::Usd),
        drawdown: Money::zero(Currency::Usd),
        open_position: None,
        duplicate_order: false,
        kill_switch_active: false,
    };

    let result = Backtester::new(
        Arc::new(strategy),
        Arc::new(risk),
        clock,
        BacktestConfig {
            initial_cash: Money::new(Decimal::new(100_000, 0), Currency::Usd)?,
            order_quantity: Quantity::new(Decimal::ONE)?,
            fee: Money::new(Decimal::ONE, Currency::Usd)?,
            latency: StdDuration::ZERO,
            years: Decimal::ONE,
        },
    )
    .run(&events, context)
    .await?;

    info!(
        trading_mode = "paper",
        report = %serde_json::to_string(&result.report)?,
        outcomes = result.outcomes.len(),
        "research backtest completed"
    );
    Ok(())
}
