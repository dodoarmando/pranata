use axum::{Json, extract::State, http::StatusCode};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::{PgConnection, PgPool};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{ApiError, auth::Session};

/// Who an Audit Entry says acted.
#[derive(Clone, Copy)]
pub enum Actor {
    /// A signed-in User.
    User(Uuid),
    /// The `pranata` CLI, run by the vendor.
    System,
    /// Someone not (yet) identified, such as a failed sign-in.
    Anonymous,
}

/// Appends an Audit Entry; call it in the transaction of the change it records.
///
/// # Errors
/// When the insert fails, which must abort the change.
pub async fn record(
    conn: &mut PgConnection,
    actor: Actor,
    action: &str,
    (object_type, object_id): (&str, Option<&str>),
    details: Value,
) -> sqlx::Result<()> {
    let (kind, actor_id) = match actor {
        Actor::User(id) => ("user", Some(id)),
        Actor::System => ("system", None),
        Actor::Anonymous => ("anonymous", None),
    };
    sqlx::query!(
        "INSERT INTO audit_entries (actor, actor_id, action, object_type, object_id, details)
         VALUES ($1, $2, $3, $4, $5, $6)",
        kind,
        actor_id,
        action,
        object_type,
        object_id,
        details,
    )
    .execute(conn)
    .await?;
    Ok(())
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    id: Uuid,
    at: DateTime<Utc>,
    /// `user`, `system` (the CLI) or `anonymous` (not identified).
    actor: String,
    /// The acting User's username, when the actor is a User.
    username: Option<String>,
    action: String,
    object_type: String,
    object_id: Option<String>,
    details: Value,
}

/// The latest Audit Entries, newest first.
///
/// # Errors
/// `401` without a live session, `403` without `audit.read`.
#[utoipa::path(
    get,
    path = "/audit-entries",
    operation_id = "listAuditEntries",
    tag = "audit",
    responses(
        (status = OK, description = "The latest 100 Audit Entries, newest first", body = Vec<AuditEntry>),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not read the Audit Log"),
    ),
)]
pub async fn list(
    State(pool): State<PgPool>,
    session: Session,
) -> Result<Json<Vec<AuditEntry>>, ApiError> {
    // shortcut: only the Administrator Role exists yet, so it stands for `audit.read`; the Permission catalog replaces this check.
    if !session.administrator {
        return Err(ApiError::Status(StatusCode::FORBIDDEN));
    }
    // shortcut: the latest 100 only; the Audit Log viewer adds filters and paging.
    let entries = sqlx::query_as!(
        AuditEntry,
        "SELECT audit_entries.id, at, actor, users.username AS \"username?\", action, object_type, object_id, details
         FROM audit_entries LEFT JOIN users ON users.id = actor_id
         ORDER BY at DESC, audit_entries.id DESC LIMIT 100",
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(entries))
}
