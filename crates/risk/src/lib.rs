//! Composable pre-trade risk policy.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashSet};
use trading_domain::{
    Clock, DomainError, Money, OrderIntent, OrderType, RiskContext, RiskDecision, RiskEvaluator,
    RiskOutcome, Side, TradingMode,
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub struct RiskLimits {
    pub maximum_quote_age: Duration,
    pub maximum_order_value: Money,
    pub maximum_position_exposure: Money,
    pub maximum_strategy_exposure: Money,
    pub maximum_portfolio_exposure: Money,
    pub maximum_daily_loss: Money,
    pub maximum_drawdown: Money,
    pub maximum_spread_fraction: Decimal,
}

#[derive(Debug)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

pub struct RuleBasedRisk {
    allowed_instruments: HashSet<Uuid>,
    limits: RiskLimits,
    clock: Box<dyn Clock>,
}

impl RuleBasedRisk {
    #[must_use]
    pub fn new(
        allowed_instruments: impl IntoIterator<Item = Uuid>,
        limits: RiskLimits,
        clock: Box<dyn Clock>,
    ) -> Self {
        Self {
            allowed_instruments: allowed_instruments.into_iter().collect(),
            limits,
            clock,
        }
    }
}

#[async_trait]
impl RiskEvaluator for RuleBasedRisk {
    async fn evaluate(
        &self,
        intent: &OrderIntent,
        context: &RiskContext,
    ) -> Result<RiskDecision, DomainError> {
        let now = self.clock.now();
        let notional = context.quote.ask.times(intent.quantity)?;
        let resulting_position = context.position_exposure.checked_add(notional)?;
        let resulting_strategy = context.strategy_exposure.checked_add(notional)?;
        let resulting_portfolio = context.portfolio_exposure.checked_add(notional)?;
        let quote_fresh = now.signed_duration_since(context.quote.observed_at)
            <= self.limits.maximum_quote_age
            && context.quote.observed_at >= context.quote.occurred_at;
        let board_lot_valid = intent.instrument.fractional_supported
            || intent.quantity.value() % intent.instrument.board_lot.value() == Decimal::ZERO;
        let tick_size_valid = match intent.order_type {
            OrderType::Market => true,
            OrderType::Limit(price) => {
                price.value() % intent.instrument.tick_size.value() == Decimal::ZERO
            }
        };

        let checks = BTreeMap::from([
            ("board_lot".to_owned(), board_lot_valid),
            ("daily_loss".to_owned(), context.daily_loss <= self.limits.maximum_daily_loss),
            (
                "duplicate_order".to_owned(),
                !context.duplicate_order,
            ),
            ("global_kill_switch".to_owned(), !context.kill_switch_active),
            (
                "instrument_allowlist".to_owned(),
                self.allowed_instruments.contains(&intent.instrument.id),
            ),
            (
                "instrument_currency".to_owned(),
                intent.instrument.currency == intent.instrument.exchange.currency(),
            ),
            (
                "liquidity_spread".to_owned(),
                context.quote.spread_fraction() <= self.limits.maximum_spread_fraction,
            ),
            ("market_session".to_owned(), context.market_session_open),
            (
                "maximum_drawdown".to_owned(),
                context.drawdown <= self.limits.maximum_drawdown,
            ),
            (
                "maximum_order_value".to_owned(),
                notional <= self.limits.maximum_order_value,
            ),
            (
                "maximum_portfolio_exposure".to_owned(),
                resulting_portfolio <= self.limits.maximum_portfolio_exposure,
            ),
            (
                "maximum_position_exposure".to_owned(),
                resulting_position <= self.limits.maximum_position_exposure,
            ),
            (
                "maximum_strategy_exposure".to_owned(),
                resulting_strategy <= self.limits.maximum_strategy_exposure,
            ),
            ("quote_freshness".to_owned(), quote_fresh),
            (
                "settled_cash".to_owned(),
                intent.side == Side::Sell || context.settled_cash >= notional,
            ),
            ("tick_size".to_owned(), tick_size_valid),
            ("trading_mode_paper".to_owned(), context.mode == TradingMode::Paper),
        ]);
        let reasons = checks
            .iter()
            .filter_map(|(name, passed)| (!passed).then_some(name.clone()))
            .collect::<Vec<_>>();
        let outcome = if reasons.is_empty() {
            RiskOutcome::Approved
        } else {
            RiskOutcome::Rejected { reasons }
        };

        Ok(RiskDecision {
            id: Uuid::new_v4(),
            intent_id: intent.id,
            outcome,
            evaluated_at: now,
            checks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use trading_domain::{
        Currency, Exchange, Instrument, Price, Quantity, Quote, StrategyId,
    };

    #[derive(Debug)]
    struct FixedClock(DateTime<Utc>);

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            self.0
        }
    }

    fn fixture() -> (OrderIntent, RiskContext, RiskLimits, DateTime<Utc>) {
        let now = Utc::now();
        let instrument = Instrument {
            id: Uuid::new_v4(),
            symbol: "0700".to_owned(),
            exchange: trading_domain::Exchange::Hkex,
            currency: Currency::Hkd,
            board_lot: Quantity::new(Decimal::new(100, 0)).expect("lot"),
            tick_size: Price::new(Decimal::new(5, 2)).expect("tick"),
            fractional_supported: false,
        };
        let intent = OrderIntent {
            id: Uuid::new_v4(),
            client_order_id: Uuid::new_v4(),
            strategy_id: StrategyId("long-term-value".to_owned()),
            instrument: instrument.clone(),
            side: Side::Buy,
            quantity: Quantity::new(Decimal::new(100, 0)).expect("quantity"),
            order_type: OrderType::Market,
            created_at: now,
        };
        let quote = Quote {
            event_id: "quote-1".to_owned(),
            instrument,
            bid: Price::new(Decimal::new(400_00, 2)).expect("bid"),
            ask: Price::new(Decimal::new(400_05, 2)).expect("ask"),
            sequence: 1,
            occurred_at: now,
            observed_at: now,
            trading_date: NaiveDate::from_ymd_opt(2026, 8, 11).expect("date"),
            source: "synthetic".to_owned(),
        };
        let context = RiskContext {
            mode: TradingMode::Paper,
            quote,
            market_session_open: true,
            settled_cash: Money::new(Decimal::new(100_000, 0)).expect("cash"),
            position_exposure: Money::ZERO,
            strategy_exposure: Money::ZERO,
            portfolio_exposure: Money::ZERO,
            daily_loss: Money::ZERO,
            drawdown: Money::ZERO,
            duplicate_order: false,
            kill_switch_active: false,
        };
        let limits = RiskLimits {
            maximum_quote_age: Duration::seconds(5),
            maximum_order_value: Money::new(Decimal::new(50_000, 0)).expect("limit"),
            maximum_position_exposure: Money::new(Decimal::new(50_000, 0)).expect("limit"),
            maximum_strategy_exposure: Money::new(Decimal::new(75_000, 0)).expect("limit"),
            maximum_portfolio_exposure: Money::new(Decimal::new(100_000, 0)).expect("limit"),
            maximum_daily_loss: Money::new(Decimal::new(5_000, 0)).expect("limit"),
            maximum_drawdown: Money::new(Decimal::new(10_000, 0)).expect("limit"),
            maximum_spread_fraction: Decimal::new(5, 3),
        };
        (intent, context, limits, now)
    }

    #[tokio::test]
    async fn approves_valid_paper_order() {
        let (intent, context, limits, now) = fixture();
        let risk = RuleBasedRisk::new(
            [intent.instrument.id],
            limits,
            Box::new(FixedClock(now)),
        );
        let decision = risk.evaluate(&intent, &context).await.expect("decision");
        assert!(decision.approved());
        assert!(decision.checks.values().all(|passed| *passed));
    }

    #[tokio::test]
    async fn rejects_live_mode_and_kill_switch() {
        let (intent, mut context, limits, now) = fixture();
        context.mode = TradingMode::Live;
        context.kill_switch_active = true;
        let risk = RuleBasedRisk::new(
            [intent.instrument.id],
            limits,
            Box::new(FixedClock(now)),
        );
        let decision = risk.evaluate(&intent, &context).await.expect("decision");
        assert!(!decision.approved());
        assert_eq!(decision.checks["trading_mode_paper"], false);
        assert_eq!(decision.checks["global_kill_switch"], false);
    }

    #[tokio::test]
    async fn rejects_invalid_hk_board_lot() {
        let (mut intent, context, limits, now) = fixture();
        intent.quantity = Quantity::new(Decimal::new(99, 0)).expect("quantity");
        let risk = RuleBasedRisk::new(
            [intent.instrument.id],
            limits,
            Box::new(FixedClock(now)),
        );
        let decision = risk.evaluate(&intent, &context).await.expect("decision");
        assert_eq!(decision.checks["board_lot"], false);
    }
}
