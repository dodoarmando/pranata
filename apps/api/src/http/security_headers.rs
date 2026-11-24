use axum::{
    http::{HeaderName, HeaderValue},
    response::Response,
};

const HEADERS: [(&str, &str); 4] = [
    ("strict-transport-security", "max-age=31536000"),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "strict-origin-when-cross-origin"),
    ("x-frame-options", "DENY"),
];

/// Add the fixed security headers to every response (D-36). The CSP with a
/// per-response nonce belongs to the web app.
pub async fn apply(mut response: Response) -> Response {
    for (name, value) in HEADERS {
        response.headers_mut().insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    response
}
