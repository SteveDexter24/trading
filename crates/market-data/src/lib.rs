//! Market-data validation with a pure validation core.

mod validator;

pub use validator::{validate_quote, ValidatedMarketData};
