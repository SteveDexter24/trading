use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use rust_decimal::Decimal;
use std::{env, sync::Arc, time::Duration};
use tracing::info;
use trading_domain::{
    Currency, Exchange, Instrument, MarketEvent, Money, Price, Quantity, Quote, RiskContext,
    StoragePort, StrategyId, TradingMode,
};
use trading_execution::TradingEngine;
use trading_paper_broker::{ExecutionModel, FillPolicy, PaperBroker};
use trading_risk::{RiskLimits, RuleBasedRisk, SystemClock};
use trading_storage::{InMemoryStorage, PostgresStorage};
use trading_strategy::PriceThresholdStrategy;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let now = Utc::now();
    let instrument = Instrument {
        id: Uuid::new_v4(),
        symbol: "AAPL".to_owned(),
        exchange: Exchange::Nasdaq,
        currency: Currency::Usd,
        board_lot: Quantity::new(Decimal::ONE)?,
        tick_size: Price::new(Decimal::new(1, 2))?,
        fractional_supported: false,
    };
    let quote = Quote {
        event_id: "startup-synthetic-quote".to_owned(),
        instrument: instrument.clone(),
        bid: Price::new(Decimal::new(19_995, 2))?,
        ask: Price::new(Decimal::new(20_000, 2))?,
        sequence: 1,
        occurred_at: now,
        observed_at: now,
        trading_date: NaiveDate::from_ymd_opt(2026, 8, 11).ok_or("invalid trading date")?,
        source: "synthetic".to_owned(),
    };
    let context = RiskContext {
        mode: TradingMode::Paper,
        quote: quote.clone(),
        market_session_open: true,
        settled_cash: Money::new(Decimal::new(100_000, 0))?,
        position_exposure: Money::ZERO,
        strategy_exposure: Money::ZERO,
        portfolio_exposure: Money::ZERO,
        daily_loss: Money::ZERO,
        drawdown: Money::ZERO,
        duplicate_order: false,
        kill_switch_active: false,
    };
    let limits = RiskLimits {
        maximum_quote_age: ChronoDuration::seconds(5),
        maximum_order_value: Money::new(Decimal::new(5_000, 0))?,
        maximum_position_exposure: Money::new(Decimal::new(10_000, 0))?,
        maximum_strategy_exposure: Money::new(Decimal::new(25_000, 0))?,
        maximum_portfolio_exposure: Money::new(Decimal::new(100_000, 0))?,
        maximum_daily_loss: Money::new(Decimal::new(5_000, 0))?,
        maximum_drawdown: Money::new(Decimal::new(10_000, 0))?,
        maximum_spread_fraction: Decimal::new(1, 2),
    };
    let storage: Arc<dyn StoragePort> = if let Ok(database_url) = env::var("DATABASE_URL") {
        let postgres = PostgresStorage::connect(&database_url).await?;
        postgres.migrate().await?;
        Arc::new(postgres)
    } else {
        Arc::new(InMemoryStorage::new())
    };
    let engine = TradingEngine::new(
        Arc::new(PriceThresholdStrategy::new(
            StrategyId("short-term-baseline".to_owned()),
            Price::new(Decimal::new(25_000, 2))?,
            Decimal::new(1, 2),
        )),
        Arc::new(RuleBasedRisk::new(
            [instrument.id],
            limits,
            Box::new(SystemClock),
        )),
        Arc::new(PaperBroker::new(ExecutionModel {
            price: quote.ask,
            fee: Money::new(Decimal::new(1, 0))?,
            latency: Duration::from_millis(5),
            fill_policy: FillPolicy::Full,
        })),
        storage,
        Quantity::new(Decimal::ONE)?,
    );

    let outcome = engine
        .process(&MarketEvent::Quote(quote), &context)
        .await?;
    info!(
        trading_mode = "paper",
        outcome = %serde_json::to_string(&outcome)?,
        "synthetic paper-order flow completed"
    );
    Ok(())
}
