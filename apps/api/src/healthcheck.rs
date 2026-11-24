//! `pranata healthcheck`: asks the API on this host whether it is healthy.
//!
//! The runtime image is distroless and has no curl, so the container's
//! `HealthCmd` runs the binary itself.

use std::{net::SocketAddr, time::Duration};

use anyhow::{Context, Result, bail};

const DEFAULT_LISTEN: &str = "127.0.0.1:8080";
const TIMEOUT: Duration = Duration::from_secs(3);

/// The health URL for the address the API listens on (`PRANATA_LISTEN`).
/// A wildcard address is reached through loopback.
///
/// # Errors
/// Fails when the address is not a socket address.
pub fn url(listen: Option<&str>) -> Result<String> {
    let listen = listen.unwrap_or(DEFAULT_LISTEN);
    let mut addr: SocketAddr = listen
        .parse()
        .with_context(|| format!("PRANATA_LISTEN is not a socket address: {listen}"))?;
    if addr.ip().is_unspecified() {
        addr.set_ip(if addr.is_ipv4() {
            [127, 0, 0, 1].into()
        } else {
            std::net::Ipv6Addr::LOCALHOST.into()
        });
    }
    Ok(format!("http://{addr}/api/healthz"))
}

/// Request the health endpoint once.
///
/// # Errors
/// Fails when the API does not answer 200 in time.
pub async fn run(listen: Option<&str>) -> Result<()> {
    let url = url(listen)?;
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .context("building the health client")?;
    let response = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("requesting {url}"))?;
    if !response.status().is_success() {
        bail!("{url} answered {}", response.status());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::url;

    #[test]
    fn defaults_to_loopback() {
        assert_eq!(url(None).unwrap(), "http://127.0.0.1:8080/api/healthz");
    }

    #[test]
    fn wildcard_becomes_loopback() {
        assert_eq!(
            url(Some("0.0.0.0:9000")).unwrap(),
            "http://127.0.0.1:9000/api/healthz"
        );
        assert_eq!(
            url(Some("[::]:9000")).unwrap(),
            "http://[::1]:9000/api/healthz"
        );
    }

    #[test]
    fn keeps_a_specific_address() {
        assert_eq!(
            url(Some("10.0.0.5:8080")).unwrap(),
            "http://10.0.0.5:8080/api/healthz"
        );
    }

    #[test]
    fn rejects_garbage() {
        assert!(url(Some("nope")).is_err());
    }
}
