use crate::{RiskLimits, RuleBasedRisk};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use rust_decimal::Decimal;
use trading_domain::{
    Clock, Currency, Instrument, Money, OrderIntent, OrderType, Price, Quantity, Quote,
    RiskContext, RiskEvaluator, Side, StrategyId, TradingMode,
};
use uuid::Uuid;

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
        bid: Price::new(Decimal::new(40_000, 2)).expect("bid"),
        ask: Price::new(Decimal::new(40_005, 2)).expect("ask"),
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
        settled_cash: Money::new(Decimal::new(100_000, 0), Currency::Hkd).expect("cash"),
        position_exposure: Money::zero(Currency::Hkd),
        strategy_exposure: Money::zero(Currency::Hkd),
        portfolio_exposure: Money::zero(Currency::Hkd),
        daily_loss: Money::zero(Currency::Hkd),
        drawdown: Money::zero(Currency::Hkd),
        open_position: None,
        duplicate_order: false,
        kill_switch_active: false,
    };
    let limits = RiskLimits {
        maximum_quote_age: Duration::seconds(5),
        maximum_order_value: Money::new(Decimal::new(50_000, 0), Currency::Hkd).expect("limit"),
        maximum_position_exposure: Money::new(Decimal::new(50_000, 0), Currency::Hkd)
            .expect("limit"),
        maximum_strategy_exposure: Money::new(Decimal::new(75_000, 0), Currency::Hkd)
            .expect("limit"),
        maximum_portfolio_exposure: Money::new(Decimal::new(100_000, 0), Currency::Hkd)
            .expect("limit"),
        maximum_daily_loss: Money::new(Decimal::new(5_000, 0), Currency::Hkd).expect("limit"),
        maximum_drawdown: Money::new(Decimal::new(10_000, 0), Currency::Hkd).expect("limit"),
        maximum_spread_fraction: Decimal::new(5, 3),
    };
    (intent, context, limits, now)
}

async fn assert_rule_rejects(
    rule: &str,
    intent: &OrderIntent,
    context: &RiskContext,
    limits: RiskLimits,
    now: DateTime<Utc>,
    allowed_instruments: Vec<Uuid>,
) {
    let risk = RuleBasedRisk::new(allowed_instruments, limits, Box::new(FixedClock(now)));
    let decision = risk.evaluate(intent, context).await.expect("decision");
    assert_eq!(
        decision.checks.get(rule),
        Some(&false),
        "expected {rule} to reject"
    );
}

#[tokio::test]
async fn approves_valid_paper_order() {
    let (intent, context, limits, now) = fixture();
    let risk = RuleBasedRisk::new([intent.instrument.id], limits, Box::new(FixedClock(now)));
    let decision = risk.evaluate(&intent, &context).await.expect("decision");
    assert!(decision.approved());
    assert!(decision.checks.values().all(|passed| *passed));
}

#[tokio::test]
async fn sell_requires_open_position_and_uses_bid() {
    let (mut intent, mut context, limits, now) = fixture();
    intent.side = Side::Sell;
    context.open_position = None;
    assert_rule_rejects(
        "sellable_position",
        &intent,
        &context,
        limits,
        now,
        vec![intent.instrument.id],
    )
    .await;

    context.open_position = Some(intent.quantity);
    let risk = RuleBasedRisk::new([intent.instrument.id], limits, Box::new(FixedClock(now)));
    let decision = risk.evaluate(&intent, &context).await.expect("decision");
    assert!(decision.approved());
}

#[tokio::test]
async fn every_configured_risk_rule_has_a_rejection_case() {
    let (intent, context, limits, now) = fixture();
    let allowed = vec![intent.instrument.id];

    let mut changed_intent = intent.clone();
    changed_intent.quantity = Quantity::new(Decimal::new(99, 0)).expect("quantity");
    assert_rule_rejects(
        "board_lot",
        &changed_intent,
        &context,
        limits,
        now,
        allowed.clone(),
    )
    .await;

    let mut changed = context.clone();
    changed.daily_loss = Money::new(Decimal::new(5_001, 0), Currency::Hkd).expect("daily loss");
    assert_rule_rejects(
        "daily_loss",
        &intent,
        &changed,
        limits,
        now,
        allowed.clone(),
    )
    .await;

    for (name, duplicate, kill_switch) in [
        ("duplicate_order", true, false),
        ("global_kill_switch", false, true),
    ] {
        changed = context.clone();
        changed.duplicate_order = duplicate;
        changed.kill_switch_active = kill_switch;
        assert_rule_rejects(name, &intent, &changed, limits, now, allowed.clone()).await;
    }

    assert_rule_rejects(
        "instrument_allowlist",
        &intent,
        &context,
        limits,
        now,
        Vec::new(),
    )
    .await;

    changed_intent = intent.clone();
    changed_intent.instrument.currency = Currency::Usd;
    for rule in ["instrument_currency", "quote_instrument"] {
        assert_rule_rejects(
            rule,
            &changed_intent,
            &context,
            limits,
            now,
            allowed.clone(),
        )
        .await;
    }

    changed = context.clone();
    changed.quote.ask = Price::new(Decimal::new(45_000, 2)).expect("ask");
    assert_rule_rejects(
        "liquidity_spread",
        &intent,
        &changed,
        limits,
        now,
        allowed.clone(),
    )
    .await;

    changed = context.clone();
    changed.quote.bid = Price::new(Decimal::new(40_100, 2)).expect("bid");
    changed.quote.ask = Price::new(Decimal::new(40_000, 2)).expect("ask");
    assert_rule_rejects(
        "quote_book",
        &intent,
        &changed,
        limits,
        now,
        allowed.clone(),
    )
    .await;

    changed = context.clone();
    changed.market_session_open = false;
    assert_rule_rejects(
        "market_session",
        &intent,
        &changed,
        limits,
        now,
        allowed.clone(),
    )
    .await;

    changed = context.clone();
    changed.drawdown = Money::new(Decimal::new(10_001, 0), Currency::Hkd).expect("drawdown");
    assert_rule_rejects(
        "maximum_drawdown",
        &intent,
        &changed,
        limits,
        now,
        allowed.clone(),
    )
    .await;

    changed_intent = intent.clone();
    changed_intent.quantity = Quantity::new(Decimal::new(200, 0)).expect("quantity");
    assert_rule_rejects(
        "maximum_order_value",
        &changed_intent,
        &context,
        limits,
        now,
        allowed.clone(),
    )
    .await;

    for (rule, position, strategy, portfolio) in [
        ("maximum_position_exposure", 20_000, 0, 0),
        ("maximum_strategy_exposure", 0, 40_000, 0),
        ("maximum_portfolio_exposure", 0, 0, 70_000),
    ] {
        changed = context.clone();
        changed.position_exposure =
            Money::new(Decimal::new(position, 0), Currency::Hkd).expect("exposure");
        changed.strategy_exposure =
            Money::new(Decimal::new(strategy, 0), Currency::Hkd).expect("exposure");
        changed.portfolio_exposure =
            Money::new(Decimal::new(portfolio, 0), Currency::Hkd).expect("exposure");
        assert_rule_rejects(rule, &intent, &changed, limits, now, allowed.clone()).await;
    }

    changed = context.clone();
    changed.quote.occurred_at = now - Duration::seconds(6);
    changed.quote.observed_at = now - Duration::seconds(6);
    assert_rule_rejects(
        "quote_freshness",
        &intent,
        &changed,
        limits,
        now,
        allowed.clone(),
    )
    .await;

    changed = context.clone();
    changed.quote.observed_at = now + Duration::seconds(1);
    assert_rule_rejects(
        "quote_freshness",
        &intent,
        &changed,
        limits,
        now,
        allowed.clone(),
    )
    .await;

    changed = context.clone();
    changed.settled_cash = Money::zero(Currency::Hkd);
    assert_rule_rejects(
        "settled_cash",
        &intent,
        &changed,
        limits,
        now,
        allowed.clone(),
    )
    .await;

    changed_intent = intent.clone();
    changed_intent.side = Side::Sell;
    changed_intent.order_type = OrderType::Market;
    assert_rule_rejects(
        "sellable_position",
        &changed_intent,
        &context,
        limits,
        now,
        allowed.clone(),
    )
    .await;

    changed_intent = intent.clone();
    changed_intent.order_type =
        OrderType::Limit(Price::new(Decimal::new(40_003, 2)).expect("price"));
    assert_rule_rejects(
        "tick_size",
        &changed_intent,
        &context,
        limits,
        now,
        allowed.clone(),
    )
    .await;

    changed = context.clone();
    changed.mode = TradingMode::Live;
    assert_rule_rejects(
        "trading_mode_paper",
        &intent,
        &changed,
        limits,
        now,
        allowed,
    )
    .await;
}
