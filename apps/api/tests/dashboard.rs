mod common;

use axum::http::StatusCode;
use common::TestApp;
use sqlx::PgPool;

#[sqlx::test]
async fn the_dashboard_counts_active_users_for_any_user(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    app.administrator(&pool, "leaver").await;
    let cookie = app
        .user_with(&pool, "kontributor", &["content.write"])
        .await;
    sqlx::query("UPDATE users SET active = false WHERE username = 'leaver'")
        .execute(&pool)
        .await
        .unwrap();

    let reply = app.call("GET", "/dashboard", None, Some(&cookie)).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["activeUsers"], 2);
}

#[sqlx::test]
async fn the_dashboard_needs_a_session(pool: PgPool) {
    let app = TestApp::new(pool);

    let (status, _) = app.get("/dashboard").await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
