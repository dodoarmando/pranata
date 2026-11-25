//! Append-only audit log (D-35). Entries are written in the same transaction
//! as the change they describe; the app role cannot update or delete them.

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{PgConnection, PgPool};
use time::{Date, Duration, OffsetDateTime, Time, UtcOffset};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::ids::{AuditEntryId, UserId};

/// What happened to the audited object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    Create,
    Update,
    Publish,
    Unpublish,
    Submit,
    Deactivate,
    Reactivate,
    IssueAccessLink,
    Delete,
    SignIn,
    SignOut,
    Reauthenticate,
}

impl AuditAction {
    /// The snake case text stored in the `action` column.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Publish => "publish",
            Self::Unpublish => "unpublish",
            Self::Submit => "submit",
            Self::Deactivate => "deactivate",
            Self::Reactivate => "reactivate",
            Self::IssueAccessLink => "issue_access_link",
            Self::Delete => "delete",
            Self::SignIn => "sign_in",
            Self::SignOut => "sign_out",
            Self::Reauthenticate => "reauthenticate",
        }
    }
}

/// One changed top-level field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct FieldChange {
    pub field: String,
    pub before: Value,
    pub after: Value,
}

/// An entry to append to the audit log.
#[derive(Debug, Clone)]
pub struct NewAuditEntry<'a> {
    pub actor: Option<UserId>,
    pub action: AuditAction,
    pub object_type: &'a str,
    pub object_id: String,
    pub summary: String,
    pub changes: Vec<FieldChange>,
}

/// Append an entry. Call it with the transaction that carries the change.
///
/// # Errors
/// Fails when the insert fails.
pub async fn record(conn: &mut PgConnection, entry: NewAuditEntry<'_>) -> Result<AuditEntryId> {
    let changes = serde_json::to_value(&entry.changes).context("serializing audit changes")?;
    sqlx::query_scalar!(
        r#"INSERT INTO audit_entries (actor, action, object_type, object_id, summary, changes)
           VALUES ($1, $2, $3, $4, $5, $6) RETURNING id AS "id: AuditEntryId""#,
        entry.actor.map(|user| user.0),
        entry.action.as_str(),
        entry.object_type,
        entry.object_id,
        entry.summary,
        changes,
    )
    .fetch_one(conn)
    .await
    .context("inserting audit entry")
}

fn fields_of<T: Serialize>(value: &T) -> Map<String, Value> {
    match serde_json::to_value(value) {
        Ok(Value::Object(map)) => map,
        Ok(other) => Map::from_iter([("value".to_owned(), other)]),
        Err(_) => Map::new(),
    }
}

/// Compare the top-level fields of two serializable objects and return the
/// ones that differ, sorted by field name. A field missing on one side counts
/// as `null` there. A value that is not an object counts as one field named
/// `value`.
#[must_use]
pub fn diff<T: Serialize>(before: &T, after: &T) -> Vec<FieldChange> {
    let (before, after) = (fields_of(before), fields_of(after));
    let names: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    names
        .into_iter()
        .filter_map(|name| {
            let b = before.get(name).cloned().unwrap_or(Value::Null);
            let a = after.get(name).cloned().unwrap_or(Value::Null);
            (a != b).then(|| FieldChange {
                field: name.clone(),
                before: b,
                after: a,
            })
        })
        .collect()
}

/// Where a page of the log ends: the `(at, id)` of its last entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditCursor {
    pub at: OffsetDateTime,
    pub id: AuditEntryId,
}

impl AuditCursor {
    /// The opaque text handed to clients: base64url of the instant in
    /// nanoseconds since the epoch and the entry id.
    #[must_use]
    pub fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(format!("{}.{}", self.at.unix_timestamp_nanos(), self.id.0))
    }

    /// Read text written by [`AuditCursor::encode`]; `None` for anything else.
    #[must_use]
    pub fn decode(text: &str) -> Option<Self> {
        let bytes = URL_SAFE_NO_PAD.decode(text).ok()?;
        let text = String::from_utf8(bytes).ok()?;
        let (nanos, id) = text.split_once('.')?;
        // Only instants the log can hold: `timestamptz` reaches back much
        // further than `OffsetDateTime` but a client has no reason to ask for
        // anything before the epoch, and the database would reject some.
        let nanos: i128 = nanos.parse().ok()?;
        if nanos < 0 {
            return None;
        }
        Some(Self {
            at: OffsetDateTime::from_unix_timestamp_nanos(nanos).ok()?,
            id: AuditEntryId(Uuid::parse_str(id).ok()?),
        })
    }
}

/// What to list. `from` and `to` are calendar dates in the Village's time
/// zone and `to` is inclusive.
#[derive(Debug, Clone)]
pub struct AuditFilter {
    pub actor: Option<UserId>,
    pub action: Option<String>,
    pub object_type: Option<String>,
    pub from: Option<Date>,
    pub to: Option<Date>,
    pub before: Option<AuditCursor>,
    pub limit: i64,
}

/// The User who made a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ActorRef {
    #[schema(value_type = uuid::Uuid)]
    pub id: UserId,
    pub display_name: String,
    pub username: String,
}

/// One entry of the log as the admin screen and the CSV show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AuditRow {
    #[schema(value_type = uuid::Uuid)]
    pub id: AuditEntryId,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    pub at: OffsetDateTime,
    /// `null` for entries the system wrote itself.
    pub actor: Option<ActorRef>,
    pub action: String,
    pub object_type: String,
    pub object_id: String,
    pub summary: String,
    pub changes: Vec<FieldChange>,
}

struct RawRow {
    id: AuditEntryId,
    at: OffsetDateTime,
    action: String,
    object_type: String,
    object_id: String,
    summary: String,
    changes: Value,
    actor_id: Option<Uuid>,
    actor_display_name: Option<String>,
    actor_username: Option<String>,
}

impl RawRow {
    fn into_row(self) -> Result<AuditRow> {
        let actor = match (self.actor_id, self.actor_display_name, self.actor_username) {
            (Some(id), Some(display_name), Some(username)) => Some(ActorRef {
                id: UserId(id),
                display_name,
                username,
            }),
            _ => None,
        };
        Ok(AuditRow {
            id: self.id,
            at: self.at,
            actor,
            action: self.action,
            object_type: self.object_type,
            object_id: self.object_id,
            summary: self.summary,
            changes: serde_json::from_value(self.changes).context("reading audit changes")?,
        })
    }
}

fn start_of_day(date: Date, offset: UtcOffset) -> OffsetDateTime {
    date.with_time(Time::MIDNIGHT).assume_offset(offset)
}

/// Entries newest first, at most `filter.limit`, after `filter.before`.
///
/// # Errors
/// Fails when the query fails or a stored `changes` value is malformed.
pub async fn list(db: &PgPool, filter: &AuditFilter, offset: UtcOffset) -> Result<Vec<AuditRow>> {
    let from = filter.from.map(|date| start_of_day(date, offset));
    // `to` is inclusive, so the range ends where the next day begins.
    let until = filter
        .to
        .and_then(|date| date.checked_add(Duration::days(1)))
        .map(|date| start_of_day(date, offset));
    let rows = sqlx::query_as!(
        RawRow,
        r#"SELECT e.id AS "id: AuditEntryId", e.at, e.action, e.object_type, e.object_id,
                  e.summary, e.changes,
                  u.id AS "actor_id?", u.display_name AS "actor_display_name?",
                  u.username AS "actor_username?"
           FROM audit_entries e
           LEFT JOIN users u ON u.id = e.actor
           WHERE ($1::uuid IS NULL OR e.actor = $1)
             AND ($2::text IS NULL OR e.action = $2)
             AND ($3::text IS NULL OR e.object_type = $3)
             AND ($4::timestamptz IS NULL OR e.at >= $4)
             AND ($5::timestamptz IS NULL OR e.at < $5)
             AND ($6::timestamptz IS NULL OR (e.at, e.id) < ($6, $7::uuid))
           ORDER BY e.at DESC, e.id DESC
           LIMIT $8"#,
        filter.actor.map(|user| user.0),
        filter.action,
        filter.object_type,
        from,
        until,
        filter.before.map(|cursor| cursor.at),
        filter.before.map(|cursor| cursor.id.0),
        filter.limit,
    )
    .fetch_all(db)
    .await
    .context("listing audit entries")?;
    rows.into_iter().map(RawRow::into_row).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn cursor_round_trips_and_rejects_noise() {
        let cursor = AuditCursor {
            at: OffsetDateTime::from_unix_timestamp_nanos(1_795_593_600_123_456_000).unwrap(),
            id: AuditEntryId(Uuid::nil()),
        };
        assert_eq!(AuditCursor::decode(&cursor.encode()), Some(cursor));
        assert_eq!(AuditCursor::decode("not a cursor"), None);
        assert_eq!(AuditCursor::decode(&URL_SAFE_NO_PAD.encode("1.x")), None);
        let id = Uuid::nil();
        // Well-formed, but before 1970 and beyond year 9999.
        for nanos in ["-1", "-300000000000000000000", "300000000000000000000000"] {
            let text = URL_SAFE_NO_PAD.encode(format!("{nanos}.{id}"));
            assert_eq!(AuditCursor::decode(&text), None, "{nanos}");
        }
        let epoch = URL_SAFE_NO_PAD.encode(format!("0.{id}"));
        assert!(AuditCursor::decode(&epoch).is_some());
    }

    #[test]
    fn diff_of_non_objects_uses_a_value_field() {
        assert_eq!(
            diff(&"a", &"b"),
            vec![FieldChange {
                field: "value".into(),
                before: json!("a"),
                after: json!("b"),
            }]
        );
        assert_eq!(diff(&1, &1), Vec::new());
    }
}
