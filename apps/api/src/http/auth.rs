use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{AppendHeaders, IntoResponse},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    ApiError, AppState,
    audit::{self, AuditAction, NewAuditEntry},
    auth::{
        CurrentUser, access_link, password,
        session::{self, AuthSession},
    },
    error::Problem,
    ids::UserId,
};

/// Longest password sign-in will hash. Anything longer cannot be valid.
const MAX_SIGN_IN_PASSWORD: usize = password::MAX_LENGTH;

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    /// Keep the session for 30 days instead of until the browser closes.
    #[serde(default)]
    pub remember: bool,
}

/// The signed-in User and the CSRF token to send as `x-csrf-token`.
#[derive(Debug, Serialize, ToSchema)]
pub struct SessionInfo {
    pub user: CurrentUser,
    pub csrf_token: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetPasswordRequest {
    pub password: String,
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
        (status = 401, description = "Wrong username or password (code invalid_credentials)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
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

    // Verify on a blocking thread. Every failure path costs one argon2 run, so
    // timing does not tell an unknown user from a wrong password.
    let candidate = request.password;
    let hash = account
        .as_ref()
        .filter(|account| account.is_active)
        .and_then(|account| account.password_hash.clone());
    let verified = tokio::task::spawn_blocking(move || match hash {
        Some(hash) if candidate.chars().count() <= MAX_SIGN_IN_PASSWORD => {
            password::verify_password(&candidate, &hash)
        }
        _ => {
            password::verify_against_dummy(&candidate);
            false
        }
    })
    .await
    .map_err(anyhow::Error::from)?;
    let Some(account) = account.filter(|_| verified) else {
        return Err(invalid);
    };

    let now = state.clock.now();
    let mut tx = state.db.begin().await?;
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
/// 404 `access_link_not_found`, 410 `access_link_used` or
/// `access_link_expired`, 422 `password_length`.
#[utoipa::path(
    post,
    path = "/auth/access-links/{token}/password",
    operation_id = "setPasswordWithAccessLink",
    tag = "auth",
    params(("token" = String, Path, description = "The Access Link token")),
    request_body = SetPasswordRequest,
    responses(
        (status = 204, description = "Password set; the link is used up"),
        (status = 404, description = "Unknown link (code access_link_not_found)", body = Problem, content_type = "application/problem+json"),
        (status = 410, description = "Link used or expired (codes access_link_used, access_link_expired)", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Password length rule broken (code password_length)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn set_password_with_access_link(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Json(request): Json<SetPasswordRequest>,
) -> Result<StatusCode, ApiError> {
    access_link::redeem(&state.db, &token, &request.password, state.clock.now()).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(login))
        .routes(routes!(logout))
        .routes(routes!(get_me))
        .routes(routes!(set_password_with_access_link))
}
