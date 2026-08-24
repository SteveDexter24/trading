use std::collections::BTreeMap;
use trading_domain::{DomainError, FeatureSet, FeatureVersion, Quote};
use uuid::Uuid;

/// Deterministic research features derived only from the current quote.
pub fn quote_momentum_features(quote: &Quote) -> Result<FeatureSet, DomainError> {
    quote.validate_book()?;
    let spread = quote.spread_fraction()?;
    let mid = quote.midpoint()?.value();
    Ok(FeatureSet {
        id: Uuid::new_v4(),
        instrument: quote.instrument.clone(),
        feature_version: FeatureVersion("quote-momentum-v1".to_owned()),
        values: BTreeMap::from([
            ("mid".to_owned(), mid),
            ("spread_fraction".to_owned(), spread),
            ("bid".to_owned(), quote.bid.value()),
            ("ask".to_owned(), quote.ask.value()),
            (
                "relative_spread_to_tick".to_owned(),
                (quote.ask.value() - quote.bid.value()) / quote.instrument.tick_size.value(),
            ),
        ]),
        observed_at: quote.observed_at,
        effective_at: quote.occurred_at,
        source: quote.source.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};
    use rust_decimal::Decimal;
    use trading_domain::{Currency, Exchange, Instrument, Price, Quantity};

    #[test]
    fn features_are_point_in_time_safe() {
        let now = Utc::now();
        let quote = Quote {
            event_id: "q1".to_owned(),
            instrument: Instrument {
                id: Uuid::nil(),
                symbol: "AAPL".to_owned(),
                exchange: Exchange::Nasdaq,
                currency: Currency::Usd,
                board_lot: Quantity::new(Decimal::ONE).expect("lot"),
                tick_size: Price::new(Decimal::new(1, 2)).expect("tick"),
                fractional_supported: false,
            },
            bid: Price::new(Decimal::new(100, 0)).expect("bid"),
            ask: Price::new(Decimal::new(101, 0)).expect("ask"),
            sequence: 1,
            occurred_at: now,
            observed_at: now,
            trading_date: NaiveDate::from_ymd_opt(2026, 8, 11).expect("date"),
            source: "synthetic".to_owned(),
        };
        let features = quote_momentum_features(&quote).expect("features");
        features
            .validate_point_in_time(now)
            .expect("visible at decision time");
        assert!(features
            .validate_point_in_time(now - chrono::Duration::seconds(1))
            .is_err());
    }
}
