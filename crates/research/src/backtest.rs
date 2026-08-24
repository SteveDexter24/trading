//! Backtest harness that reuses live strategy and risk ports.

use crate::{
    metrics::{summarize_equity_curve, EquityPoint, PerformanceReport},
    SimulationClock,
};
use chrono::Duration;
use rust_decimal::Decimal;
use std::{sync::Arc, time::Duration as StdDuration};
use trading_domain::{
    Clock, MarketEvent, Money, Quantity, RiskContext, RiskEvaluator, StoragePort, StrategyPort,
    TradingMode,
};
use trading_execution::{ExecutionOutcome, TradingEngine};
use trading_paper_broker::{ExecutionModel, FillPolicy, PaperBroker};
use trading_portfolio::cash_debit;
use trading_storage::InMemoryStorage;

#[derive(Debug, Clone)]
pub struct BacktestConfig {
    pub initial_cash: Money,
    pub order_quantity: Quantity,
    pub fee: Money,
    pub latency: StdDuration,
    pub years: Decimal,
}

#[derive(Debug, Clone)]
pub struct BacktestResult {
    pub report: PerformanceReport,
    pub outcomes: Vec<ExecutionOutcome>,
}

pub struct Backtester {
    strategy: Arc<dyn StrategyPort>,
    risk: Arc<dyn RiskEvaluator>,
    clock: Arc<SimulationClock>,
    config: BacktestConfig,
}

impl Backtester {
    #[must_use]
    pub fn new(
        strategy: Arc<dyn StrategyPort>,
        risk: Arc<dyn RiskEvaluator>,
        clock: Arc<SimulationClock>,
        config: BacktestConfig,
    ) -> Self {
        Self {
            strategy,
            risk,
            clock,
            config,
        }
    }

    pub async fn run(
        &self,
        events: &[MarketEvent],
        context_template: RiskContext,
    ) -> Result<BacktestResult, trading_domain::DomainError> {
        let storage = Arc::new(InMemoryStorage::new());
        let currency = self.config.initial_cash.currency();
        let mut cash = self.config.initial_cash;
        let mut outcomes = Vec::new();
        let mut points = vec![EquityPoint {
            equity: cash,
            benchmark: self.config.initial_cash,
        }];

        for event in events {
            let decision_time = match event {
                MarketEvent::Quote(quote) => quote.observed_at,
                MarketEvent::Bar(bar) => bar.observed_at,
            };
            self.clock.set(decision_time);

            let quote = match event {
                MarketEvent::Quote(quote) => quote.clone(),
                MarketEvent::Bar(_) => continue,
            };
            let broker = Arc::new(PaperBroker::new(ExecutionModel {
                price: quote.ask,
                currency,
                fee: self.config.fee,
                latency: self.config.latency,
                fill_policy: FillPolicy::Full,
            }));
            let engine = TradingEngine::new(
                Arc::clone(&self.strategy),
                Arc::clone(&self.risk),
                broker,
                Arc::clone(&storage) as Arc<dyn StoragePort>,
                Arc::clone(&self.clock) as Arc<dyn Clock>,
                self.config.order_quantity,
            );

            let mut context = context_template.clone();
            context.mode = TradingMode::Paper;
            context.quote = quote;
            context.settled_cash = cash;
            context.kill_switch_active = storage.kill_switch_active().await?;

            let outcome = engine.process(event, &context).await?;
            if let ExecutionOutcome::Filled { order: _ } = &outcome {
                let fills = storage.fills().await?;
                if let Some(fill) = fills.last() {
                    cash = cash.checked_sub(cash_debit(fill)?)?;
                }
            }
            outcomes.push(outcome);

            let benchmark = benchmark_value(self.config.initial_cash, &context.quote.ask.value())?;
            points.push(EquityPoint {
                equity: cash,
                benchmark,
            });
            self.clock.advance(Duration::seconds(1));
        }

        let fills = storage.fills().await?;
        let report = summarize_equity_curve(&points, &fills, self.config.years)?;
        Ok(BacktestResult { report, outcomes })
    }
}

fn benchmark_value(
    initial: Money,
    last_price: &Decimal,
) -> Result<Money, trading_domain::DomainError> {
    // Buy-and-hold proxy: scale initial cash by normalized last price / 100.
    let factor = (*last_price / Decimal::new(100, 0)).max(Decimal::new(1, 2));
    Money::new(initial.amount() * factor, initial.currency())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SharedClock;
    use chrono::{NaiveDate, Utc};
    use rust_decimal::Decimal;
    use trading_domain::{Currency, Exchange, Instrument, Price, Quote, StrategyId};
    use trading_ml_inference::DeterministicResearchModel;
    use trading_risk::{RiskLimits, RuleBasedRisk};
    use trading_strategy::PredictionGatedStrategy;
    use uuid::Uuid;

    #[tokio::test]
    async fn prediction_strategy_backtest_reuses_risk_ports() {
        let start = Utc::now();
        let clock = Arc::new(SimulationClock::new(start));
        let instrument = Instrument {
            id: Uuid::new_v4(),
            symbol: "AAPL".to_owned(),
            exchange: Exchange::Nasdaq,
            currency: Currency::Usd,
            board_lot: Quantity::new(Decimal::ONE).expect("lot"),
            tick_size: Price::new(Decimal::new(1, 2)).expect("tick"),
            fractional_supported: false,
        };
        let events = (0..4)
            .map(|index| {
                let at = start + Duration::days(index);
                MarketEvent::Quote(Quote {
                    event_id: format!("bt-{index}"),
                    instrument: instrument.clone(),
                    bid: Price::new(Decimal::new(99, 0)).expect("bid"),
                    ask: Price::new(Decimal::new(100, 0)).expect("ask"),
                    sequence: (index + 1) as u64,
                    occurred_at: at,
                    observed_at: at,
                    trading_date: NaiveDate::from_ymd_opt(2026, 8, 11 + index as u32)
                        .unwrap_or_else(|| NaiveDate::from_ymd_opt(2026, 8, 11).expect("date")),
                    source: "synthetic".to_owned(),
                })
            })
            .collect::<Vec<_>>();

        let strategy = PredictionGatedStrategy::new(
            StrategyId("research-prediction".to_owned()),
            Arc::new(DeterministicResearchModel::quote_momentum_v1()),
            Decimal::new(50, 2),
            Decimal::new(50, 2),
        );
        let limits = RiskLimits {
            maximum_quote_age: Duration::days(30),
            maximum_order_value: Money::new(Decimal::new(10_000, 0), Currency::Usd).expect("limit"),
            maximum_position_exposure: Money::new(Decimal::new(50_000, 0), Currency::Usd)
                .expect("limit"),
            maximum_strategy_exposure: Money::new(Decimal::new(50_000, 0), Currency::Usd)
                .expect("limit"),
            maximum_portfolio_exposure: Money::new(Decimal::new(100_000, 0), Currency::Usd)
                .expect("limit"),
            maximum_daily_loss: Money::new(Decimal::new(5_000, 0), Currency::Usd).expect("limit"),
            maximum_drawdown: Money::new(Decimal::new(10_000, 0), Currency::Usd).expect("limit"),
            maximum_spread_fraction: Decimal::new(5, 2),
        };
        let risk = RuleBasedRisk::new(
            [instrument.id],
            limits,
            Box::new(SharedClock(Arc::clone(&clock))),
        );
        let first_quote = match &events[0] {
            MarketEvent::Quote(quote) => quote.clone(),
            MarketEvent::Bar(_) => panic!("quote"),
        };
        let context = RiskContext {
            mode: TradingMode::Paper,
            quote: first_quote,
            market_session_open: true,
            settled_cash: Money::new(Decimal::new(100_000, 0), Currency::Usd).expect("cash"),
            position_exposure: Money::zero(Currency::Usd),
            strategy_exposure: Money::zero(Currency::Usd),
            portfolio_exposure: Money::zero(Currency::Usd),
            daily_loss: Money::zero(Currency::Usd),
            drawdown: Money::zero(Currency::Usd),
            open_position: None,
            duplicate_order: false,
            kill_switch_active: false,
        };

        let backtester = Backtester::new(
            Arc::new(strategy),
            Arc::new(risk),
            clock,
            BacktestConfig {
                initial_cash: Money::new(Decimal::new(100_000, 0), Currency::Usd).expect("cash"),
                order_quantity: Quantity::new(Decimal::ONE).expect("qty"),
                fee: Money::new(Decimal::ONE, Currency::Usd).expect("fee"),
                latency: StdDuration::ZERO,
                years: Decimal::ONE,
            },
        );
        let result = backtester.run(&events, context).await.expect("backtest");
        assert!(!result.outcomes.is_empty());
        assert!(result.report.total_costs.amount() >= Decimal::ZERO);
    }
}
