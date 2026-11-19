mod common;

use axum::http::StatusCode;
use common::{TestApp, authenticator::Authenticator};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn last_entry(pool: &PgPool) -> (String, Option<String>, Value) {
    sqlx::query_as("SELECT action, object_id, details FROM audit_entries ORDER BY id DESC LIMIT 1")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn user_id(pool: &PgPool, username: &str) -> String {
    sqlx::query_scalar("SELECT id::text FROM users WHERE username = $1")
        .bind(username)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn list(app: &TestApp, cookie: &str) -> Vec<Value> {
    let reply = app.call("GET", "/auth/passkeys", None, Some(cookie)).await;
    assert_eq!(reply.status, StatusCode::OK);
    reply.body.as_array().unwrap().clone()
}

#[sqlx::test]
async fn a_user_adds_a_named_passkey_and_sees_it_listed(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;

    let reply = app
        .add_passkey(&cookie, " Laptop kantor ", &Authenticator::new(1))
        .await;

    assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.body);
    assert_eq!(reply.body["name"], "Laptop kantor");
    let passkeys = list(&app, &cookie).await;
    assert_eq!(passkeys.len(), 1);
    assert_eq!(passkeys[0]["name"], "Laptop kantor");
    assert!(passkeys[0]["createdAt"].is_string());
    assert!(passkeys[0]["lastUsedAt"].is_null());
    assert_eq!(
        last_entry(&pool).await,
        (
            "passkey.create".to_owned(),
            Some(user_id(&pool, "admin").await),
            json!({ "name": "Laptop kantor" })
        )
    );
}

#[sqlx::test]
async fn a_passkey_needs_a_name_and_is_never_registered_twice(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;
    let passkey = Authenticator::new(1);

    for name in ["  ", &"x".repeat(65)] {
        let reply = app.add_passkey(&cookie, name, &passkey).await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{name:?}");
    }
    assert_eq!(
        app.add_passkey(&cookie, "Laptop", &passkey).await.status,
        StatusCode::CREATED
    );
    // The browser is told to exclude it, so only a misbehaving client sends it again.
    let challenge = app
        .call("POST", "/auth/passkeys/registration", None, Some(&cookie))
        .await;
    let excluded = &challenge.body["options"]["publicKey"]["excludeCredentials"];
    assert_eq!(excluded.as_array().unwrap().len(), 1);
    // Another User registering the same credential is refused too.
    app.administrator(&pool, "admin2").await;
    let other = app.sign_in("admin2", false).await;
    for cookie in [&cookie, &other] {
        let reply = app.add_passkey(cookie, "Lagi", &passkey).await;
        assert_eq!(reply.status, StatusCode::CONFLICT);
    }
    assert_eq!(list(&app, &cookie).await.len(), 1);
    assert_eq!(list(&app, &other).await, Vec::<Value>::new());
}

#[sqlx::test]
async fn a_registration_is_single_use_and_bound_to_the_user_who_started_it(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    app.administrator(&pool, "admin2").await;
    let cookie = app.sign_in("admin", false).await;
    let other = app.sign_in("admin2", false).await;
    let challenge = app
        .call("POST", "/auth/passkeys/registration", None, Some(&cookie))
        .await;
    let body = json!({
        "ceremonyId": challenge.body["ceremonyId"],
        "name": "Laptop",
        "credential": Authenticator::new(1).register(&challenge.body["options"]),
    });

    let stolen = app
        .call("POST", "/auth/passkeys", Some(&body), Some(&other))
        .await;
    assert_eq!(stolen.status, StatusCode::NOT_FOUND);
    // The attempt used the ceremony up.
    let replayed = app
        .call("POST", "/auth/passkeys", Some(&body), Some(&cookie))
        .await;
    assert_eq!(replayed.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn adding_and_removing_passkeys_needs_sudo_mode(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;
    let added = app
        .add_passkey(&cookie, "Laptop", &Authenticator::new(1))
        .await;
    sqlx::query("UPDATE sessions SET authenticated_at = now() - interval '16 minutes'")
        .execute(&pool)
        .await
        .unwrap();

    let start = app
        .call("POST", "/auth/passkeys/registration", None, Some(&cookie))
        .await;
    assert_eq!(start.status, StatusCode::PRECONDITION_REQUIRED);
    let path = format!("/auth/passkeys/{}", added.body["id"].as_str().unwrap());
    let remove = app.call("DELETE", &path, None, Some(&cookie)).await;
    assert_eq!(remove.status, StatusCode::PRECONDITION_REQUIRED);
    assert_eq!(list(&app, &cookie).await.len(), 1);
}

#[sqlx::test]
async fn a_user_removes_their_own_passkeys_only(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    app.administrator(&pool, "admin2").await;
    let cookie = app.sign_in("admin", false).await;
    let other = app.sign_in("admin2", false).await;
    let laptop = app
        .add_passkey(&cookie, "Laptop", &Authenticator::new(1))
        .await;
    app.add_passkey(&cookie, "Ponsel", &Authenticator::new(2))
        .await;
    let path = format!("/auth/passkeys/{}", laptop.body["id"].as_str().unwrap());

    let foreign = app.call("DELETE", &path, None, Some(&other)).await;
    assert_eq!(foreign.status, StatusCode::NOT_FOUND);
    let own = app.call("DELETE", &path, None, Some(&cookie)).await;
    assert_eq!(own.status, StatusCode::NO_CONTENT);

    let passkeys = list(&app, &cookie).await;
    assert_eq!(passkeys.len(), 1);
    assert_eq!(passkeys[0]["name"], "Ponsel");
    assert_eq!(
        last_entry(&pool).await,
        (
            "passkey.delete".to_owned(),
            Some(user_id(&pool, "admin").await),
            json!({ "name": "Laptop" })
        )
    );
    let gone = app.call("DELETE", &path, None, Some(&cookie)).await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);
}
