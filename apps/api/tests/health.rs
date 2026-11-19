mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test]
async fn healthz_reports_ok(pool: PgPool) {
    let app = TestApp::new(pool);

    let (status, body) = app.get("/healthz").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "status": "ok" }));
}

#[sqlx::test(migrations = false)]
async fn schema_check_rejects_an_unmigrated_database(pool: PgPool) {
    assert!(pranata::db::ensure_schema(&pool).await.is_err());

    pranata::db::MIGRATOR.run(&pool).await.unwrap();

    assert!(pranata::db::ensure_schema(&pool).await.is_ok());
}
