//! Pure portfolio accounting helpers.

use trading_domain::{DomainError, Fill, Money};

pub fn cash_debit(fill: &Fill) -> Result<Money, DomainError> {
    fill.price.times(fill.quantity)?.checked_add(fill.fee)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use proptest::prelude::*;
    use rust_decimal::Decimal;
    use trading_domain::{Currency, Price, Quantity};
    use uuid::Uuid;

    proptest! {
        #[test]
        fn debit_always_equals_exact_notional_plus_fee(
            price_minor in 1_i64..1_000_000,
            quantity_whole in 1_i64..10_000,
            fee_minor in 0_i64..10_000,
        ) {
            let price_value = Decimal::new(price_minor, 2);
            let quantity_value = Decimal::new(quantity_whole, 0);
            let fee_value = Decimal::new(fee_minor, 2);
            let fill = Fill {
                id: Uuid::new_v4(),
                event_id: "property-fill".to_owned(),
                order_id: Uuid::new_v4(),
                quantity: Quantity::new(quantity_value).expect("positive"),
                price: Price::new(price_value).expect("positive"),
                fee: Money::new(fee_value).expect("non-negative"),
                currency: Currency::Usd,
                filled_at: Utc::now(),
            };

            prop_assert_eq!(
                cash_debit(&fill).expect("debit").value(),
                price_value * quantity_value + fee_value
            );
        }
    }
}
