use crate::{KeycloakConfig, KeycloakToken};
use std::collections::HashMap;
use std::future::Future;
use std::ops::Add;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use backon::{ExponentialBuilder, Retryable};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::sync::RwLock;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::StreamExt;
use tracing::{debug, info, warn};

#[derive(Deserialize, Debug)]
struct KeycloakTokenResponse {
    pub access_token: String,
    // Number of seconds until the access_token expires.
    pub expires_in: i32,
}

impl KeycloakTokenResponse {
    pub fn calculate_token_expiry(&self, now: Instant) -> Instant {
        // Factor in a number of seconds to allow for the time to token to be used, so it does not
        // expire between the check to see if it's still valid and the time at which it is used.
        let leeway_seconds: i32 = 10;

        now.add(Duration::from_secs(
            (self.expires_in - leeway_seconds) as u64,
        ))
    }
}

pub async fn retrieve_keycloak_token(
    client: &Client,
    keycloak_config: KeycloakConfig,
) -> Result<KeycloakToken> {
    let keycloak_admin_url = format!(
        "{}/auth/realms/flexys/protocol/openid-connect/token",
        keycloak_config.url
    );

    info!(
        keycloak_endpoint = keycloak_admin_url,
        "Retrieving keycloak token from endpoint",
    );

    let mut params: HashMap<&str, &str> = HashMap::new();

    params.insert("grant_type", "client_credentials");

    let resp = client
        .post(keycloak_admin_url)
        .basic_auth(
            keycloak_config.client_id,
            Some(keycloak_config.client_secret),
        )
        .form(&params)
        .send()
        .await?;

    if resp.status() != StatusCode::OK {
        bail!(
            "Expected 200 OK from keycloak token retrieval. Got status code: {}",
            resp.status()
        );
    }

    let resp_text = resp
        .text()
        .await
        .context("Failed to get text body from response")?;

    let token = serde_json::from_str::<KeycloakTokenResponse>(&resp_text)
        .with_context(|| format!("Failed to convert applied config from endpoint into expected format. Response: {resp_text}."))
        .map(derive_expiry_calculated_keycloak_token)?;

    info!("Successfully retrieved keycloak token.");

    Ok(token)
}

pub async fn keycloak_token_with_retry(
    client: &Client,
    keycloak_config: KeycloakConfig,
    max_retries: usize,
) -> std::result::Result<KeycloakToken, anyhow::Error> {
    (|| async { retrieve_keycloak_token(client, keycloak_config.clone()).await })
        .retry(&ExponentialBuilder::default().with_max_times(max_retries))
        .notify(|err: &anyhow::Error, duration: Duration| {
            warn!(
                "Failed to retrieve keycloak token. Error: {}: {}. Retrying with wait: {:?}",
                err,
                err.root_cause(),
                duration
            )
        })
        .await
}

pub fn refresh_keycloak_token_periodically_in_background<F, Fut>(
    client: Client,
    keycloak_config: KeycloakConfig,
    keycloak_expiry_check_interval_in_seconds: u64,
    keycloak_token: Arc<RwLock<KeycloakToken>>,
    on_token_refresh: F,
) where
    F: Fn(KeycloakToken) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        let mut interval_stream = IntervalStream::new(tokio::time::interval(Duration::from_secs(
            keycloak_expiry_check_interval_in_seconds,
        )));

        let max_retries = 3;

        while let Some(ts) = interval_stream.next().await {
            // Code block scoped rwlock read so that when the block goes out of scope, it is automatically dropped
            let expiry: Instant = {
                let keycloak_token_read_guard = keycloak_token.read().await;

                keycloak_token_read_guard.expiry
            };

            if ts.into_std() > expiry {
                // Separate out logging the fact the token has expired from the writing of a new token to keep exclusive
                // access to an absolute minimum. Scoped to automatically drop the read lock.
                let old_access_token: String = {
                    let token_read_lock = keycloak_token.read().await;

                    token_read_lock.access_token.clone()
                };

                debug!(
                    token = old_access_token,
                    "Keycloak token will expire soon. Retrieving new token.",
                );

                // Code block scoped rwlock write so that when the block goes out of scope, it is automatically dropped.
                // Do this before additional logging and the callback to keep the time when it is exclusively held to
                // the minimum.
                let new_keycloak_token: KeycloakToken = {
                    let new_token =  keycloak_token_with_retry(
                        &client,
                        keycloak_config.clone(),
                        max_retries,
                    )
                        .await
                        .unwrap_or_else(|_| {
                            panic!("Unable to retrieve keycloak token after {max_retries} retries.")
                        });
                    
                    let mut keycloak_token_write_guard = keycloak_token.write().await;

                    *keycloak_token_write_guard = new_token.clone();

                    new_token
                };

                debug!(
                    token = new_keycloak_token.access_token,
                    "Retrieved new keycloak token.",
                );

                on_token_refresh(new_keycloak_token).await;
            }
        }
    });
}

fn derive_expiry_calculated_keycloak_token(response: KeycloakTokenResponse) -> KeycloakToken {
    let calculated_expiry: Instant = response.calculate_token_expiry(Instant::now());

    KeycloakToken {
        access_token: response.access_token,
        expiry: calculated_expiry,
    }
}

#[cfg(test)]
mod tests {
    use crate::retrieval::KeycloakTokenResponse;
    use std::time::Duration;
    use std::time::Instant;

    #[test]
    fn keycloak_token_calculate_token_expiry() {
        let token = KeycloakTokenResponse {
            access_token: "".to_string(),
            expires_in: 80000,
        };

        let now: Instant = Instant::now();

        let expiry: Instant = token.calculate_token_expiry(now);

        assert_eq!(
            expiry.checked_duration_since(now),
            Some(Duration::from_secs(79990))
        )
    }
}
