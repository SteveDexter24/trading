use secrecy::SecretString;
use trading_domain::DomainError;

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
