use pranata::audit::{AuditAction, FieldChange, NewAuditEntry, diff, record};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};

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

    let row = sqlx::query("SELECT action, object_type, object_id, summary, changes, actor FROM audit_entries WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("action"), "update");
    assert_eq!(row.get::<String, _>("object_type"), "page");
    assert_eq!(row.get::<String, _>("object_id"), "p1");
    assert_eq!(row.get::<String, _>("summary"), "Edited page");
    assert_eq!(
        row.get::<Value, _>("changes"),
        json!([{"field": "title", "before": "a", "after": "b"}])
    );
    assert!(row.get::<Option<uuid::Uuid>, _>("actor").is_none());
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
