//! Server-side sessions (D-30). The cookie carries a random token; the
//! database keeps only its SHA-256 plus the CSRF token for that session.

use anyhow::Context;
use axum::{
    extract::{FromRequestParts, Request, State},
    http::{HeaderMap, HeaderValue, header, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use sqlx::{PgConnection, PgPool};
use time::{Duration, OffsetDateTime};

use super::{CurrentUser, hash_token, random_token};
use crate::{ApiError, AppState, ids::UserId};

/// Name of the session cookie.
pub const COOKIE_NAME: &str = "pranata_session";
/// Lifetime of a session without "remember me", counted from sign-in.
pub const SHORT_LIFETIME: Duration = Duration::hours(12);
/// Lifetime of a "remember me" session, sliding with activity.
pub const REMEMBER_LIFETIME: Duration = Duration::days(30);
/// A "remember me" session is renewed at most this often.
pub const RENEW_INTERVAL: Duration = Duration::hours(1);

/// Source of "now" for session expiry. Production uses [`SystemClock`]; tests
/// inject a fixed clock.
pub trait SessionClock: Send + Sync {
    fn now(&self) -> OffsetDateTime;
}

/// The real clock.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock;

impl SessionClock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

/// A session found for a request, with the CSRF token that belongs to it.
#[derive(Debug, Clone)]
pub struct AuthSession {
    pub user: CurrentUser,
    pub csrf_token: String,
    pub(crate) token_hash: Vec<u8>,
}

/// A session just created at sign-in.
#[derive(Debug)]
pub struct NewSession {
    pub token: String,
    pub csrf_token: String,
}

/// Create a session for `user`.
///
/// # Errors
/// Fails when the insert or the random source fails.
pub async fn create(
    conn: &mut PgConnection,
    user: UserId,
    remember: bool,
    now: OffsetDateTime,
) -> anyhow::Result<NewSession> {
    let token = random_token()?;
    let csrf_token = random_token()?;
    let lifetime = if remember {
        REMEMBER_LIFETIME
    } else {
        SHORT_LIFETIME
    };
    sqlx::query!(
        "INSERT INTO sessions (token_hash, user_id, csrf_token, created_at, last_seen_at, expires_at, remember)
         VALUES ($1, $2, $3, $4, $4, $5, $6)",
        hash_token(&token),
        user.0,
        csrf_token,
        now,
        now + lifetime,
        remember,
    )
    .execute(conn)
    .await
    .context("inserting session")?;
    Ok(NewSession { token, csrf_token })
}

/// Delete a session by its token hash.
///
/// # Errors
/// Fails when the delete fails.
pub async fn delete(conn: &mut PgConnection, token_hash: &[u8]) -> anyhow::Result<()> {
    sqlx::query!("DELETE FROM sessions WHERE token_hash = $1", token_hash)
        .execute(conn)
        .await
        .context("deleting session")?;
    Ok(())
}

/// Look a session up. Returns `None` for unknown or expired tokens and for
/// deactivated Users. The flag says that a "remember me" session was renewed,
/// so the cookie should be re-issued.
async fn find(
    db: &PgPool,
    token: &str,
    now: OffsetDateTime,
    renew: bool,
) -> anyhow::Result<Option<(AuthSession, bool)>> {
    let token_hash = hash_token(token);
    let Some(row) = sqlx::query!(
        r#"SELECT s.csrf_token, s.remember, s.last_seen_at, s.expires_at,
                  u.id AS "user_id: UserId", u.username, u.display_name
           FROM sessions s JOIN users u ON u.id = s.user_id
           WHERE s.token_hash = $1 AND u.is_active"#,
        token_hash,
    )
    .fetch_optional(db)
    .await
    .context("looking up session")?
    else {
        return Ok(None);
    };

    if row.expires_at <= now {
        delete(&mut *db.acquire().await?, &token_hash).await?;
        return Ok(None);
    }
    let renewed = renew && row.remember && now - row.last_seen_at >= RENEW_INTERVAL;
    if renewed {
        sqlx::query!(
            "UPDATE sessions SET last_seen_at = $2, expires_at = $3 WHERE token_hash = $1",
            token_hash,
            now,
            now + REMEMBER_LIFETIME,
        )
        .execute(db)
        .await
        .context("renewing session")?;
    }
    let session = AuthSession {
        user: CurrentUser {
            id: row.user_id,
            username: row.username,
            display_name: row.display_name,
        },
        csrf_token: row.csrf_token,
        token_hash,
    };
    Ok(Some((session, renewed)))
}

/// Read the session token from the `Cookie` headers.
#[must_use]
pub fn token_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find_map(|(name, value)| (name == COOKIE_NAME).then(|| value.to_owned()))
}

/// The `Set-Cookie` value that carries a session token. A "remember me"
/// session gets a `Max-Age`; without it the browser drops the cookie on exit.
#[must_use]
pub fn cookie_value(token: &str, remember: bool) -> HeaderValue {
    let max_age = if remember {
        format!("; Max-Age={}", REMEMBER_LIFETIME.whole_seconds())
    } else {
        String::new()
    };
    header_value(&format!(
        "{COOKIE_NAME}={token}; HttpOnly; Secure; SameSite=Lax; Path=/{max_age}"
    ))
}

/// The `Set-Cookie` value that removes the session cookie.
#[must_use]
pub fn clear_cookie_value() -> HeaderValue {
    header_value(&format!(
        "{COOKIE_NAME}=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0"
    ))
}

fn header_value(text: &str) -> HeaderValue {
    // Tokens are base64url, so the text is always valid header ASCII.
    HeaderValue::from_str(text).expect("cookie text is visible ASCII")
}

/// Middleware that resolves the session cookie once per request and stores the
/// result in the request extensions (for [`CurrentUser`], the CSRF guard and
/// handlers). It also renews "remember me" sessions and re-issues their cookie.
/// Requests without a valid session pass through untouched; the extractor
/// decides whether that is an error.
pub async fn resolve(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let mut reissue = None;
    if let Some(token) = token_from_headers(request.headers()) {
        match find(&state.db, &token, state.clock.now(), true).await {
            Ok(Some((session, renewed))) => {
                request.extensions_mut().insert(session);
                if renewed {
                    reissue = Some(token);
                }
            }
            Ok(None) => {}
            Err(error) => return ApiError::Internal(error).into_response(),
        }
    }
    let mut response = next.run(request).await;
    if let Some(token) = reissue
        && !response.headers().contains_key(header::SET_COOKIE)
    {
        response
            .headers_mut()
            .insert(header::SET_COOKIE, cookie_value(&token, true));
    }
    response
}

impl FromRequestParts<AppState> for AuthSession {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(session) = parts.extensions.get::<Self>() {
            return Ok(session.clone());
        }
        // Reached without the `resolve` middleware (a router built by hand).
        let token =
            token_from_headers(&parts.headers).ok_or(ApiError::Unauthorized("unauthenticated"))?;
        let (session, _) = find(&state.db, &token, state.clock.now(), false)
            .await?
            .ok_or(ApiError::Unauthorized("unauthenticated"))?;
        parts.extensions.insert(session.clone());
        Ok(session)
    }
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(AuthSession::from_request_parts(parts, state).await?.user)
    }
}
