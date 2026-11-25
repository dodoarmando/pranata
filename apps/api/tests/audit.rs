mod common;

use pranata::audit::{AuditAction, FieldChange, NewAuditEntry, diff, record};
use pranata::ids::UserId;
use serde_json::{Value, json};
use sqlx::PgPool;

fn entry() -> NewAuditEntry<'static> {
    NewAuditEntry {
        actor: None,
        action: AuditAction::Update,
        object_type: "page",
        object_id: "p1".into(),
        summary: "Edited page".into(),
        changes: vec![FieldChange {
            field: "title".into(),
            before: json!("a"),
            after: json!("b"),
        }],
    }
}

#[sqlx::test]
async fn record_inserts_entry(pool: PgPool) {
    let mut tx = pool.begin().await.unwrap();
    let id = record(&mut tx, entry()).await.unwrap();
    tx.commit().await.unwrap();

    let row = sqlx::query!(
        r#"SELECT action, object_type, object_id, summary, changes, actor FROM audit_entries WHERE id = $1"#,
        id.0
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.action, "update");
    assert_eq!(row.object_type, "page");
    assert_eq!(row.object_id, "p1");
    assert_eq!(row.summary, "Edited page");
    assert_eq!(
        row.changes,
        json!([{"field": "title", "before": "a", "after": "b"}])
    );
    assert!(row.actor.is_none());
}

#[sqlx::test]
async fn record_with_actor_stores_it(pool: PgPool) {
    // `audit_entries.actor` is a foreign key to `users`, so the actor must exist.
    let role = common::administrator_role(&pool).await;
    let actor = sqlx::query_scalar!(
        r#"INSERT INTO users (username, display_name, role_id) VALUES ('petugas', 'Petugas', $1)
           RETURNING id AS "id: UserId""#,
        role.0
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let id = record(
        &mut tx,
        NewAuditEntry {
            actor: Some(actor),
            ..entry()
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let stored = sqlx::query_scalar!("SELECT actor FROM audit_entries WHERE id = $1", id.0)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, Some(actor.0));
}

#[sqlx::test]
async fn rollback_discards_audit_entry(pool: PgPool) {
    let mut tx = pool.begin().await.unwrap();
    record(&mut tx, entry()).await.unwrap();
    tx.rollback().await.unwrap();

    let count = sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM audit_entries"#)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn action_names_are_pinned_and_match_serde() {
    let expected = [
        (AuditAction::Create, "create"),
        (AuditAction::Update, "update"),
        (AuditAction::Publish, "publish"),
        (AuditAction::Unpublish, "unpublish"),
        (AuditAction::Submit, "submit"),
        (AuditAction::Deactivate, "deactivate"),
        (AuditAction::Delete, "delete"),
        (AuditAction::SignIn, "sign_in"),
        (AuditAction::SignOut, "sign_out"),
    ];
    for (action, name) in expected {
        assert_eq!(action.as_str(), name);
        assert_eq!(serde_json::to_value(action).unwrap(), json!(name));
    }
}

#[test]
fn diff_lists_only_changed_fields() {
    let changes = diff(&json!({"a": 1, "b": 2}), &json!({"a": 1, "b": 3}));
    assert_eq!(
        changes,
        vec![FieldChange {
            field: "b".into(),
            before: json!(2),
            after: json!(3)
        }]
    );
}

#[test]
fn diff_is_sorted_and_treats_missing_as_null() {
    let changes = diff(&json!({"z": 1, "m": 1}), &json!({"z": 2, "a": 5}));
    let fields: Vec<&str> = changes.iter().map(|c| c.field.as_str()).collect();
    assert_eq!(fields, ["a", "m", "z"]);
    assert_eq!(changes[0].before, Value::Null);
    assert_eq!(changes[1].after, Value::Null);
}

// Runtime SQL on purpose: the statements are caller-supplied so one helper can
// try INSERT/UPDATE/DELETE/TRUNCATE under `SET LOCAL ROLE`; the compile-time
// macros would run them as the connecting superuser at build time instead.
/// Run one statement as the app role inside a transaction that is rolled back.
async fn as_app(pool: &PgPool, sql: &'static str) -> Result<u64, sqlx::Error> {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE pranata_app")
        .execute(&mut *tx)
        .await
        .unwrap();
    let result = sqlx::query(sql).execute(&mut *tx).await;
    tx.rollback().await.unwrap();
    result.map(|r| r.rows_affected())
}

fn sqlstate(err: &sqlx::Error) -> Option<String> {
    err.as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .map(std::borrow::Cow::into_owned)
}

const INSERT: &str = "INSERT INTO audit_entries (action, object_type, object_id, summary) VALUES ('create', 'page', 'p1', 's')";

#[sqlx::test]
async fn app_role_can_insert_and_select_audit(pool: PgPool) {
    // audit_entries is created by a migration after 0001, so a successful
    // insert also proves the default-privilege grants reach later tables.
    assert_eq!(as_app(&pool, INSERT).await.unwrap(), 1);
    as_app(&pool, "SELECT * FROM audit_entries").await.unwrap();
}

#[sqlx::test]
async fn app_role_cannot_update_audit(pool: PgPool) {
    sqlx::query(INSERT).execute(&pool).await.unwrap();
    let err = as_app(&pool, "UPDATE audit_entries SET summary = 'x'")
        .await
        .unwrap_err();
    assert_eq!(sqlstate(&err).as_deref(), Some("42501"));
}

#[sqlx::test]
async fn app_role_cannot_delete_audit(pool: PgPool) {
    sqlx::query(INSERT).execute(&pool).await.unwrap();
    let err = as_app(&pool, "DELETE FROM audit_entries")
        .await
        .unwrap_err();
    assert_eq!(sqlstate(&err).as_deref(), Some("42501"));
}

#[sqlx::test]
async fn app_role_cannot_truncate_audit(pool: PgPool) {
    let err = as_app(&pool, "TRUNCATE audit_entries").await.unwrap_err();
    assert_eq!(sqlstate(&err).as_deref(), Some("42501"));
}
