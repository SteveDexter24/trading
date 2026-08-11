use crate::{Instrument, Price, Quantity};
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
    #[must_use]
    pub fn midpoint(&self) -> Price {
        Price((self.bid.0 + self.ask.0) / Decimal::TWO)
    }

    #[must_use]
    pub fn spread_fraction(&self) -> Decimal {
        (self.ask.0 - self.bid.0) / self.midpoint().0
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
