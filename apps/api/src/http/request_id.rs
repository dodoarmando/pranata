use axum::{Router, http::Request};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::{DefaultOnResponse, TraceLayer},
};
use tracing::{Level, info_span};

pub const HEADER: &str = "x-request-id";

/// Give every request an ID (keeping a caller-supplied one), put it on the
/// tracing span and echo it back in the response.
pub fn layer(router: Router) -> Router {
    router
        .layer(PropagateRequestIdLayer::new(
            axum::http::HeaderName::from_static(HEADER),
        ))
        .layer(
            TraceLayer::new_for_http()
                .on_response(DefaultOnResponse::new().level(Level::INFO))
                .make_span_with(|request: &Request<_>| {
                    let request_id = request
                        .headers()
                        .get(HEADER)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or_default();
                    info_span!(
                        "request",
                        %request_id,
                        method = %request.method(),
                        path = %request.uri().path(),
                    )
                }),
        )
        .layer(SetRequestIdLayer::new(
            axum::http::HeaderName::from_static(HEADER),
            MakeRequestUuid,
        ))
}
