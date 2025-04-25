pub mod retrieval;

use std::time::Instant;

#[derive(Debug, Clone)]
pub struct KeycloakConfig {
    pub url: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone)]
pub struct KeycloakToken {
    pub access_token: String,
    pub expiry: Instant,
}
