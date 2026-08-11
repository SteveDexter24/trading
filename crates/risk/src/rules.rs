use crate::RiskLimits;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashSet};
use trading_domain::{DomainError, OrderIntent, OrderType, RiskContext, Side, TradingMode};
use uuid::Uuid;

pub(crate) fn evaluate_checks(
    intent: &OrderIntent,
    context: &RiskContext,
    limits: RiskLimits,
    allowed_instruments: &HashSet<Uuid>,
    now: DateTime<Utc>,
) -> Result<BTreeMap<String, bool>, DomainError> {
    let notional = context
        .quote
        .ask
        .times(intent.quantity, context.quote.instrument.currency)?;
    let resulting_position = context.position_exposure.checked_add(notional)?;
    let resulting_strategy = context.strategy_exposure.checked_add(notional)?;
    let resulting_portfolio = context.portfolio_exposure.checked_add(notional)?;

    Ok(BTreeMap::from([
        (
            "board_lot".to_owned(),
            intent.instrument.fractional_supported
                || intent.quantity.value() % intent.instrument.board_lot.value() == Decimal::ZERO,
        ),
        (
            "daily_loss".to_owned(),
            context.daily_loss.is_at_most(limits.maximum_daily_loss)?,
        ),
        ("duplicate_order".to_owned(), !context.duplicate_order),
        ("global_kill_switch".to_owned(), !context.kill_switch_active),
        (
            "instrument_allowlist".to_owned(),
            allowed_instruments.contains(&intent.instrument.id),
        ),
        (
            "instrument_currency".to_owned(),
            intent.instrument.currency == intent.instrument.exchange.currency(),
        ),
        (
            "quote_instrument".to_owned(),
            context.quote.instrument == intent.instrument,
        ),
        (
            "liquidity_spread".to_owned(),
            context.quote.spread_fraction() <= limits.maximum_spread_fraction,
        ),
        ("market_session".to_owned(), context.market_session_open),
        (
            "maximum_drawdown".to_owned(),
            context.drawdown.is_at_most(limits.maximum_drawdown)?,
        ),
        (
            "maximum_order_value".to_owned(),
            notional.is_at_most(limits.maximum_order_value)?,
        ),
        (
            "maximum_portfolio_exposure".to_owned(),
            resulting_portfolio.is_at_most(limits.maximum_portfolio_exposure)?,
        ),
        (
            "maximum_position_exposure".to_owned(),
            resulting_position.is_at_most(limits.maximum_position_exposure)?,
        ),
        (
            "maximum_strategy_exposure".to_owned(),
            resulting_strategy.is_at_most(limits.maximum_strategy_exposure)?,
        ),
        (
            "quote_freshness".to_owned(),
            quote_is_fresh(context, now, limits.maximum_quote_age),
        ),
        (
            "settled_cash".to_owned(),
            intent.side == Side::Sell || notional.is_at_most(context.settled_cash)?,
        ),
        ("tick_size".to_owned(), tick_size_is_valid(intent)),
        (
            "trading_mode_paper".to_owned(),
            context.mode == TradingMode::Paper,
        ),
    ]))
}

fn quote_is_fresh(
    context: &RiskContext,
    now: DateTime<Utc>,
    maximum_age: chrono::Duration,
) -> bool {
    now.signed_duration_since(context.quote.observed_at) <= maximum_age
        && context.quote.observed_at >= context.quote.occurred_at
}

fn tick_size_is_valid(intent: &OrderIntent) -> bool {
    match intent.order_type {
        OrderType::Market => true,
        OrderType::Limit(price) => {
            price.value() % intent.instrument.tick_size.value() == Decimal::ZERO
        }
    }
}
