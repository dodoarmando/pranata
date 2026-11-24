//! Asks the web server to drop its cached public HTML (D-12, D-35).
//!
//! The web server caches rendered public pages. After a content change the
//! `PurgePublicCache` job calls [`Purger::purge_all`]; a failure becomes a
//! failed job attempt, so the purge is retried with backoff.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use secrecy::{ExposeSecret, SecretString};
use url::Url;

use crate::Config;

/// Header that carries the shared purge secret.
pub const TOKEN_HEADER: &str = "x-pranata-purge-token";

/// Path of the web server's purge endpoint.
const PURGE_PATH: &str = "_internal/cache/purge";

/// How long one purge request may take.
const TIMEOUT: Duration = Duration::from_secs(5);

/// Client for the web server's internal purge endpoint.
#[derive(Clone)]
pub struct Purger {
    client: reqwest::Client,
    url: Url,
    token: SecretString,
}

impl std::fmt::Debug for Purger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Purger")
            .field("url", &self.url)
            .field("token", &"[redacted]")
            .finish_non_exhaustive()
    }
}

impl Purger {
    /// Build a purger for `config.web_internal_url`.
    ///
    /// # Errors
    /// Fails when the HTTP client cannot be built or the endpoint URL cannot
    /// be derived from the base URL.
    pub fn new(config: &Config) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .build()
            .context("building the purge client")?;
        let url = config
            .web_internal_url
            .join(PURGE_PATH)
            .context("deriving the purge URL")?;
        Ok(Self {
            client,
            url,
            token: config.purge_token.clone(),
        })
    }

    /// Ask the web server to drop every cached public page.
    ///
    /// # Errors
    /// Fails on a transport error, a timeout or any non-2xx answer. Messages
    /// never contain the token.
    pub async fn purge_all(&self) -> Result<()> {
        let response = self
            .client
            .post(self.url.clone())
            .header(TOKEN_HEADER, self.token.expose_secret())
            .send()
            .await
            .map_err(|error| {
                anyhow::anyhow!("the purge request failed: {}", error.without_url())
            })?;
        let status = response.status();
        if !status.is_success() {
            bail!("the web server answered the purge with {status}");
        }
        Ok(())
    }
}
