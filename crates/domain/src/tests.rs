use crate::*;
use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use uuid::Uuid;

#[test]
fn rejects_negative_money() {
    assert_eq!(
        Money::new(Decimal::NEGATIVE_ONE, Currency::Usd),
        Err(DomainError::NegativeAmount)
    );
}

#[test]
fn rejects_implicit_fx_arithmetic() {
    let usd = Money::new(Decimal::ONE, Currency::Usd).expect("USD");
    let hkd = Money::new(Decimal::ONE, Currency::Hkd).expect("HKD");
    assert_eq!(
        usd.checked_add(hkd),
        Err(DomainError::CurrencyMismatch {
            left: Currency::Usd,
            right: Currency::Hkd,
        })
    );
}

#[test]
fn calculates_notional_without_floating_point() {
    let price = Price::new(Decimal::new(10_125, 2)).expect("valid price");
    let quantity = Quantity::new(Decimal::new(25, 1)).expect("valid quantity");
    assert_eq!(
        price
            .times(quantity, Currency::Usd)
            .expect("valid notional")
            .amount(),
        Decimal::new(25_3125, 3)
    );
}

#[test]
fn order_transition_is_pure_and_rejects_skips() {
    let now = Utc::now();
    let instrument = Instrument {
        id: Uuid::new_v4(),
        symbol: "AAPL".to_owned(),
        exchange: Exchange::Nasdaq,
        currency: Currency::Usd,
        board_lot: Quantity::new(Decimal::ONE).expect("valid quantity"),
        tick_size: Price::new(Decimal::new(1, 2)).expect("valid price"),
        fractional_supported: false,
    };
    let signal = Signal {
        id: Uuid::new_v4(),
        strategy_id: StrategyId("baseline".to_owned()),
        instrument,
        side: Side::Buy,
        generated_at: now,
        rationale: "test".to_owned(),
        prediction: None,
    };
    let intent =
        OrderIntent::from_signal(&signal, Quantity::new(Decimal::ONE).expect("valid"), now);
    let decision = RiskDecision {
        id: Uuid::new_v4(),
        intent_id: intent.id,
        outcome: RiskOutcome::Approved,
        evaluated_at: now,
        checks: BTreeMap::new(),
    };
    let order = ApprovedOrder::new(intent, &decision, now)
        .expect("approved")
        .into_order();

    assert!(matches!(
        order.transition(OrderStatus::Filled, now),
        Err(DomainError::InvalidOrderTransition { .. })
    ));
}
