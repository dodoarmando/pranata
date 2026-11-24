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

/// Path segments of the web server's purge endpoint.
const PURGE_PATH: [&str; 3] = ["_internal", "cache", "purge"];

/// The purge endpoint below `base`, keeping any path `base` already has.
fn purge_url(base: &Url) -> Result<Url> {
    let mut url = base.clone();
    url.path_segments_mut()
        .map_err(|()| anyhow::anyhow!("the web internal URL cannot carry a path"))?
        .pop_if_empty()
        .extend(PURGE_PATH);
    Ok(url)
}

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
        let url = purge_url(&config.web_internal_url).context("deriving the purge URL")?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn purge_url_of(base: &str) -> String {
        purge_url(&base.parse().unwrap()).unwrap().to_string()
    }

    #[test]
    fn purge_url_appends_to_the_base_path() {
        assert_eq!(
            purge_url_of("http://web:3000"),
            "http://web:3000/_internal/cache/purge"
        );
        assert_eq!(
            purge_url_of("http://web/app"),
            "http://web/app/_internal/cache/purge"
        );
        assert_eq!(
            purge_url_of("http://web/app/"),
            "http://web/app/_internal/cache/purge"
        );
    }
}
