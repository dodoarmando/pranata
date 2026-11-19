mod common;

use axum::http::StatusCode;
use common::{PASSWORD, TestApp};
use serde_json::{Value, json};
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

async fn entries(app: &TestApp, cookie: &str) -> Vec<Value> {
    let reply = app.call("GET", "/audit-entries", None, Some(cookie)).await;
    assert_eq!(reply.status, StatusCode::OK);
    reply.body.as_array().unwrap().clone()
}

#[sqlx::test]
async fn the_audit_log_needs_a_session(pool: PgPool) {
    let app = TestApp::new(pool);

    let (status, _) = app.get("/audit-entries").await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn provisioning_and_signing_in_and_out_are_audited(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    app.admin(&pool, "admin").await;
    app.call(
        "POST",
        "/auth/sign-in",
        Some(&json!({ "username": "admin", "password": "wrong password", "remember": false })),
        None,
    )
    .await;
    let cookie = app.sign_in("admin", false).await;
    app.call("POST", "/auth/sign-out", None, Some(&cookie))
        .await;
    let cookie = app.sign_in("admin", true).await;

    let log: Vec<_> = entries(&app, &cookie)
        .await
        .into_iter()
        .rev()
        .map(|entry| {
            (
                entry["action"].clone(),
                entry["actor"].clone(),
                entry["username"].clone(),
            )
        })
        .collect();

    let system = (json!("system"), Value::Null);
    let admin = (json!("user"), json!("admin"));
    let expected = [
        ("village.set_code", &system),
        ("user.create", &system),
        ("access_link.create", &system),
        ("access_link.use", &admin),
        ("session.sign_in_failed", &(json!("visitor"), Value::Null)),
        ("session.sign_in", &admin),
        ("session.sign_out", &admin),
        ("session.sign_in", &admin),
    ]
    .map(|(action, (actor, username))| (json!(action), actor.clone(), username.clone()));
    assert_eq!(log, expected);
}

#[sqlx::test]
async fn a_failed_change_leaves_no_audit_entry_and_a_successful_one_exactly_one(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.admin(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;
    let token = pranata::user::create_admin(&pool, "kedua").await.unwrap();
    let uses = || async {
        entries(&app, &cookie)
            .await
            .into_iter()
            .filter(|entry| entry["action"] == "access_link.use" && entry["username"] == "kedua")
            .count()
    };

    let (refused, _) = app
        .post(
            "/auth/access-link",
            &json!({ "token": token, "password": "short" }),
        )
        .await;
    assert_eq!(refused, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(uses().await, 0);

    let (accepted, _) = app
        .post(
            "/auth/access-link",
            &json!({ "token": token, "password": PASSWORD }),
        )
        .await;
    assert_eq!(accepted, StatusCode::NO_CONTENT);
    assert_eq!(uses().await, 1);
}
