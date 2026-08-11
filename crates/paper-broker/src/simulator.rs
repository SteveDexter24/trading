use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use std::{collections::HashMap, time::Duration};
use tokio::sync::Mutex;
use trading_domain::{
    ApprovedOrder, BrokerPort, BrokerReceipt, Currency, DomainError, Fill, Money, Price, Quantity,
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
            let half = Quantity::new(quantity.value() / Decimal::TWO)?;
            Ok(vec![
                PlannedFill {
                    quantity: half,
                    fee: Money::zero(currency),
                    event_suffix: "partial",
                },
                PlannedFill {
                    quantity: half,
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
        if let Some(receipt) = self.receipts.lock().await.get(&client_order_id).cloned() {
            return Ok(receipt);
        }

        let plans = plan_fills(
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
        self.receipts
            .lock()
            .await
            .insert(client_order_id, receipt.clone());
        Ok(receipt)
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
