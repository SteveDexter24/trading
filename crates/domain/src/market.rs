use crate::{DomainError, Instrument, Price, Quantity};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    pub event_id: String,
    pub instrument: Instrument,
    pub bid: Price,
    pub ask: Price,
    pub sequence: u64,
    pub occurred_at: DateTime<Utc>,
    pub observed_at: DateTime<Utc>,
    pub trading_date: NaiveDate,
    pub source: String,
}

impl Quote {
    pub fn validate_book(&self) -> Result<(), DomainError> {
        (self.ask >= self.bid)
            .then_some(())
            .ok_or(DomainError::InvalidQuote)
    }

    pub fn midpoint(&self) -> Result<Price, DomainError> {
        self.validate_book()?;
        Price::new((self.bid.value() + self.ask.value()) / Decimal::TWO)
    }

    pub fn spread_fraction(&self) -> Result<Decimal, DomainError> {
        let midpoint = self.midpoint()?;
        Ok((self.ask.value() - self.bid.value()) / midpoint.value())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bar {
    pub event_id: String,
    pub instrument: Instrument,
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub volume: Quantity,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub observed_at: DateTime<Utc>,
    pub trading_date: NaiveDate,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum MarketEvent {
    Quote(Quote),
    Bar(Bar),
}

impl MarketEvent {
    #[must_use]
    pub fn event_id(&self) -> &str {
        match self {
            Self::Quote(quote) => &quote.event_id,
            Self::Bar(bar) => &bar.event_id,
        }
    }
}
