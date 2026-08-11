use crate::{WebullEnvironment, WebullMarket, WebullTransport};
use async_trait::async_trait;
use trading_domain::{ApprovedOrder, BrokerPort, BrokerReceipt, DomainError};

pub struct WebullBroker {
    environment: WebullEnvironment,
    market: WebullMarket,
    _transport: WebullTransport,
}

impl WebullBroker {
    pub fn sandbox(market: WebullMarket, transport: WebullTransport) -> Result<Self, DomainError> {
        (market != WebullMarket::HongKong)
            .then_some(Self {
                environment: WebullEnvironment::Sandbox,
                market,
                _transport: transport,
            })
            .ok_or_else(|| {
                DomainError::Adapter(
                    "official Webull HK order contract has not been verified".to_owned(),
                )
            })
    }

    #[must_use]
    pub const fn environment(&self) -> WebullEnvironment {
        self.environment
    }

    #[must_use]
    pub const fn market(&self) -> WebullMarket {
        self.market
    }
}

#[async_trait]
impl BrokerPort for WebullBroker {
    async fn submit(&self, _order: &ApprovedOrder) -> Result<BrokerReceipt, DomainError> {
        Err(DomainError::Adapter(
            "Webull submission is disabled in the paper-only release".to_owned(),
        ))
    }
}

pub fn production_broker_disabled() -> Result<WebullBroker, DomainError> {
    Err(DomainError::Adapter(
        "production Webull order submission is disabled by code".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WebullCredentials;
    use secrecy::SecretString;

    #[test]
    fn production_order_path_cannot_be_constructed() {
        assert!(production_broker_disabled().is_err());
    }

    #[test]
    fn unverified_hk_contract_cannot_be_constructed() {
        let credentials = WebullCredentials {
            app_key: SecretString::from("test-key".to_owned()),
            app_secret: SecretString::from("test-secret".to_owned()),
            access_token: None,
        };
        let transport = WebullTransport::sandbox(credentials).expect("transport");
        assert!(WebullBroker::sandbox(WebullMarket::HongKong, transport).is_err());
    }
}
