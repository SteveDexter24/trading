use crate::{Currency, DomainError};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    #[serde(with = "rust_decimal::serde::str")]
    amount: Decimal,
    currency: Currency,
}

impl Money {
    pub fn new(amount: Decimal, currency: Currency) -> Result<Self, DomainError> {
        (!amount.is_sign_negative())
            .then_some(Self { amount, currency })
            .ok_or(DomainError::NegativeAmount)
    }

    #[must_use]
    pub const fn zero(currency: Currency) -> Self {
        Self {
            amount: Decimal::ZERO,
            currency,
        }
    }

    #[must_use]
    pub const fn amount(self) -> Decimal {
        self.amount
    }

    #[must_use]
    pub const fn currency(self) -> Currency {
        self.currency
    }

    pub fn checked_add(self, other: Self) -> Result<Self, DomainError> {
        self.combine(other, Decimal::checked_add)
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, DomainError> {
        self.ensure_same_currency(other)?;
        self.amount
            .checked_sub(other.amount)
            .filter(|amount| !amount.is_sign_negative())
            .map(|amount| Self {
                amount,
                currency: self.currency,
            })
            .ok_or(DomainError::NegativeAmount)
    }

    pub fn is_at_most(self, other: Self) -> Result<bool, DomainError> {
        self.ensure_same_currency(other)?;
        Ok(self.amount <= other.amount)
    }

    fn combine(
        self,
        other: Self,
        operation: impl FnOnce(Decimal, Decimal) -> Option<Decimal>,
    ) -> Result<Self, DomainError> {
        self.ensure_same_currency(other)?;
        operation(self.amount, other.amount)
            .map(|amount| Self {
                amount,
                currency: self.currency,
            })
            .ok_or_else(|| DomainError::Adapter("money overflow".to_owned()))
    }

    fn ensure_same_currency(self, other: Self) -> Result<(), DomainError> {
        (self.currency == other.currency)
            .then_some(())
            .ok_or(DomainError::CurrencyMismatch {
                left: self.currency,
                right: other.currency,
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Price(Decimal);

impl Price {
    pub fn new(value: Decimal) -> Result<Self, DomainError> {
        (value > Decimal::ZERO)
            .then_some(Self(value))
            .ok_or(DomainError::NonPositiveValue)
    }

    #[must_use]
    pub const fn value(self) -> Decimal {
        self.0
    }

    pub fn times(self, quantity: Quantity, currency: Currency) -> Result<Money, DomainError> {
        self.0
            .checked_mul(quantity.0)
            .map(|amount| Money { amount, currency })
            .ok_or_else(|| DomainError::Adapter("notional overflow".to_owned()))
    }
}

impl Serialize for Price {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        rust_decimal::serde::str::serialize(&self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for Price {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = rust_decimal::serde::str::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Quantity(Decimal);

impl Quantity {
    pub fn new(value: Decimal) -> Result<Self, DomainError> {
        (value > Decimal::ZERO)
            .then_some(Self(value))
            .ok_or(DomainError::NonPositiveValue)
    }

    #[must_use]
    pub const fn value(self) -> Decimal {
        self.0
    }

    pub fn checked_add(self, other: Self) -> Result<Self, DomainError> {
        self.0
            .checked_add(other.0)
            .and_then(|value| (value > Decimal::ZERO).then_some(Self(value)))
            .ok_or_else(|| DomainError::Adapter("quantity overflow".to_owned()))
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, DomainError> {
        self.0
            .checked_sub(other.0)
            .filter(|value| *value > Decimal::ZERO)
            .map(Self)
            .ok_or(DomainError::NonPositiveValue)
    }

    pub fn is_at_most(self, other: Self) -> bool {
        self.0 <= other.0
    }
}

impl Serialize for Quantity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        rust_decimal::serde::str::serialize(&self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for Quantity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = rust_decimal::serde::str::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}
