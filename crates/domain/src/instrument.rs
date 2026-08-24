use crate::{Price, Quantity};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Currency {
    Hkd,
    Usd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Exchange {
    Hkex,
    Nasdaq,
    Nyse,
}

impl Exchange {
    #[must_use]
    pub const fn timezone(self) -> &'static str {
        match self {
            Self::Hkex => "Asia/Hong_Kong",
            Self::Nasdaq | Self::Nyse => "America/New_York",
        }
    }

    #[must_use]
    pub const fn currency(self) -> Currency {
        match self {
            Self::Hkex => Currency::Hkd,
            Self::Nasdaq | Self::Nyse => Currency::Usd,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Instrument {
    pub id: Uuid,
    pub symbol: String,
    pub exchange: Exchange,
    pub currency: Currency,
    pub board_lot: Quantity,
    pub tick_size: Price,
    pub fractional_supported: bool,
}
