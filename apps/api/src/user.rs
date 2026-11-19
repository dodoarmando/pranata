use anyhow::Context;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgConnection, PgPool};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    ApiError,
    audit::{self, Actor},
    auth::{self, Session},
};

/// Lowercase `[a-z0-9._-]`, 3–32 characters.
fn is_valid_username(username: &str) -> bool {
    (3..=32).contains(&username.len())
        && username
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'))
}

/// Inserts a User with a valid username and an existing Role, records it, and returns their id
/// and first Access Link token; `None` when the username is taken.
async fn insert(
    conn: &mut PgConnection,
    actor: Actor,
    username: &str,
    role_id: Uuid,
) -> sqlx::Result<Option<(Uuid, String)>> {
    let Some(user) = sqlx::query!(
        r#"INSERT INTO users (username, role_id) VALUES ($1, $2)
           ON CONFLICT (username) DO NOTHING
           RETURNING id, (SELECT name FROM roles WHERE id = $2) AS "role!""#,
        username,
        role_id,
    )
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(None);
    };
    audit::record(
        conn,
        actor,
        "user.create",
        ("user", Some(&user.id.to_string())),
        json!({ "username": username, "role": user.role }),
    )
    .await?;
    let token = auth::create_access_link(conn, actor, user.id).await?;
    Ok(Some((user.id, token)))
}

/// Creates a User with the Administrator Role and returns the token of their Access Link.
///
/// # Errors
/// When the username is malformed or already taken, or the database write fails.
pub async fn create_administrator(pool: &PgPool, username: &str) -> anyhow::Result<String> {
    anyhow::ensure!(
        is_valid_username(username),
        "a username is 3–32 lowercase letters, digits, dots, underscores or dashes, got {username:?}"
    );
    let mut tx = pool.begin().await?;
    let role = sqlx::query_scalar!("SELECT id FROM roles WHERE administrator")
        .fetch_one(&mut *tx)
        .await?;
    let (_, token) = insert(&mut tx, Actor::System, username, role)
        .await
        .context("cannot save the User")?
        .with_context(|| format!("the username {username:?} is taken"))?;
    tx.commit().await?;
    Ok(token)
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct User {
    id: Uuid,
    username: String,
    role_id: Uuid,
    /// The name of the User's Role.
    role: String,
}

/// Every User, oldest first.
///
/// # Errors
/// `401` without a live session, `403` outside the Administrator Role.
#[utoipa::path(
    get,
    path = "/users",
    operation_id = "listUsers",
    tag = "users",
    responses(
        (status = OK, description = "Every User", body = Vec<User>),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User is not an Administrator"),
    ),
)]
pub(crate) async fn list(
    State(pool): State<PgPool>,
    session: Session,
) -> Result<Json<Vec<User>>, ApiError> {
    session.ensure_administrator()?;
    let users = sqlx::query_as!(
        User,
        "SELECT users.id, username, role_id, roles.name AS role
         FROM users JOIN roles ON roles.id = role_id ORDER BY users.id",
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(users))
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewUser {
    /// 3–32 lowercase letters, digits, dots, underscores or dashes; never changed or reused.
    username: String,
    role_id: Uuid,
}

/// A fresh Access Link, shown once: the web app links to `/akses#<token>`.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccessLink {
    user_id: Uuid,
    /// Single-use and valid for 24 hours.
    token: String,
}

/// Creates a User and their first Access Link.
///
/// # Errors
/// `401` without a live session, `403` outside the Administrator Role, `409` for a taken
/// username, `422` for a malformed username or an unknown Role, `428` outside sudo mode.
#[utoipa::path(
    post,
    path = "/users",
    operation_id = "createUser",
    tag = "users",
    request_body = NewUser,
    responses(
        (status = CREATED, description = "The User and their Access Link", body = AccessLink),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User is not an Administrator"),
        (status = CONFLICT, description = "The username is taken"),
        (status = UNPROCESSABLE_ENTITY, description = "The username is malformed or the Role unknown"),
        (status = PRECONDITION_REQUIRED, description = "The User must re-authenticate first"),
    ),
)]
pub(crate) async fn create(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<NewUser>,
) -> Result<(StatusCode, Json<AccessLink>), ApiError> {
    session.ensure_administrator()?;
    session.ensure_sudo()?;
    if !is_valid_username(&body.username) {
        return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
    }
    let mut tx = pool.begin().await?;
    let created = insert(
        &mut tx,
        Actor::User(session.user_id),
        &body.username,
        body.role_id,
    )
    .await;
    let (user_id, token) = match created {
        Err(sqlx::Error::Database(error)) if error.is_foreign_key_violation() => {
            return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
        }
        created => created?.ok_or(ApiError::Status(StatusCode::CONFLICT))?,
    };
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(AccessLink { user_id, token })))
}

/// Locks the User's row for the transaction, refusing an unknown User with `404`.
async fn lock(conn: &mut PgConnection, id: Uuid) -> Result<(), ApiError> {
    sqlx::query_scalar!("SELECT id FROM users WHERE id = $1 FOR UPDATE", id)
        .fetch_optional(conn)
        .await?
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    Ok(())
}

/// Revokes every session of the User and records it.
async fn end_sessions(conn: &mut PgConnection, actor: Actor, id: Uuid) -> sqlx::Result<()> {
    sqlx::query!("DELETE FROM sessions WHERE user_id = $1", id)
        .execute(&mut *conn)
        .await?;
    audit::record(
        conn,
        actor,
        "user.sessions_revoke",
        ("user", Some(&id.to_string())),
        json!({}),
    )
    .await
}

/// Resets a User's access: a new Access Link that voids the previous one, and every session
/// of theirs revoked.
///
/// # Errors
/// `401` without a live session, `403` outside the Administrator Role, `404` for an unknown
/// User, `428` outside sudo mode.
#[utoipa::path(
    post,
    path = "/users/{id}/access-link",
    operation_id = "createAccessLink",
    tag = "users",
    params(("id" = Uuid, Path, description = "The User")),
    responses(
        (status = CREATED, description = "The new Access Link", body = AccessLink),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User is not an Administrator"),
        (status = NOT_FOUND, description = "There is no such User"),
        (status = PRECONDITION_REQUIRED, description = "The User must re-authenticate first"),
    ),
)]
pub(crate) async fn reset(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<AccessLink>), ApiError> {
    session.ensure_administrator()?;
    session.ensure_sudo()?;
    let mut tx = pool.begin().await?;
    lock(&mut tx, id).await?;
    let actor = Actor::User(session.user_id);
    end_sessions(&mut tx, actor, id).await?;
    let token = auth::create_access_link(&mut tx, actor, id).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(AccessLink { user_id: id, token })))
}

/// Signs a User out everywhere by revoking every session of theirs.
///
/// # Errors
/// `401` without a live session, `403` outside the Administrator Role, `404` for an unknown
/// User, `428` outside sudo mode.
#[utoipa::path(
    delete,
    path = "/users/{id}/sessions",
    operation_id = "revokeSessions",
    tag = "users",
    params(("id" = Uuid, Path, description = "The User")),
    responses(
        (status = NO_CONTENT, description = "Every session of the User is revoked"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User is not an Administrator"),
        (status = NOT_FOUND, description = "There is no such User"),
        (status = PRECONDITION_REQUIRED, description = "The User must re-authenticate first"),
    ),
)]
pub(crate) async fn revoke_sessions(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    session.ensure_administrator()?;
    session.ensure_sudo()?;
    let mut tx = pool.begin().await?;
    lock(&mut tx, id).await?;
    end_sessions(&mut tx, Actor::User(session.user_id), id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
