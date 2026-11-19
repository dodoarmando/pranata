use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgConnection, PgPool, types::Json as Jsonb};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{ApiError, auth::Session, village::TimeZone};

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

/// One field's change in an Audit Entry.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct FieldChange {
    field: String,
    from: Value,
    to: Value,
    /// Whether `from` and `to` are shown as `•••` to readers without `resident.sensitive`.
    #[serde(default)]
    sensitive: bool,
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
    /// Context for the action; never sensitive values, which go in `changes` to be masked.
    details: Value,
    /// The per-field diff of a data change.
    #[schema(value_type = Vec<FieldChange>)]
    changes: Jsonb<Vec<FieldChange>>,
}

const PAGE: usize = 50;

/// Filters on the Audit Log; they all apply together.
#[derive(Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query, rename_all = "camelCase")]
pub struct AuditFilter {
    object_type: Option<String>,
    object_id: Option<String>,
    /// The acting User's username.
    username: Option<String>,
    /// The first day, in the Village's time zone.
    from: Option<NaiveDate>,
    /// The last day, inclusive, in the Village's time zone.
    to: Option<NaiveDate>,
    /// The `next` of the previous page.
    before: Option<Uuid>,
}

#[derive(Serialize, ToSchema)]
pub struct AuditPage {
    /// At most 50 Audit Entries, newest first.
    entries: Vec<AuditEntry>,
    /// Pass as `before` for the next, older page; null on the last page.
    next: Option<Uuid>,
}

/// The Audit Log, newest first, filtered and a page at a time.
///
/// # Errors
/// `401` without a live session, `403` without `audit.read`.
#[utoipa::path(
    get,
    path = "/audit-entries",
    operation_id = "listAuditEntries",
    tag = "audit",
    params(AuditFilter),
    responses(
        (status = OK, description = "A page of matching Audit Entries", body = AuditPage),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not read the Audit Log"),
    ),
)]
pub async fn list(
    State(pool): State<PgPool>,
    session: Session,
    Query(filter): Query<AuditFilter>,
) -> Result<Json<AuditPage>, ApiError> {
    if !session.holds("audit.read") {
        return Err(ApiError::Status(StatusCode::FORBIDDEN));
    }
    // Before the Village Code is set, its days are taken as WIB.
    let code = sqlx::query_scalar!("SELECT code FROM village")
        .fetch_optional(&pool)
        .await?;
    let zone = code.map_or(TimeZone::Wib, |code| TimeZone::of_province(&code));
    let mut entries = sqlx::query_as!(
        AuditEntry,
        "SELECT audit_entries.id, at, actor, users.username AS \"username?\", action, object_type, object_id, details,
                changes AS \"changes: Jsonb<Vec<FieldChange>>\"
         FROM audit_entries LEFT JOIN users ON users.id = actor_id
         WHERE ($1::text IS NULL OR object_type = $1)
           AND ($2::text IS NULL OR object_id = $2)
           AND ($3::text IS NULL OR users.username = $3)
           AND ($4::date IS NULL OR at >= $4::date::timestamp AT TIME ZONE $7)
           AND ($5::date IS NULL OR at < ($5::date + 1)::timestamp AT TIME ZONE $7)
           AND ($6::uuid IS NULL OR (at, audit_entries.id) < (SELECT at, id FROM audit_entries WHERE id = $6))
         ORDER BY at DESC, audit_entries.id DESC LIMIT $8",
        filter.object_type,
        filter.object_id,
        filter.username,
        filter.from,
        filter.to,
        filter.before,
        zone.iana(),
        i64::try_from(PAGE + 1)?,
    )
    .fetch_all(&pool)
    .await?;
    if !session.holds("resident.sensitive") {
        let sensitive = entries
            .iter_mut()
            .flat_map(|entry| entry.changes.iter_mut())
            .filter(|change| change.sensitive);
        for change in sensitive {
            change.from = "•••".into();
            change.to = "•••".into();
        }
    }
    let next = (entries.len() > PAGE).then(|| {
        entries.truncate(PAGE);
        entries[PAGE - 1].id
    });
    Ok(Json(AuditPage { entries, next }))
}
