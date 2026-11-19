mod common;

use axum::http::StatusCode;
use common::{PASSWORD, Reply, TestApp, authenticator::Authenticator};
use serde_json::{Value, json};
use sqlx::PgPool;

const WRONG: &str = "bukan kata sandinya";

async fn sign_in_from(app: &TestApp, ip: &str, username: &str, password: &str) -> Reply {
    app.call_with(
        "POST",
        "/auth/sign-in",
        Some(&json!({ "username": username, "password": password, "remember": false })),
        None,
        &[("x-forwarded-for", ip)],
    )
    .await
}

async fn failures(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit_entries WHERE action = 'session.sign_in_failed'")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The answer to a refused sign-in: no body and no cookie, whatever refused it.
fn assert_refused(reply: &Reply, status: StatusCode) {
    assert_eq!(reply.status, status);
    assert_eq!(reply.body, Value::Null);
    assert!(!reply.headers.contains_key("set-cookie"));
}

#[sqlx::test]
async fn five_failures_for_a_username_delay_its_next_attempt(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    app.administrator(&pool, "kedua").await;

    // Each from its own address, so only the username limit applies.
    for (i, username) in ["admin", "nobody"].iter().enumerate() {
        for attempt in 0..5 {
            let ip = format!("203.0.113.{}", i * 10 + attempt);
            let reply = sign_in_from(&app, &ip, username, WRONG).await;
            assert_refused(&reply, StatusCode::UNAUTHORIZED);
        }
        // Even the right password waits, and an unknown username is delayed just the same.
        let reply = sign_in_from(&app, "198.51.100.1", username, PASSWORD).await;
        assert_refused(&reply, StatusCode::TOO_MANY_REQUESTS);
    }
    assert_eq!(failures(&pool).await, 10, "every failure is audited");

    // Another User is not delayed, and the delayed one is never locked: a passkey still works.
    let cookie = app.sign_in("kedua", false).await;
    let mut passkey = Authenticator::new(1);
    let added = app.add_passkey(&cookie, "Laptop", &passkey).await;
    assert_eq!(added.status, StatusCode::CREATED, "{:?}", added.body);
    let (_, challenge) = app
        .post(
            "/auth/passkey-sign-in/challenge",
            &json!({ "username": "kedua" }),
        )
        .await;
    for _ in 0..5 {
        sign_in_from(&app, "192.0.2.1", "kedua", WRONG).await;
    }
    let body = json!({
        "ceremonyId": challenge["ceremonyId"],
        "credential": passkey.sign_in(&challenge["options"]),
        "remember": false,
    });
    let reply = app
        .call("POST", "/auth/passkey-sign-in", Some(&body), None)
        .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT, "{:?}", reply.body);
}

#[sqlx::test]
async fn signing_in_clears_the_failures(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;

    for _ in 0..2 {
        for _ in 0..4 {
            let reply = sign_in_from(&app, "203.0.113.7", "admin", WRONG).await;
            assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
        }
        let reply = sign_in_from(&app, "203.0.113.7", "admin", PASSWORD).await;
        assert_eq!(reply.status, StatusCode::NO_CONTENT);
    }
}

#[sqlx::test]
async fn an_address_gets_20_attempts_a_minute(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;

    // Different usernames, so only the address limit applies.
    for attempt in 0..20 {
        let reply = sign_in_from(&app, "203.0.113.7", &format!("user{attempt}"), WRONG).await;
        assert_refused(&reply, StatusCode::UNAUTHORIZED);
    }
    let reply = sign_in_from(&app, "203.0.113.7", "admin", PASSWORD).await;
    assert_refused(&reply, StatusCode::TOO_MANY_REQUESTS);
    // A passkey attempt counts against the same limit, before its ceremony is even looked up.
    let passkey = app
        .call_with(
            "POST",
            "/auth/passkey-sign-in",
            Some(&json!({
                "ceremonyId": "00000000-0000-0000-0000-000000000000",
                "credential": Authenticator::new(1).sign_in(&json!({ "publicKey": { "challenge": "AAAA", "rpId": "desa.example" } })),
                "remember": false,
            })),
            None,
            &[("x-forwarded-for", "203.0.113.7")],
        )
        .await;
    assert_refused(&passkey, StatusCode::TOO_MANY_REQUESTS);

    // Only the last address, the one the web app vouches for, counts.
    let reply = sign_in_from(&app, "203.0.113.7, 198.51.100.1", "admin", PASSWORD).await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    let reply = sign_in_from(&app, "198.51.100.1, 203.0.113.7", "admin", PASSWORD).await;
    assert_refused(&reply, StatusCode::TOO_MANY_REQUESTS);
}
