use crate::RiskLimits;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashSet};
use trading_domain::{DomainError, Money, OrderIntent, OrderType, RiskContext, Side, TradingMode};
use uuid::Uuid;

pub(crate) fn evaluate_checks(
    intent: &OrderIntent,
    context: &RiskContext,
    limits: RiskLimits,
    allowed_instruments: &HashSet<Uuid>,
    now: DateTime<Utc>,
) -> Result<BTreeMap<String, bool>, DomainError> {
    let book_ok = context.quote.validate_book().is_ok();
    let spread_ok = context
        .quote
        .spread_fraction()
        .map(|spread| spread <= limits.maximum_spread_fraction)
        .unwrap_or(false);
    let notional = order_notional(intent, context).ok();
    let resulting_position = notional
        .and_then(|value| resulting_exposure(intent.side, context.position_exposure, value).ok());
    let resulting_strategy = notional
        .and_then(|value| resulting_exposure(intent.side, context.strategy_exposure, value).ok());
    let resulting_portfolio = notional
        .and_then(|value| resulting_exposure(intent.side, context.portfolio_exposure, value).ok());

    Ok(BTreeMap::from([
        (
            "board_lot".to_owned(),
            intent.instrument.fractional_supported
                || intent.quantity.value() % intent.instrument.board_lot.value() == Decimal::ZERO,
        ),
        (
            "daily_loss".to_owned(),
            context
                .daily_loss
                .is_at_most(limits.maximum_daily_loss)
                .unwrap_or(false),
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
        ("liquidity_spread".to_owned(), spread_ok),
        ("market_session".to_owned(), context.market_session_open),
        (
            "maximum_drawdown".to_owned(),
            context
                .drawdown
                .is_at_most(limits.maximum_drawdown)
                .unwrap_or(false),
        ),
        (
            "maximum_order_value".to_owned(),
            notional
                .and_then(|value| value.is_at_most(limits.maximum_order_value).ok())
                .unwrap_or(false),
        ),
        (
            "maximum_portfolio_exposure".to_owned(),
            resulting_portfolio
                .and_then(|value| value.is_at_most(limits.maximum_portfolio_exposure).ok())
                .unwrap_or(false),
        ),
        (
            "maximum_position_exposure".to_owned(),
            resulting_position
                .and_then(|value| value.is_at_most(limits.maximum_position_exposure).ok())
                .unwrap_or(false),
        ),
        (
            "maximum_strategy_exposure".to_owned(),
            resulting_strategy
                .and_then(|value| value.is_at_most(limits.maximum_strategy_exposure).ok())
                .unwrap_or(false),
        ),
        (
            "quote_freshness".to_owned(),
            quote_is_fresh(context, now, limits.maximum_quote_age),
        ),
        ("quote_book".to_owned(), book_ok),
        (
            "settled_cash".to_owned(),
            intent.side == Side::Sell
                || notional
                    .and_then(|value| value.is_at_most(context.settled_cash).ok())
                    .unwrap_or(false),
        ),
        (
            "sellable_position".to_owned(),
            intent.side == Side::Buy
                || context
                    .open_position
                    .is_some_and(|position| intent.quantity.is_at_most(position)),
        ),
        ("tick_size".to_owned(), tick_size_is_valid(intent)),
        (
            "trading_mode_paper".to_owned(),
            context.mode == TradingMode::Paper,
        ),
    ]))
}

fn resulting_exposure(side: Side, current: Money, notional: Money) -> Result<Money, DomainError> {
    match side {
        Side::Buy => current.checked_add(notional),
        Side::Sell => current
            .checked_sub(notional)
            .or_else(|_| Ok(Money::zero(current.currency()))),
    }
}

fn order_notional(intent: &OrderIntent, context: &RiskContext) -> Result<Money, DomainError> {
    let price = match (intent.side, intent.order_type) {
        (Side::Buy, OrderType::Market) => context.quote.ask,
        (Side::Sell, OrderType::Market) => context.quote.bid,
        (Side::Buy, OrderType::Limit(limit)) => {
            // Conservative cash/value check for buy limits.
            if limit > context.quote.ask {
                limit
            } else {
                context.quote.ask
            }
        }
        (Side::Sell, OrderType::Limit(limit)) => {
            if limit < context.quote.bid {
                limit
            } else {
                context.quote.bid
            }
        }
    };
    price.times(intent.quantity, context.quote.instrument.currency)
}

fn quote_is_fresh(context: &RiskContext, now: DateTime<Utc>, maximum_age: Duration) -> bool {
    let age = now.signed_duration_since(context.quote.observed_at);
    age >= Duration::zero()
        && age <= maximum_age
        && context.quote.observed_at >= context.quote.occurred_at
        && context.quote.observed_at <= now
}

fn tick_size_is_valid(intent: &OrderIntent) -> bool {
    match intent.order_type {
        OrderType::Market => true,
        OrderType::Limit(price) => {
            price.value() % intent.instrument.tick_size.value() == Decimal::ZERO
        }
    }
}
