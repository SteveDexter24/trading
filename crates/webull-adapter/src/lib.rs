//! Webull anti-corruption boundary.
//!
//! Official documentation currently describes HTTP trading/data, MQTT market
//! streams, and gRPC trade events. HK payloads are deliberately not encoded
//! here until an official HK contract is available and reviewed.

use async_trait::async_trait;
use reqwest::Client;
use rumqttc::MqttOptions;
use secrecy::SecretString;
use std::time::Duration;
use tonic::transport::Endpoint;
use trading_domain::{ApprovedOrder, BrokerPort, BrokerReceipt, DomainError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebullEnvironment {
    Sandbox,
    Production,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebullMarket {
    HongKong,
    UnitedStates,
}

pub struct WebullCredentials {
    pub app_key: SecretString,
    pub app_secret: SecretString,
    pub access_token: Option<SecretString>,
}

pub trait RequestSigner: Send + Sync {
    fn sign(&self, request: &SigningInput) -> Result<SignedHeaders, DomainError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningInput {
    pub method: String,
    pub host: String,
    pub path: String,
    pub canonical_query: String,
    pub body_digest: String,
    pub utc_timestamp: String,
    pub nonce: String,
}

pub struct SignedHeaders {
    pub values: Vec<(String, SecretString)>,
}

pub struct WebullTransport {
    pub http: Client,
    pub mqtt: MqttOptions,
    pub grpc: Endpoint,
    pub credentials: WebullCredentials,
}

impl WebullTransport {
    pub fn sandbox(credentials: WebullCredentials) -> Result<Self, DomainError> {
        let http = Client::builder()
            .https_only(true)
            .build()
            .map_err(adapter_error)?;
        let mut mqtt = MqttOptions::new(
            "trading-system-sandbox",
            "data-api.sandbox.webull.com",
            443,
        );
        mqtt.set_keep_alive(Duration::from_secs(30));
        let grpc = Endpoint::from_static("https://events-api.sandbox.webull.com");
        Ok(Self {
            http,
            mqtt,
            grpc,
            credentials,
        })
    }
}

pub struct WebullBroker {
    environment: WebullEnvironment,
    market: WebullMarket,
    _transport: WebullTransport,
}

impl WebullBroker {
    pub fn sandbox(
        market: WebullMarket,
        transport: WebullTransport,
    ) -> Result<Self, DomainError> {
        if market == WebullMarket::HongKong {
            return Err(DomainError::Adapter(
                "official Webull HK order contract has not been verified".to_owned(),
            ));
        }
        Ok(Self {
            environment: WebullEnvironment::Sandbox,
            market,
            _transport: transport,
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

fn adapter_error(error: impl std::fmt::Display) -> DomainError {
    DomainError::Adapter(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
