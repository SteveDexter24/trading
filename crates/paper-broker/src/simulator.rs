use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use std::{collections::HashMap, time::Duration};
use tokio::sync::Mutex;
use trading_domain::{
    ApprovedOrder, BrokerPort, BrokerReceipt, Currency, DomainError, Fill, Instrument, Money,
    Price, Quantity,
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum FillPolicy {
    Full,
    PartialThenFull,
    Reject,
}

#[derive(Debug, Clone, Copy)]
pub struct ExecutionModel {
    pub price: Price,
    pub currency: Currency,
    pub fee: Money,
    pub latency: Duration,
    pub fill_policy: FillPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFill {
    pub quantity: Quantity,
    pub fee: Money,
    pub event_suffix: &'static str,
}

pub fn plan_fills(
    instrument: &Instrument,
    quantity: Quantity,
    currency: Currency,
    fee: Money,
    policy: FillPolicy,
) -> Result<Vec<PlannedFill>, DomainError> {
    match policy {
        FillPolicy::Full => Ok(vec![PlannedFill {
            quantity,
            fee,
            event_suffix: "full",
        }]),
        FillPolicy::PartialThenFull => {
            let (first, second) = split_quantity(instrument, quantity)?;
            Ok(vec![
                PlannedFill {
                    quantity: first,
                    fee: Money::zero(currency),
                    event_suffix: "partial",
                },
                PlannedFill {
                    quantity: second,
                    fee,
                    event_suffix: "final",
                },
            ])
        }
        FillPolicy::Reject => Err(DomainError::Adapter(
            "paper broker configured to reject order".to_owned(),
        )),
    }
}

fn split_quantity(
    instrument: &Instrument,
    quantity: Quantity,
) -> Result<(Quantity, Quantity), DomainError> {
    let half = quantity.value() / Decimal::TWO;
    let first = if instrument.fractional_supported {
        Quantity::new(half.round_dp(8))?
    } else {
        let lot = instrument.board_lot.value();
        let lots = (half / lot).floor();
        let first_value = lots * lot;
        if first_value <= Decimal::ZERO || first_value >= quantity.value() {
            return Err(DomainError::Adapter(
                "unable to split quantity into valid board lots".to_owned(),
            ));
        }
        Quantity::new(first_value)?
    };
    let second = quantity.checked_sub(first)?;
    Ok((first, second))
}

#[derive(Debug)]
pub struct PaperBroker {
    model: ExecutionModel,
    receipts: Mutex<HashMap<Uuid, BrokerReceipt>>,
}

impl PaperBroker {
    #[must_use]
    pub fn new(model: ExecutionModel) -> Self {
        Self {
            model,
            receipts: Mutex::new(HashMap::new()),
        }
    }

    fn create_fill(&self, order: &ApprovedOrder, plan: PlannedFill) -> Fill {
        Fill {
            id: Uuid::new_v4(),
            event_id: format!(
                "paper-fill-{}-{}",
                order.order().intent.client_order_id,
                plan.event_suffix
            ),
            order_id: order.order().id,
            quantity: plan.quantity,
            price: self.model.price,
            fee: plan.fee,
            currency: self.model.currency,
            filled_at: Utc::now(),
        }
    }
}

#[async_trait]
impl BrokerPort for PaperBroker {
    async fn submit(&self, order: &ApprovedOrder) -> Result<BrokerReceipt, DomainError> {
        let client_order_id = order.order().intent.client_order_id;
        validate_currency(self.model, order)?;

        {
            let receipts = self.receipts.lock().await;
            if let Some(receipt) = receipts.get(&client_order_id) {
                return Ok(receipt.clone());
            }
        }

        let plans = plan_fills(
            &order.order().intent.instrument,
            order.order().intent.quantity,
            self.model.currency,
            self.model.fee,
            self.model.fill_policy,
        )?;
        tokio::time::sleep(self.model.latency).await;

        let receipt = BrokerReceipt {
            broker_order_id: format!("paper-{client_order_id}"),
            acknowledged_at: Utc::now(),
            fills: plans
                .into_iter()
                .map(|plan| self.create_fill(order, plan))
                .collect(),
        };

        let mut receipts = self.receipts.lock().await;
        Ok(receipts.entry(client_order_id).or_insert(receipt).clone())
    }
}

fn validate_currency(model: ExecutionModel, order: &ApprovedOrder) -> Result<(), DomainError> {
    let order_currency = order.order().intent.instrument.currency;
    if model.currency != order_currency {
        return Err(DomainError::CurrencyMismatch {
            left: model.currency,
            right: order_currency,
        });
    }
    (model.fee.currency() == model.currency)
        .then_some(())
        .ok_or(DomainError::CurrencyMismatch {
            left: model.fee.currency(),
            right: model.currency,
        })
}
