use sqlx::PgPool;

// The one test below the HTTP seam: append-only is a database guarantee.
#[sqlx::test]
async fn audit_entries_cannot_be_changed_or_removed(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();

    for statement in [
        "UPDATE audit_entries SET action = 'forged'",
        "DELETE FROM audit_entries",
        "TRUNCATE audit_entries",
    ] {
        assert!(
            sqlx::raw_sql(statement).execute(&pool).await.is_err(),
            "{statement} succeeded"
        );
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_entries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}
