//! CSRF protection for state-changing requests (D-30).
//!
//! Every non-safe request to `/api/admin/*` and to `/api/auth/logout` must send
//! the session's CSRF token in `x-csrf-token` (the web app gets it from sign-in
//! and `GET /api/auth/me`). Otherwise the answer is 403 `csrf_mismatch`.
//!
//! The guard is applied to the whole application in [`crate::http::router`] and
//! matches on the request path, so a route added under `/api/admin` is covered
//! without any wiring: later tasks mount their admin routes in
//! `http::api_router` as usual and take [`crate::auth::CurrentUser`] in the
//! handler. Do not add `/api/admin` routes outside that router.

use axum::{
    extract::Request,
    http::Method,
    middleware::Next,
    response::{IntoResponse, Response},
};
use subtle::ConstantTimeEq;

use super::session::{AuthSession, Resolved, not_resolved};
use crate::ApiError;

/// Header that carries the CSRF token.
pub const HEADER: &str = "x-csrf-token";

/// Whether a request must carry the CSRF token.
#[must_use]
pub fn is_protected(method: &Method, path: &str) -> bool {
    let safe = matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    );
    let in_scope =
        path == "/api/admin" || path.starts_with("/api/admin/") || path == "/api/auth/logout";
    in_scope && !safe
}

/// Middleware that rejects protected requests with a missing or wrong token.
/// It needs [`super::session::resolve`] to run first (otherwise 500, so a
/// miswired router fails closed); without a session the answer is 401
/// `unauthenticated`.
pub async fn guard(request: Request, next: Next) -> Response {
    if is_protected(request.method(), request.uri().path()) {
        if request.extensions().get::<Resolved>().is_none() {
            return not_resolved().into_response();
        }
        let Some(session) = request.extensions().get::<AuthSession>() else {
            return ApiError::Unauthorized("unauthenticated").into_response();
        };
        let sent = request
            .headers()
            .get(HEADER)
            .map_or(&[][..], |value| value.as_bytes());
        if !bool::from(sent.ct_eq(session.csrf_token.as_bytes())) {
            return ApiError::Forbidden("csrf_mismatch").into_response();
        }
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_covers_unsafe_admin_and_logout_only() {
        let post = Method::POST;
        assert!(is_protected(&post, "/api/admin"));
        assert!(is_protected(&post, "/api/admin/pages/1"));
        assert!(is_protected(&Method::DELETE, "/api/admin/pages/1"));
        assert!(is_protected(&post, "/api/auth/logout"));

        assert!(!is_protected(&Method::GET, "/api/admin/pages"));
        assert!(!is_protected(&Method::GET, "/api/auth/logout"));
        assert!(!is_protected(&post, "/api/auth/login"));
        assert!(!is_protected(&post, "/api/administrators"));
        assert!(!is_protected(&post, "/api/auth/access-links/x/password"));
    }
}
