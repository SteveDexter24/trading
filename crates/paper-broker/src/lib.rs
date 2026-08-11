//! Deterministic broker simulator. This crate has no live-order transport.

use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::time::Duration;
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

    fn create_fill(
        order: &ApprovedOrder,
        quantity: Quantity,
        price: Price,
        fee: Money,
        suffix: &str,
    ) -> Fill {
        Fill {
            id: Uuid::new_v4(),
            event_id: format!(
                "paper-fill-{}-{suffix}",
                order.order().intent.client_order_id
            ),
            order_id: order.order().id,
            quantity,
            price,
            fee,
            currency: order.order().intent.instrument.currency,
            filled_at: Utc::now(),
        }
    }
}

#[async_trait]
impl BrokerPort for PaperBroker {
    async fn submit(&self, order: &ApprovedOrder) -> Result<BrokerReceipt, DomainError> {
        let client_order_id = order.order().intent.client_order_id;
        if self.model.currency != order.order().intent.instrument.currency {
            return Err(DomainError::CurrencyMismatch {
                left: self.model.currency,
                right: order.order().intent.instrument.currency,
            });
        }
        if self.model.fee.currency() != self.model.currency {
            return Err(DomainError::CurrencyMismatch {
                left: self.model.fee.currency(),
                right: self.model.currency,
            });
        }
        if let Some(receipt) = self.receipts.lock().await.get(&client_order_id).cloned() {
            return Ok(receipt);
        }
        if matches!(self.model.fill_policy, FillPolicy::Reject) {
            return Err(DomainError::Adapter(
                "paper broker configured to reject order".to_owned(),
            ));
        }
        tokio::time::sleep(self.model.latency).await;

        let fills = match self.model.fill_policy {
            FillPolicy::Full => vec![Self::create_fill(
                order,
                order.order().intent.quantity,
                self.model.price,
                self.model.fee,
                "full",
            )],
            FillPolicy::PartialThenFull => {
                let half = Quantity::new(order.order().intent.quantity.value() / Decimal::TWO)?;
                vec![
                    Self::create_fill(
                        order,
                        half,
                        self.model.price,
                        Money::zero(order.order().intent.instrument.currency),
                        "partial",
                    ),
                    Self::create_fill(order, half, self.model.price, self.model.fee, "final"),
                ]
            }
            FillPolicy::Reject => unreachable!("rejection returned before fill generation"),
        };
        let receipt = BrokerReceipt {
            broker_order_id: format!("paper-{client_order_id}"),
            acknowledged_at: Utc::now(),
            fills,
        };
        self.receipts
            .lock()
            .await
            .insert(client_order_id, receipt.clone());
        Ok(receipt)
    }
}
