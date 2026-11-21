use std::{fmt::Write, net::IpAddr, sync::Arc};

use axum::{
    Json,
    extract::{FromRef, FromRequestParts, State},
    http::{HeaderMap, StatusCode, header, request::Parts},
    response::{AppendHeaders, IntoResponse},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    ApiError,
    audit::{self, Actor},
    complaint,
    password::{self, PasswordProblem, Verdict},
    role::Permission,
    throttle::Throttle,
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

/// Sets the User's password from their Access Link, which is then used up, and revokes every
/// session of theirs.
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
        (status = NO_CONTENT, description = "The password is set and every session of the User revoked"),
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
    // A reset: whoever holds a session with the old password is signed out.
    let revoked = sqlx::query!("DELETE FROM sessions WHERE user_id = $1", user.id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    audit::record(
        &mut tx,
        Actor::User(user.id),
        "access_link.use",
        ("user", Some(&user.id.to_string())),
        json!({ "sessionsRevoked": revoked }),
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
/// `401`, the same for every wrong username or password; `429` without checking them, after 5
/// failures for the username or 20 attempts a minute from the address.
#[utoipa::path(
    post,
    path = "/auth/sign-in",
    operation_id = "signIn",
    tag = "auth",
    request_body = SignIn,
    responses(
        (status = NO_CONTENT, description = "Signed in; the response sets the session cookie"),
        (status = UNAUTHORIZED, description = "The username or password is wrong"),
        (status = TOO_MANY_REQUESTS, description = "Too many attempts for the username or from the address; try again later"),
    ),
)]
pub async fn sign_in(
    State(pool): State<PgPool>,
    State(throttle): State<Arc<Throttle>>,
    headers: HeaderMap,
    Json(body): Json<SignIn>,
) -> Result<impl IntoResponse, ApiError> {
    if !throttle.admit_password(client_ip(&headers), &body.username) {
        return Err(ApiError::Status(StatusCode::TOO_MANY_REQUESTS));
    }
    let user = sqlx::query!(
        "SELECT id, password_hash, active FROM users WHERE username = $1",
        body.username,
    )
    .fetch_optional(&pool)
    .await?;
    // A deactivated User is refused like a wrong password.
    let stored = user
        .as_ref()
        .filter(|user| user.active)
        .and_then(|user| user.password_hash.clone());
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
    let cookie = start_session(
        &mut tx,
        user.id,
        body.remember,
        &headers,
        json!({ "remember": body.remember }),
    )
    .await?;
    tx.commit().await?;
    throttle.succeeded(&body.username);
    Ok((
        StatusCode::NO_CONTENT,
        AppendHeaders([(header::SET_COOKIE, cookie)]),
    ))
}

/// Starts a session for a User who just signed in and records it with `details`, returning
/// the `Set-Cookie` value that carries it.
pub(crate) async fn start_session(
    conn: &mut PgConnection,
    user_id: Uuid,
    remember: bool,
    headers: &HeaderMap,
    details: serde_json::Value,
) -> sqlx::Result<String> {
    let (token, hash) = new_token();
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.chars().take(512).collect::<String>());
    let ip = client_ip(headers).map(|ip| ip.to_string());
    sqlx::query!(
        "INSERT INTO sessions (token_hash, user_id, idle_timeout, expires_at, user_agent, ip) VALUES ($1, $2,
         CASE WHEN $3::boolean THEN interval '7 days' ELSE interval '30 minutes' END,
         now() + CASE WHEN $3::boolean THEN interval '30 days' ELSE interval '12 hours' END, $4, $5)",
        hash,
        user_id,
        remember,
        user_agent,
        ip,
    )
    .execute(&mut *conn)
    .await?;
    audit::record(
        conn,
        Actor::User(user_id),
        "session.sign_in",
        ("user", Some(&user_id.to_string())),
        details,
    )
    .await?;
    // Max-Age only with "Ingat saya", set to the absolute lifetime; otherwise it dies with the browser.
    let max_age = if remember { "; Max-Age=2592000" } else { "" };
    Ok(format!(
        "{COOKIE}={token}; Path=/; Secure; HttpOnly; SameSite=Lax{max_age}"
    ))
}

/// The client's address as the web app forwards it, which it takes from Caddy alone.
pub(crate) fn client_ip(headers: &HeaderMap) -> Option<IpAddr> {
    // The last entry is the one the nearest proxy, the web app, vouches for.
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.rsplit(',').next())
        .and_then(|value| value.trim().parse().ok())
}

/// The User of a live session, which each use keeps from going idle.
pub(crate) struct Session {
    token_hash: Vec<u8>,
    pub(crate) user_id: Uuid,
    pub(crate) username: String,
    role: String,
    pub(crate) administrator: bool,
    permissions: Vec<Permission>,
    /// Sudo mode: the User gave their password within the last 15 minutes.
    sudo: bool,
}

impl Session {
    /// Whether the User's Role holds `permission`; the Administrator holds every one.
    pub(crate) fn holds(&self, permission: Permission) -> bool {
        self.administrator || self.permissions.contains(&permission)
    }

    /// Refuses with `403` outside the Administrator Role, which alone manages Users and Roles.
    pub(crate) fn ensure_administrator(&self) -> Result<(), ApiError> {
        if self.administrator {
            Ok(())
        } else {
            Err(ApiError::Status(StatusCode::FORBIDDEN))
        }
    }

    /// Refuses with `428` outside sudo mode, so the User re-authenticates first.
    pub(crate) fn ensure_sudo(&self) -> Result<(), ApiError> {
        if self.sudo {
            Ok(())
        } else {
            Err(ApiError::Status(StatusCode::PRECONDITION_REQUIRED))
        }
    }
}

fn session_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .find_map(|cookie| cookie.trim().strip_prefix(COOKIE)?.strip_prefix('='))
}

impl<S: Send + Sync> FromRequestParts<S> for Session
where
    PgPool: FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        let pool = &PgPool::from_ref(state);
        let token =
            session_token(&parts.headers).ok_or(ApiError::Status(StatusCode::UNAUTHORIZED))?;
        let token_hash = token_hash(token);
        let row = sqlx::query!(
            "WITH live AS (
                 UPDATE sessions SET last_seen_at = now()
                 WHERE token_hash = $1 AND expires_at > now() AND last_seen_at + idle_timeout > now()
                 RETURNING user_id, authenticated_at > now() - interval '15 minutes' AS sudo
             )
             SELECT users.id, users.username, roles.name AS role, roles.administrator, live.sudo AS \"sudo!\",
                    roles.permissions AS \"permissions: Vec<Permission>\"
             FROM live JOIN users ON users.id = live.user_id AND users.active JOIN roles ON roles.id = users.role_id",
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
            permissions: row.permissions,
            sudo: row.sudo,
        })
    }
}

#[derive(Deserialize, ToSchema)]
pub struct Reauthenticate {
    password: String,
}

/// Confirms the signed-in User's password, entering sudo mode for 15 minutes.
///
/// # Errors
/// `401` without a live session or for a wrong password.
#[utoipa::path(
    post,
    path = "/auth/reauthenticate",
    operation_id = "reauthenticate",
    tag = "auth",
    request_body = Reauthenticate,
    responses(
        (status = NO_CONTENT, description = "The password is right; sudo mode starts"),
        (status = UNAUTHORIZED, description = "There is no live session, or the password is wrong"),
    ),
)]
pub async fn reauthenticate(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<Reauthenticate>,
) -> Result<StatusCode, ApiError> {
    let stored = sqlx::query_scalar!(
        "SELECT password_hash FROM users WHERE id = $1",
        session.user_id
    )
    .fetch_one(&pool)
    .await?;
    let actor = Actor::User(session.user_id);
    let user_id = session.user_id.to_string();
    let mut tx = pool.begin().await?;
    let Verdict::Right { rehashed } = password::verify(body.password, stored).await? else {
        audit::record(
            &mut tx,
            actor,
            "session.reauthenticate_failed",
            ("user", Some(&user_id)),
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
            session.user_id
        )
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query!(
        "UPDATE sessions SET authenticated_at = now() WHERE token_hash = $1",
        session.token_hash
    )
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        actor,
        "session.reauthenticate",
        ("user", Some(&user_id)),
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
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
#[serde(rename_all = "camelCase")]
pub struct Me {
    username: String,
    /// The name of the User's Role.
    role: String,
    /// How many Complaints are New, for the menu badge; null without `complaint.read`.
    new_complaints: Option<i64>,
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
pub async fn me(State(pool): State<PgPool>, session: Session) -> Result<Json<Me>, ApiError> {
    let new_complaints = complaint::count_new(&pool, &session).await?;
    Ok(Json(Me {
        username: session.username,
        role: session.role,
        new_complaints,
    }))
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSession {
    /// The browser's user agent at sign-in.
    user_agent: Option<String>,
    /// The address signed in from.
    ip: Option<String>,
    last_seen_at: DateTime<Utc>,
    /// Whether this is the session making the request.
    current: bool,
}

/// The signed-in User's live sessions, most recently active first.
///
/// # Errors
/// `401` without a live session.
#[utoipa::path(
    get,
    path = "/auth/sessions",
    operation_id = "listSessions",
    tag = "auth",
    responses(
        (status = OK, description = "The User's live sessions", body = Vec<ActiveSession>),
        (status = UNAUTHORIZED, description = "There is no live session"),
    ),
)]
pub async fn sessions(
    State(pool): State<PgPool>,
    session: Session,
) -> Result<Json<Vec<ActiveSession>>, ApiError> {
    let sessions = sqlx::query_as!(
        ActiveSession,
        r#"SELECT user_agent, ip, last_seen_at, token_hash = $2 AS "current!" FROM sessions
           WHERE user_id = $1 AND expires_at > now() AND last_seen_at + idle_timeout > now()
           ORDER BY last_seen_at DESC"#,
        session.user_id,
        session.token_hash,
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(sessions))
}

/// "Keluar dari perangkat lain": ends every session of the User but the current one.
///
/// # Errors
/// `401` without a live session.
#[utoipa::path(
    post,
    path = "/auth/sign-out-others",
    operation_id = "signOutOthers",
    tag = "auth",
    responses(
        (status = NO_CONTENT, description = "Every other session of the User is ended"),
        (status = UNAUTHORIZED, description = "There is no live session"),
    ),
)]
pub async fn sign_out_others(
    State(pool): State<PgPool>,
    session: Session,
) -> Result<StatusCode, ApiError> {
    let mut tx = pool.begin().await?;
    let revoked = sqlx::query!(
        "DELETE FROM sessions WHERE user_id = $1 AND token_hash <> $2",
        session.user_id,
        session.token_hash,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    audit::record(
        &mut tx,
        Actor::User(session.user_id),
        "session.sign_out_others",
        ("user", Some(&session.user_id.to_string())),
        json!({ "sessionsRevoked": revoked }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub struct NewPassword {
    password: String,
}

/// Changes the signed-in User's password, under the same policy as at set-up, and ends every
/// other session of theirs.
///
/// # Errors
/// `401` without a live session, `422` for a refused password, `428` outside sudo mode.
#[utoipa::path(
    put,
    path = "/auth/password",
    operation_id = "changePassword",
    tag = "auth",
    request_body = NewPassword,
    responses(
        (status = NO_CONTENT, description = "The password is changed and every other session ended"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = UNPROCESSABLE_ENTITY, description = "The password is refused", body = PasswordRejected),
        (status = PRECONDITION_REQUIRED, description = "The User must re-authenticate first"),
    ),
)]
pub async fn change_password(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<NewPassword>,
) -> Result<StatusCode, ApiError> {
    session.ensure_sudo()?;
    let village = sqlx::query_scalar!("SELECT name FROM village")
        .fetch_optional(&pool)
        .await?;
    password::check(&body.password, &session.username, village.as_deref())
        .map_err(ApiError::PasswordRejected)?;
    let password_hash = password::hash(body.password).await?;
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "UPDATE users SET password_hash = $1 WHERE id = $2",
        password_hash,
        session.user_id,
    )
    .execute(&mut *tx)
    .await?;
    // Whoever holds a session with the old password is signed out; this one stays.
    let revoked = sqlx::query!(
        "DELETE FROM sessions WHERE user_id = $1 AND token_hash <> $2",
        session.user_id,
        session.token_hash,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    audit::record(
        &mut tx,
        Actor::User(session.user_id),
        "user.password_change",
        ("user", Some(&session.user_id.to_string())),
        json!({ "sessionsRevoked": revoked }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
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
