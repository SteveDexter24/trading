//! Market-data validation and in-process adapter.

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
        if now.signed_duration_since(quote.observed_at) > self.max_age
            || quote.observed_at < quote.occurred_at
        {
            return Err(DomainError::InvalidMarketDataOrder);
        }

        let mut quotes = self.quotes.write().await;
        if quotes.get(&quote.instrument.id).is_some_and(|previous| {
            quote.sequence <= previous.sequence || quote.occurred_at <= previous.occurred_at
        }) {
            return Err(DomainError::InvalidMarketDataOrder);
        }
        quotes.insert(quote.instrument.id, quote);
        Ok(())
    }
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
