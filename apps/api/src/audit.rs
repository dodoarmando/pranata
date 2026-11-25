//! Append-only audit log (D-35). Entries are written in the same transaction
//! as the change they describe; the app role cannot update or delete them.

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Map, Value};
use sqlx::PgConnection;

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

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
