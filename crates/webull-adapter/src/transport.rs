use crate::WebullCredentials;
use reqwest::Client;
use rumqttc::MqttOptions;
use std::time::Duration;
use tonic::transport::Endpoint;
use trading_domain::DomainError;

pub struct WebullTransport {
    pub http: Client,
    pub mqtt: MqttOptions,
    pub grpc: Endpoint,
    pub credentials: WebullCredentials,
}

impl WebullTransport {
    pub fn sandbox(credentials: WebullCredentials) -> Result<Self, DomainError> {
        Client::builder()
            .https_only(true)
            .build()
            .map_err(adapter_error)
            .map(|http| {
                let mut mqtt =
                    MqttOptions::new("trading-system-sandbox", "data-api.sandbox.webull.com", 443);
                mqtt.set_keep_alive(Duration::from_secs(30));
                Self {
                    http,
                    mqtt,
                    grpc: Endpoint::from_static("https://events-api.sandbox.webull.com"),
                    credentials,
                }
            })
    }
}

fn adapter_error(error: impl std::fmt::Display) -> DomainError {
    DomainError::Adapter(error.to_string())
}
