use sqlx::PgPool;

#[sqlx::test]
async fn migrator_ignores_migrations_from_a_newer_release(pool: PgPool) {
    // A newer release applied a migration this binary does not embed.
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
         VALUES (99999999999999, 'from a newer release', true, '\\x00', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    pranata::db::run_migrations(&pool)
        .await
        .expect("the previous release starts on the newer schema");
}
