mod common;

use axum::http::StatusCode;
use common::{PASSWORD, TestApp};
use serde_json::json;
use sqlx::PgPool;

/// Makes every session's last authentication `minutes` old.
async fn authenticated_ago(pool: &PgPool, minutes: i32) {
    sqlx::query("UPDATE sessions SET authenticated_at = now() - make_interval(mins => $1)")
        .bind(minutes)
        .execute(pool)
        .await
        .unwrap();
}

async fn create_role(app: &TestApp, cookie: &str, name: &str) -> StatusCode {
    let body = json!({ "name": name, "permissions": [] });
    app.call("POST", "/roles", Some(&body), Some(cookie))
        .await
        .status
}

async fn reauthenticate(app: &TestApp, cookie: &str, password: &str) -> StatusCode {
    let body = json!({ "password": password });
    app.call("POST", "/auth/reauthenticate", Some(&body), Some(cookie))
        .await
        .status
}

#[sqlx::test]
async fn actions_on_roles_need_authentication_within_15_minutes(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;

    authenticated_ago(&pool, 14).await;
    assert_eq!(
        create_role(&app, &cookie, "Kaur").await,
        StatusCode::CREATED
    );

    authenticated_ago(&pool, 16).await;
    assert_eq!(
        create_role(&app, &cookie, "Kasi").await,
        StatusCode::PRECONDITION_REQUIRED
    );
    // Reading needs no re-authentication.
    let roles = app.call("GET", "/roles", None, Some(&cookie)).await;
    assert_eq!(roles.status, StatusCode::OK);
    assert!(!roles.body.to_string().contains("Kasi"));
}

#[sqlx::test]
async fn re_authenticating_with_the_password_restarts_the_15_minutes(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;
    authenticated_ago(&pool, 60).await;

    assert_eq!(
        reauthenticate(&app, &cookie, "wrong password").await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        reauthenticate(&app, &cookie, PASSWORD).await,
        StatusCode::NO_CONTENT
    );

    assert_eq!(
        create_role(&app, &cookie, "Kaur").await,
        StatusCode::CREATED
    );
    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_entries WHERE action LIKE 'session.reauthenticate%' ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        actions,
        ["session.reauthenticate_failed", "session.reauthenticate"]
    );
}

#[sqlx::test]
async fn re_authenticating_needs_a_session(pool: PgPool) {
    let app = TestApp::new(pool);

    let (status, _) = app
        .post("/auth/reauthenticate", &json!({ "password": PASSWORD }))
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// Below the HTTP seam: what the migration gives sessions that predate sudo mode.
#[sqlx::test(migrations = false)]
async fn sessions_from_before_sudo_mode_start_outside_it(pool: PgPool) {
    let migrator = &pranata::db::MIGRATOR;
    migrator.run_to(7, &pool).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO users (username, role_id) SELECT 'admin', id FROM roles WHERE administrator;
         INSERT INTO sessions (token_hash, user_id, idle_timeout, expires_at)
         SELECT '\\x00', id, interval '30 minutes', now() + interval '12 hours' FROM users;",
    )
    .execute(&pool)
    .await
    .unwrap();

    migrator.run(&pool).await.unwrap();

    let sudo: bool =
        sqlx::query_scalar("SELECT authenticated_at > now() - interval '15 minutes' FROM sessions")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!sudo, "a session from before the migration is in sudo mode");
}
