use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use tokio::sync::RwLock;
use trading_domain::{DomainError, Instrument, MarketDataPort, Quote};
use uuid::Uuid;

#[derive(Debug)]
pub struct ValidatedMarketData {
    max_age: Duration,
    quotes: RwLock<HashMap<Uuid, Quote>>,
}

impl ValidatedMarketData {
    #[must_use]
    pub fn new(max_age: Duration) -> Self {
        Self {
            max_age,
            quotes: RwLock::new(HashMap::new()),
        }
    }

    pub async fn ingest(&self, quote: Quote, now: DateTime<Utc>) -> Result<(), DomainError> {
        let mut quotes = self.quotes.write().await;
        validate_quote(quotes.get(&quote.instrument.id), &quote, now, self.max_age)?;
        quotes.insert(quote.instrument.id, quote);
        Ok(())
    }
}

pub fn validate_quote(
    previous: Option<&Quote>,
    quote: &Quote,
    now: DateTime<Utc>,
    max_age: Duration,
) -> Result<(), DomainError> {
    let timestamp_is_valid = now.signed_duration_since(quote.observed_at) <= max_age
        && quote.observed_at >= quote.occurred_at;
    let sequence_is_valid = previous.is_none_or(|prior| {
        quote.sequence > prior.sequence && quote.occurred_at > prior.occurred_at
    });

    (timestamp_is_valid && sequence_is_valid)
        .then_some(())
        .ok_or(DomainError::InvalidMarketDataOrder)
}

#[async_trait]
impl MarketDataPort for ValidatedMarketData {
    async fn latest_quote(&self, instrument: &Instrument) -> Result<Quote, DomainError> {
        self.quotes
            .read()
            .await
            .get(&instrument.id)
            .cloned()
            .ok_or_else(|| DomainError::Adapter("quote unavailable".to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use trading_domain::{Currency, Exchange, Price, Quantity};

    fn quote(sequence: u64, occurred_at: DateTime<Utc>) -> Quote {
        Quote {
            event_id: format!("quote-{sequence}"),
            instrument: Instrument {
                id: Uuid::nil(),
                symbol: "AAPL".to_owned(),
                exchange: Exchange::Nasdaq,
                currency: Currency::Usd,
                board_lot: Quantity::new(Decimal::ONE).expect("quantity"),
                tick_size: Price::new(Decimal::new(1, 2)).expect("tick"),
                fractional_supported: false,
            },
            bid: Price::new(Decimal::new(10_000, 2)).expect("bid"),
            ask: Price::new(Decimal::new(10_010, 2)).expect("ask"),
            sequence,
            occurred_at,
            observed_at: occurred_at,
            trading_date: NaiveDate::from_ymd_opt(2026, 8, 11).expect("date"),
            source: "synthetic".to_owned(),
        }
    }

    #[tokio::test]
    async fn rejects_out_of_order_quotes() {
        let now = Utc::now();
        let data = ValidatedMarketData::new(Duration::seconds(5));
        data.ingest(quote(2, now), now).await.expect("first quote");
        assert_eq!(
            data.ingest(quote(1, now - Duration::seconds(1)), now).await,
            Err(DomainError::InvalidMarketDataOrder)
        );
    }
}
