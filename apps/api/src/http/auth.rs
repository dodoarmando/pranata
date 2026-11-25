use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{AppendHeaders, IntoResponse},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    ApiError, AppState,
    audit::{self, AuditAction, NewAuditEntry},
    auth::{
        self, CurrentUser, access_link, password,
        session::{self, AuthSession},
    },
    error::Problem,
    http::json::ApiJson,
    ids::UserId,
};

/// What sign-in verifies for `password`. A password longer than
/// [`password::MAX_LENGTH`] cannot be valid, so a fixed placeholder takes its
/// place: the run still costs one argon2 verification, but never on attacker-sized input.
fn verification_input(password: &str) -> &str {
    if password.chars().count() > password::MAX_LENGTH {
        ""
    } else {
        password
    }
}

#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    /// Keep the session for 30 days instead of until the browser closes.
    #[serde(default)]
    pub remember: bool,
}

/// The signed-in User and the CSRF token to send as `x-csrf-token`.
#[derive(Serialize, ToSchema)]
pub struct SessionInfo {
    pub user: CurrentUser,
    pub csrf_token: String,
}

/// The Access Link token travels in the body, never in the URL, so request
/// logs and proxies cannot capture it.
#[derive(Deserialize, ToSchema)]
pub struct RedeemAccessLinkRequest {
    pub token: String,
    pub password: String,
}

impl std::fmt::Debug for LoginRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginRequest")
            .field("username", &self.username)
            .field("password", &"[redacted]")
            .field("remember", &self.remember)
            .finish()
    }
}

impl std::fmt::Debug for RedeemAccessLinkRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedeemAccessLinkRequest")
            .field("token", &"[redacted]")
            .field("password", &"[redacted]")
            .finish()
    }
}

impl std::fmt::Debug for SessionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionInfo")
            .field("user", &self.user)
            .field("csrf_token", &"[redacted]")
            .finish()
    }
}

fn no_store() -> AppendHeaders<[(header::HeaderName, HeaderValue); 1]> {
    AppendHeaders([(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))])
}

struct Account {
    id: UserId,
    username: String,
    display_name: String,
    password_hash: Option<String>,
    is_active: bool,
}

/// Sign in with a username and password.
///
/// # Errors
/// 401 `invalid_credentials`, identical for an unknown user and a wrong
/// password.
#[utoipa::path(
    post,
    path = "/auth/login",
    operation_id = "login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Signed in; the session cookie is set", body = SessionInfo),
        (status = 400, description = "Body is not valid JSON (code invalid_json)", body = Problem, content_type = "application/problem+json"),
        (status = 401, description = "Wrong username or password (code invalid_credentials)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(request): ApiJson<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let invalid = ApiError::Unauthorized("invalid_credentials");
    let username = request.username.trim().to_lowercase();
    let account = sqlx::query_as!(
        Account,
        r#"SELECT id AS "id: UserId", username, display_name, password_hash, is_active
           FROM users WHERE username = $1"#,
        username,
    )
    .fetch_optional(&state.db)
    .await?;

    // Every failure path costs one argon2 run (the dummy hash stands in for
    // a missing or unusable one, a fixed placeholder for an overlong password), so timing does not tell an unknown user
    // from a wrong password. Runs are bounded by the hash limiter.
    let hash = account
        .as_ref()
        .filter(|account| account.is_active)
        .and_then(|account| account.password_hash.clone());
    let verified = state
        .hasher
        .verify(verification_input(&request.password).to_owned(), hash)
        .await?;
    let Some(account) = account.filter(|_| verified) else {
        return Err(invalid);
    };

    let now = state.clock.now();
    let mut tx = state.db.begin().await?;
    // Signing in again replaces the session this browser already holds.
    if let Some(old) = session::token_from_headers(&headers) {
        session::delete(&mut tx, &auth::hash_token(&old)).await?;
    }
    let new = session::create(&mut tx, account.id, request.remember, now).await?;
    audit::record(
        &mut tx,
        NewAuditEntry {
            actor: Some(account.id),
            action: AuditAction::SignIn,
            object_type: "user",
            object_id: account.id.0.to_string(),
            summary: format!("{} signed in", account.username),
            changes: Vec::new(),
        },
    )
    .await?;
    tx.commit().await?;

    let body = SessionInfo {
        user: CurrentUser {
            id: account.id,
            username: account.username,
            display_name: account.display_name,
        },
        csrf_token: new.csrf_token,
    };
    Ok((
        AppendHeaders([(
            header::SET_COOKIE,
            session::cookie_value(&new.token, request.remember),
        )]),
        no_store(),
        Json(body),
    ))
}

/// End the current session.
///
/// # Errors
/// 401 `unauthenticated`; 403 `csrf_mismatch` without the CSRF header.
#[utoipa::path(
    post,
    path = "/auth/logout",
    operation_id = "logout",
    tag = "auth",
    params(("x-csrf-token" = String, Header, description = "The CSRF token from sign-in")),
    security(("session" = [])),
    responses(
        (status = 204, description = "Signed out; the session cookie is cleared"),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Missing or wrong CSRF token (code csrf_mismatch)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn logout(
    State(state): State<AppState>,
    session: AuthSession,
) -> Result<impl IntoResponse, ApiError> {
    let mut tx = state.db.begin().await?;
    session::delete(&mut tx, &session.token_hash).await?;
    audit::record(
        &mut tx,
        NewAuditEntry {
            actor: Some(session.user.id),
            action: AuditAction::SignOut,
            object_type: "user",
            object_id: session.user.id.0.to_string(),
            summary: format!("{} signed out", session.user.username),
            changes: Vec::new(),
        },
    )
    .await?;
    tx.commit().await?;
    Ok((
        StatusCode::NO_CONTENT,
        AppendHeaders([(header::SET_COOKIE, session::clear_cookie_value())]),
    ))
}

/// The signed-in User and their CSRF token.
///
/// # Errors
/// 401 `unauthenticated`.
#[utoipa::path(
    get,
    path = "/auth/me",
    operation_id = "getMe",
    tag = "auth",
    security(("session" = [])),
    responses(
        (status = 200, description = "The current session", body = SessionInfo),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn get_me(session: AuthSession) -> impl IntoResponse {
    (
        no_store(),
        Json(SessionInfo {
            user: session.user,
            csrf_token: session.csrf_token,
        }),
    )
}

/// Set a password with an Access Link. The link works once.
///
/// # Errors
/// 404 `access_link_not_found`, 410 `access_link_used`,
/// `access_link_revoked` or `access_link_expired`, 422 `password_length`.
#[utoipa::path(
    post,
    path = "/auth/access-links/redeem",
    operation_id = "setPasswordWithAccessLink",
    tag = "auth",
    request_body = RedeemAccessLinkRequest,
    responses(
        (status = 204, description = "Password set; the link is used up"),
        (status = 404, description = "Unknown link (code access_link_not_found)", body = Problem, content_type = "application/problem+json"),
        (status = 410, description = "Link used or expired (codes access_link_used, access_link_revoked, access_link_expired)", body = Problem, content_type = "application/problem+json"),
        (status = 415, description = "Request is not marked as JSON (code json_required)", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Password length rule broken (code password_length)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn set_password_with_access_link(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<RedeemAccessLinkRequest>,
) -> Result<StatusCode, ApiError> {
    access_link::redeem(
        &state.db,
        &state.hasher,
        &request.token,
        &request.password,
        state.clock.now(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(login))
        .routes(routes!(logout))
        .routes(routes!(get_me))
        .routes(routes!(set_password_with_access_link))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_input_keeps_passwords_up_to_the_limit() {
        let at_limit = "é".repeat(password::MAX_LENGTH);
        assert_eq!(verification_input(&at_limit), at_limit);
    }

    #[test]
    fn verification_input_replaces_longer_passwords_with_a_placeholder() {
        let over_limit = "é".repeat(password::MAX_LENGTH + 1);
        assert_eq!(verification_input(&over_limit), "");
    }
}
