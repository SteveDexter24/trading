use secrecy::SecretString;

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
