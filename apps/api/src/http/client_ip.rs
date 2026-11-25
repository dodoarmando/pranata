//! The client address behind Caddy, and the per-IP limit on unauthenticated
//! auth requests (D-34).

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
};

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, Request, Response},
    response::IntoResponse,
};
use governor::middleware::NoOpMiddleware;
use tower_governor::{
    GovernorError, GovernorLayer,
    governor::{GovernorConfig, GovernorConfigBuilder},
    key_extractor::KeyExtractor,
};

use crate::ApiError;

/// The address the request came from: the last parseable entry of
/// `X-Forwarded-For`, else the peer address.
///
/// Trusting the header is safe because only Caddy can reach the API: on an
/// Installation the API listens on the pod network only, and in development
/// on 127.0.0.1. Caddy replaces the header with the real client address, so
/// its last entry is the one nobody else wrote.
#[must_use]
pub fn client_ip(headers: &HeaderMap, peer: Option<SocketAddr>) -> Option<IpAddr> {
    headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|entry| entry.trim().parse::<IpAddr>().ok())
        .next_back()
        .or_else(|| peer.map(|peer| peer.ip()))
}

/// The peer address `axum::serve` records when the service is built with
/// `into_make_service_with_connect_info::<SocketAddr>()`.
#[must_use]
pub fn peer_of<B>(request: &Request<B>) -> Option<SocketAddr> {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0)
}

/// Rate-limit key: the client address, with an IPv6 address reduced to its
/// /64 prefix because one home or mobile client usually holds a whole /64. A
/// request without any address shares the unspecified address.
#[derive(Debug, Clone, Copy)]
pub struct ClientIpKey;

impl ClientIpKey {
    fn reduce(ip: IpAddr) -> IpAddr {
        match ip {
            IpAddr::V4(_) => ip,
            IpAddr::V6(v6) => {
                let [a, b, c, d, ..] = v6.segments();
                IpAddr::V6(Ipv6Addr::new(a, b, c, d, 0, 0, 0, 0))
            }
        }
    }
}

impl KeyExtractor for ClientIpKey {
    type Key = IpAddr;

    fn extract<T>(&self, request: &Request<T>) -> Result<IpAddr, GovernorError> {
        let ip = client_ip(request.headers(), peer_of(request))
            .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        Ok(Self::reduce(ip))
    }
}

/// The per-IP limit: a burst of 20, refilled one request every 3 seconds.
pub type AuthLimiter = Arc<GovernorConfig<ClientIpKey, NoOpMiddleware>>;

/// A fresh limiter. Every app gets its own, so tests do not share buckets.
///
/// # Panics
/// Never: the configuration is constant and valid.
#[must_use]
pub fn auth_limiter() -> AuthLimiter {
    Arc::new(
        GovernorConfigBuilder::default()
            .per_second(3)
            .burst_size(20)
            .key_extractor(ClientIpKey)
            .finish()
            .expect("a non-zero period and burst size"),
    )
}

/// The layer that answers 429 `too_many_requests` once a client address has
/// used up its bucket.
#[must_use]
pub fn auth_limit_layer(limiter: &AuthLimiter) -> GovernorLayer<ClientIpKey, NoOpMiddleware, Body> {
    GovernorLayer::new(limiter.clone()).error_handler(|error| -> Response<Body> {
        match error {
            GovernorError::TooManyRequests { wait_time, .. } => ApiError::TooManyRequests {
                code: "too_many_requests",
                retry_after: wait_time,
            }
            .into_response(),
            other => ApiError::Internal(anyhow::anyhow!("rate limiter: {other}")).into_response(),
        }
    })
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn headers(forwarded_for: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_str(forwarded_for).unwrap(),
        );
        headers
    }

    #[test]
    fn the_last_forwarded_address_wins() {
        assert_eq!(
            client_ip(&headers("198.51.100.1, 203.0.113.7"), None),
            Some("203.0.113.7".parse().unwrap())
        );
    }

    #[test]
    fn garbage_falls_back_to_the_peer() {
        let peer: SocketAddr = "192.0.2.9:4000".parse().unwrap();
        assert_eq!(
            client_ip(&headers("not an address"), Some(peer)),
            Some(peer.ip())
        );
    }

    #[test]
    fn nothing_gives_no_address() {
        assert_eq!(client_ip(&HeaderMap::new(), None), None);
    }

    #[test]
    fn ipv6_clients_share_a_key_per_64_prefix() {
        let key = |ip: &str| {
            let request = Request::builder()
                .header("x-forwarded-for", ip)
                .body(())
                .unwrap();
            ClientIpKey.extract(&request).unwrap()
        };
        assert_eq!(key("2001:db8:1:2:3:4:5:6"), key("2001:db8:1:2:ffff::1"));
        assert_ne!(key("2001:db8:1:2:3:4:5:6"), key("2001:db8:1:3::1"));
    }
}
