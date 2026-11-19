use std::fmt::Write;

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgConnection, PgPool, types::Json as Jsonb};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    ApiError,
    auth::Session,
    export::{self, EncryptedFile, ExportReason},
    role::Permission,
    village::TimeZone,
};

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
    insert(
        conn,
        actor,
        action,
        (object_type, object_id),
        details,
        Value::Array(vec![]),
    )
    .await
}

/// Appends an Audit Entry for a data change, with its per-field diff as
/// `[{ "field", "from", "to" }]`; call it in the transaction of the change.
///
/// # Errors
/// When the insert fails, which must abort the change.
pub async fn record_change(
    conn: &mut PgConnection,
    actor: Actor,
    action: &str,
    object: (&str, Option<&str>),
    changes: Value,
) -> sqlx::Result<()> {
    insert(
        conn,
        actor,
        action,
        object,
        Value::Object(serde_json::Map::new()),
        changes,
    )
    .await
}

async fn insert(
    conn: &mut PgConnection,
    actor: Actor,
    action: &str,
    (object_type, object_id): (&str, Option<&str>),
    details: Value,
    changes: Value,
) -> sqlx::Result<()> {
    let (kind, actor_id) = match actor {
        Actor::User(id) => ("user", Some(id)),
        Actor::System => ("system", None),
        Actor::Anonymous => ("anonymous", None),
    };
    sqlx::query!(
        "INSERT INTO audit_entries (actor, actor_id, action, object_type, object_id, details, changes)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        kind,
        actor_id,
        action,
        object_type,
        object_id,
        details,
        changes,
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
#[derive(Deserialize, Serialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query, rename_all = "camelCase")]
pub struct AuditFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    object_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    object_id: Option<String>,
    /// The acting User's username.
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    /// The first day, in the Village's time zone.
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<NaiveDate>,
    /// The last day, inclusive, in the Village's time zone.
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<NaiveDate>,
    /// Only entries older than this one, such as the `next` of the previous page.
    #[serde(skip_serializing_if = "Option::is_none")]
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
    if !session.holds(Permission::AuditRead) {
        return Err(ApiError::Status(StatusCode::FORBIDDEN));
    }
    let (mut entries, _) = matching(&pool, &session, &filter, Some(PAGE + 1)).await?;
    let next = (entries.len() > PAGE).then(|| {
        entries.truncate(PAGE);
        entries[PAGE - 1].id
    });
    Ok(Json(AuditPage { entries, next }))
}

/// The Audit Entries matching `filter`, newest first and at most `limit`, masked for `session`;
/// and the Village's time zone their days are taken in.
async fn matching(
    pool: &PgPool,
    session: &Session,
    filter: &AuditFilter,
    limit: Option<usize>,
) -> Result<(Vec<AuditEntry>, TimeZone), ApiError> {
    // Before the Village Code is set, its days are taken as WIB.
    let code = sqlx::query_scalar!("SELECT code FROM village")
        .fetch_optional(pool)
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
        limit.map(i64::try_from).transpose()?,
    )
    .fetch_all(pool)
    .await?;
    // shortcut: only the Administrator sees sensitive values until R2 adds `resident.sensitive` to the catalog.
    if !session.administrator {
        let sensitive = entries
            .iter_mut()
            .flat_map(|entry| entry.changes.iter_mut())
            .filter(|change| change.sensitive);
        for change in sensitive {
            change.from = "•••".into();
            change.to = "•••".into();
        }
    }
    Ok((entries, zone))
}

/// Exports every Audit Entry matching the filter as a CSV, masked like the Audit Log; see the
/// export mechanism.
///
/// # Errors
/// `401` without a live session, `403` without `audit.read`, `422` without a reason, `428`
/// outside sudo mode.
#[utoipa::path(
    post,
    path = "/audit-entries/export",
    operation_id = "exportAuditEntries",
    tag = "audit",
    params(AuditFilter),
    request_body = ExportReason,
    responses(
        (status = OK, description = "The matching Audit Entries as `log-audit.csv`, encrypted", body = EncryptedFile),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not read the Audit Log"),
        (status = UNPROCESSABLE_ENTITY, description = "The reason is missing"),
        (status = PRECONDITION_REQUIRED, description = "The User must re-authenticate first"),
    ),
)]
pub async fn export(
    State(pool): State<PgPool>,
    session: Session,
    Query(filter): Query<AuditFilter>,
    Json(reason): Json<ExportReason>,
) -> Result<Json<EncryptedFile>, ApiError> {
    if !session.holds(Permission::AuditRead) {
        return Err(ApiError::Status(StatusCode::FORBIDDEN));
    }
    let details = serde_json::json!({ "filter": filter });
    let csv = async {
        let (entries, zone) = matching(&pool, &session, &filter, None).await?;
        Ok(csv(&entries, zone).into_bytes())
    };
    let file = export::encrypted(
        &pool,
        &session,
        reason,
        ("audit_log", details),
        "log-audit.csv",
        csv,
    )
    .await?;
    Ok(Json(file))
}

/// The entries as the Audit Log page shows them, in UTF-8 with a BOM so spreadsheets read `•••`.
fn csv(entries: &[AuditEntry], zone: TimeZone) -> String {
    let offset = FixedOffset::east_opt(zone.utc_offset_hours() * 3600).expect("within a day");
    let header = [
        &format!("Waktu ({})", zone.abbreviation()),
        "Pelaku",
        "Tindakan",
        "Jenis objek",
        "ID objek",
        "Perubahan",
        "Keterangan",
    ];
    let mut out = String::from("\u{feff}");
    row(&mut out, &header);
    for entry in entries {
        let changes = entry
            .changes
            .iter()
            .map(|change| {
                format!(
                    "{}: {} → {}",
                    change.field,
                    shown(&change.from),
                    shown(&change.to)
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let details = if entry.details == Value::Object(serde_json::Map::new()) {
            String::new()
        } else {
            entry.details.to_string()
        };
        let at = entry.at.with_timezone(&offset).format("%Y-%m-%d %H:%M:%S");
        let actor = match (&entry.username, entry.actor.as_str()) {
            (Some(username), _) => username,
            (None, "system") => "Sistem",
            (None, _) => "Tidak dikenal",
        };
        row(
            &mut out,
            &[
                &at.to_string(),
                actor,
                &entry.action,
                &entry.object_type,
                entry.object_id.as_deref().unwrap_or_default(),
                &changes,
                &details,
            ],
        );
    }
    out
}

fn shown(value: &Value) -> String {
    match value {
        Value::Null => "(kosong)".to_owned(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Appends one CSV row, every cell quoted; a cell that a spreadsheet would run as a formula
/// starts with `'` instead.
fn row(out: &mut String, cells: &[&str]) {
    for (i, cell) in cells.iter().enumerate() {
        let formula = cell.starts_with(['=', '+', '-', '@', '\t', '\r']);
        let separator = if i == 0 { "" } else { "," };
        let quote = if formula { "'" } else { "" };
        let _ = write!(out, "{separator}\"{quote}{}\"", cell.replace('"', "\"\""));
    }
    out.push_str("\r\n");
}
