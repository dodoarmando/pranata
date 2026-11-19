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

async fn challenge(app: &TestApp, username: &str) -> Value {
    let body = json!({ "username": username });
    let reply = app
        .call("POST", "/auth/passkey-sign-in/challenge", Some(&body), None)
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    reply.body
}

async fn finish_sign_in(app: &TestApp, challenge: &Value, credential: Value) -> common::Reply {
    let body = json!({ "ceremonyId": challenge["ceremonyId"], "credential": credential, "remember": true });
    app.call("POST", "/auth/passkey-sign-in", Some(&body), None)
        .await
}

#[sqlx::test]
async fn a_passkey_signs_the_user_in_without_their_password(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;
    let mut passkey = Authenticator::new(1);
    app.add_passkey(&cookie, "Laptop", &passkey).await;

    let challenge = challenge(&app, "admin").await;
    let reply = finish_sign_in(&app, &challenge, passkey.sign_in(&challenge["options"])).await;

    assert_eq!(reply.status, StatusCode::NO_CONTENT, "{:?}", reply.body);
    assert!(reply.set_cookie().contains("Max-Age=2592000"));
    let session = reply.set_cookie().split(';').next().unwrap().to_owned();
    let me = app.call("GET", "/auth/me", None, Some(&session)).await;
    assert_eq!(me.body["username"], "admin");
    assert_eq!(
        last_entry(&pool).await,
        (
            "session.sign_in".to_owned(),
            Some(user_id(&pool, "admin").await),
            json!({ "remember": true, "passkey": "Laptop" })
        )
    );
    assert!(list(&app, &session).await[0]["lastUsedAt"].is_string());
    // A passkey sign-in is a fresh authentication, so sudo mode holds.
    let start = app
        .call("POST", "/auth/passkeys/registration", None, Some(&session))
        .await;
    assert_eq!(start.status, StatusCode::OK);
}

#[sqlx::test]
async fn a_forged_or_replayed_assertion_is_refused_and_audited(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;
    let mut passkey = Authenticator::new(1);
    app.add_passkey(&cookie, "Laptop", &passkey).await;
    let admin = user_id(&pool, "admin").await;

    let first = challenge(&app, "admin").await;
    let forged = finish_sign_in(&app, &first, passkey.forged().sign_in(&first["options"])).await;
    assert_eq!(forged.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        last_entry(&pool).await,
        (
            "session.sign_in_failed".to_owned(),
            Some(admin.clone()),
            json!({})
        )
    );

    let second = challenge(&app, "admin").await;
    let credential = passkey.sign_in(&second["options"]);
    let signed_in = finish_sign_in(&app, &second, credential.clone()).await;
    assert_eq!(signed_in.status, StatusCode::NO_CONTENT);
    let replayed = finish_sign_in(&app, &second, credential).await;
    assert_eq!(replayed.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn unknown_users_get_a_decoy_challenge_shaped_like_a_real_one(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    app.administrator(&pool, "nopasskey").await;
    let cookie = app.sign_in("admin", false).await;
    app.add_passkey(&cookie, "Laptop", &Authenticator::new(1))
        .await;

    let real = challenge(&app, "admin").await;
    // The option keys, and each allowed credential's without its id, whose number and lengths
    // a decoy draws from those of real passkeys.
    let shape = |challenge: &Value| {
        let options = challenge["options"]["publicKey"].as_object().unwrap();
        let mut keys: Vec<_> = options.keys().cloned().collect();
        keys.sort();
        let credentials: Vec<_> = options["allowCredentials"]
            .as_array()
            .unwrap()
            .iter()
            .map(|credential| (credential["type"].clone(), credential["transports"].clone()))
            .collect();
        (keys, credentials)
    };
    let (real_keys, real_credentials) = shape(&real);
    for username in ["nobody", "nopasskey"] {
        let decoy = challenge(&app, username).await;
        let (keys, credentials) = shape(&decoy);
        assert_eq!(keys, real_keys, "{username}");
        assert!(
            credentials
                .iter()
                .all(|credential| *credential == real_credentials[0])
        );
        assert_ne!(
            decoy["options"]["publicKey"]["challenge"],
            real["options"]["publicKey"]["challenge"]
        );
        // The same decoy credential each time, as a real User's would be.
        let again = challenge(&app, username).await;
        assert_eq!(
            again["options"]["publicKey"]["allowCredentials"],
            decoy["options"]["publicKey"]["allowCredentials"]
        );
        let reply = finish_sign_in(
            &app,
            &decoy,
            Authenticator::new(1).sign_in(&decoy["options"]),
        )
        .await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{username}");
        assert_eq!(last_entry(&pool).await.0, "session.sign_in_failed");
    }
}

#[sqlx::test]
async fn a_deactivated_user_cannot_sign_in_with_a_passkey(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;
    let mut passkey = Authenticator::new(1);
    app.add_passkey(&cookie, "Laptop", &passkey).await;
    let started = challenge(&app, "admin").await;
    sqlx::query("UPDATE users SET active = false")
        .execute(&pool)
        .await
        .unwrap();

    // Deactivated mid-ceremony, and before it starts.
    let reply = finish_sign_in(&app, &started, passkey.sign_in(&started["options"])).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    let decoy = challenge(&app, "admin").await;
    let reply = finish_sign_in(&app, &decoy, passkey.sign_in(&decoy["options"])).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
}
