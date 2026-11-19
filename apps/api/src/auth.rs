use std::fmt::Write;

use axum::{
    Json,
    extract::{FromRequestParts, State},
    http::{HeaderMap, StatusCode, header, request::Parts},
    response::{AppendHeaders, IntoResponse},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    ApiError,
    audit::{self, Actor},
    password::{self, PasswordProblem, Verdict},
};

const COOKIE: &str = "__Host-session";

/// A random 256-bit token as 64 hex digits, and the SHA-256 hash that is stored instead.
fn new_token() -> (String, Vec<u8>) {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the OS random number generator is unavailable");
    let token = bytes.iter().fold(String::with_capacity(64), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    });
    let hash = token_hash(&token);
    (token, hash)
}

fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token).to_vec()
}

/// Issues a single-use Access Link valid for 24 hours, voiding the User's previous one.
///
/// # Errors
/// When the database write fails.
pub async fn create_access_link(
    conn: &mut PgConnection,
    actor: Actor,
    user_id: Uuid,
) -> sqlx::Result<String> {
    let (token, hash) = new_token();
    sqlx::query!(
        "INSERT INTO access_links (token_hash, user_id, expires_at)
         VALUES ($1, $2, now() + interval '24 hours')
         ON CONFLICT (user_id) DO UPDATE SET token_hash = EXCLUDED.token_hash, expires_at = EXCLUDED.expires_at",
        hash,
        user_id,
    )
    .execute(&mut *conn)
    .await?;
    audit::record(
        conn,
        actor,
        "access_link.create",
        ("user", Some(&user_id.to_string())),
        json!({}),
    )
    .await?;
    Ok(token)
}

#[derive(Deserialize, ToSchema)]
pub struct UseAccessLink {
    /// The token after `#` in the Access Link.
    token: String,
    password: String,
}

#[derive(Serialize, ToSchema)]
pub struct PasswordRejected {
    pub(crate) problem: PasswordProblem,
}

/// Sets the User's password from their Access Link, which is then used up.
///
/// # Errors
/// `404` for an unknown, used or expired link, `422` for a refused password.
#[utoipa::path(
    post,
    path = "/auth/access-link",
    operation_id = "useAccessLink",
    tag = "auth",
    request_body = UseAccessLink,
    responses(
        (status = NO_CONTENT, description = "The password is set"),
        (status = NOT_FOUND, description = "The Access Link is unknown, used or expired"),
        (status = UNPROCESSABLE_ENTITY, description = "The password is refused", body = PasswordRejected),
    ),
)]
pub async fn use_access_link(
    State(pool): State<PgPool>,
    Json(body): Json<UseAccessLink>,
) -> Result<StatusCode, ApiError> {
    let hash = token_hash(&body.token);
    let mut tx = pool.begin().await?;
    let user = sqlx::query!(
        "SELECT users.id, users.username FROM access_links JOIN users ON users.id = access_links.user_id
         WHERE token_hash = $1 AND expires_at > now() FOR UPDATE OF access_links",
        hash,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let village = sqlx::query_scalar!("SELECT name FROM village")
        .fetch_optional(&mut *tx)
        .await?;
    password::check(&body.password, &user.username, village.as_deref())
        .map_err(ApiError::PasswordRejected)?;

    let password_hash = password::hash(body.password).await?;
    sqlx::query!(
        "UPDATE users SET password_hash = $1 WHERE id = $2",
        password_hash,
        user.id,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM access_links WHERE token_hash = $1", hash)
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        Actor::User(user.id),
        "access_link.use",
        ("user", Some(&user.id.to_string())),
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub struct SignIn {
    username: String,
    password: String,
    /// "Ingat saya": 7 days idle and 30 days absolute instead of 30 minutes and 12 hours.
    remember: bool,
}

/// Signs in with a username and password, starting a session held in the `__Host-session` cookie.
///
/// # Errors
/// `401`, the same for every wrong username or password.
#[utoipa::path(
    post,
    path = "/auth/sign-in",
    operation_id = "signIn",
    tag = "auth",
    request_body = SignIn,
    responses(
        (status = NO_CONTENT, description = "Signed in; the response sets the session cookie"),
        (status = UNAUTHORIZED, description = "The username or password is wrong"),
    ),
)]
pub async fn sign_in(
    State(pool): State<PgPool>,
    Json(body): Json<SignIn>,
) -> Result<impl IntoResponse, ApiError> {
    let user = sqlx::query!(
        "SELECT id, password_hash FROM users WHERE username = $1",
        body.username,
    )
    .fetch_optional(&pool)
    .await?;
    let stored = user.as_ref().and_then(|user| user.password_hash.clone());
    let attempted = user.as_ref().map(|user| user.id.to_string());
    let mut tx = pool.begin().await?;
    let (Some(user), Verdict::Right { rehashed }) =
        (user, password::verify(body.password, stored).await?)
    else {
        audit::record(
            &mut tx,
            Actor::Anonymous,
            "session.sign_in_failed",
            ("user", attempted.as_deref()),
            json!({}),
        )
        .await?;
        tx.commit().await?;
        return Err(ApiError::Status(StatusCode::UNAUTHORIZED));
    };

    if let Some(hash) = rehashed {
        sqlx::query!(
            "UPDATE users SET password_hash = $1 WHERE id = $2",
            hash,
            user.id
        )
        .execute(&mut *tx)
        .await?;
    }
    let (token, hash) = new_token();
    sqlx::query!(
        "INSERT INTO sessions (token_hash, user_id, idle_timeout, expires_at) VALUES ($1, $2,
         CASE WHEN $3::boolean THEN interval '7 days' ELSE interval '30 minutes' END,
         now() + CASE WHEN $3::boolean THEN interval '30 days' ELSE interval '12 hours' END)",
        hash,
        user.id,
        body.remember,
    )
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        Actor::User(user.id),
        "session.sign_in",
        ("user", Some(&user.id.to_string())),
        json!({ "remember": body.remember }),
    )
    .await?;
    tx.commit().await?;

    // Max-Age only with "Ingat saya", set to the absolute lifetime; otherwise it dies with the browser.
    let max_age = if body.remember {
        "; Max-Age=2592000"
    } else {
        ""
    };
    Ok((
        StatusCode::NO_CONTENT,
        AppendHeaders([(
            header::SET_COOKIE,
            format!("{COOKIE}={token}; Path=/; Secure; HttpOnly; SameSite=Lax{max_age}"),
        )]),
    ))
}

/// The User of a live session, which each use keeps from going idle.
pub(crate) struct Session {
    token_hash: Vec<u8>,
    user_id: Uuid,
    username: String,
    role: String,
    pub(crate) administrator: bool,
}

fn session_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .find_map(|cookie| cookie.trim().strip_prefix(COOKIE)?.strip_prefix('='))
}

impl FromRequestParts<PgPool> for Session {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, pool: &PgPool) -> Result<Self, ApiError> {
        let token =
            session_token(&parts.headers).ok_or(ApiError::Status(StatusCode::UNAUTHORIZED))?;
        let token_hash = token_hash(token);
        let row = sqlx::query!(
            "WITH live AS (
                 UPDATE sessions SET last_seen_at = now()
                 WHERE token_hash = $1 AND expires_at > now() AND last_seen_at + idle_timeout > now()
                 RETURNING user_id
             )
             SELECT users.id, users.username, roles.name AS role, roles.administrator
             FROM live JOIN users ON users.id = live.user_id JOIN roles ON roles.id = users.role_id",
            token_hash,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(ApiError::Status(StatusCode::UNAUTHORIZED))?;
        Ok(Self {
            token_hash,
            user_id: row.id,
            username: row.username,
            role: row.role,
            administrator: row.administrator,
        })
    }
}

/// Ends the current session and clears its cookie.
///
/// # Errors
/// `401` without a live session.
#[utoipa::path(
    post,
    path = "/auth/sign-out",
    operation_id = "signOut",
    tag = "auth",
    responses(
        (status = NO_CONTENT, description = "Signed out; the response clears the session cookie"),
        (status = UNAUTHORIZED, description = "There is no live session"),
    ),
)]
pub async fn sign_out(
    State(pool): State<PgPool>,
    session: Session,
) -> Result<impl IntoResponse, ApiError> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "DELETE FROM sessions WHERE token_hash = $1",
        session.token_hash
    )
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        Actor::User(session.user_id),
        "session.sign_out",
        ("user", Some(&session.user_id.to_string())),
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok((
        StatusCode::NO_CONTENT,
        AppendHeaders([(
            header::SET_COOKIE,
            format!("{COOKIE}=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0"),
        )]),
    ))
}

#[derive(Serialize, ToSchema)]
pub struct Me {
    username: String,
    /// The name of the User's Role.
    role: String,
}

/// The signed-in User.
///
/// # Errors
/// `401` without a live session.
#[utoipa::path(
    get,
    path = "/auth/me",
    operation_id = "getMe",
    tag = "auth",
    responses(
        (status = OK, description = "The signed-in User", body = Me),
        (status = UNAUTHORIZED, description = "There is no live session"),
    ),
)]
pub async fn me(session: Session) -> Json<Me> {
    Json(Me {
        username: session.username,
        role: session.role,
    })
}

/// Deletes expired sessions and Access Links, which are already refused; run periodically.
///
/// # Errors
/// When the database is unreachable.
pub async fn purge_expired(pool: &PgPool) -> sqlx::Result<()> {
    sqlx::query!(
        "DELETE FROM sessions WHERE expires_at <= now() OR last_seen_at + idle_timeout <= now()"
    )
    .execute(pool)
    .await?;
    sqlx::query!("DELETE FROM access_links WHERE expires_at <= now()")
        .execute(pool)
        .await?;
    Ok(())
}
