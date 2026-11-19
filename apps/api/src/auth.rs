use std::fmt::Write;

use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    ApiError,
    audit::{self, Actor},
    password::{self, PasswordProblem},
};

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
