mod common;

use axum::http::StatusCode;
use common::{PASSWORD, TestApp};
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test]
async fn signing_in_starts_a_session_that_signing_out_ends(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.admin(&pool, "admin").await;

    let sign_in = app
        .call(
            "POST",
            "/auth/sign-in",
            Some(&json!({ "username": "admin", "password": PASSWORD, "remember": false })),
            None,
        )
        .await;
    assert_eq!(sign_in.status, StatusCode::NO_CONTENT);
    let (cookie, attributes) = sign_in.set_cookie().split_once("; ").unwrap();
    let token = cookie.strip_prefix("__Host-session=").unwrap();
    assert!(token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()));
    // No Max-Age: the cookie dies with the browser.
    assert_eq!(attributes, "Path=/; Secure; HttpOnly; SameSite=Lax");

    let me = app.call("GET", "/auth/me", None, Some(cookie)).await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(
        me.body,
        json!({ "username": "admin", "role": "Administrator" })
    );

    let sign_out = app.call("POST", "/auth/sign-out", None, Some(cookie)).await;
    assert_eq!(sign_out.status, StatusCode::NO_CONTENT);
    assert!(sign_out.set_cookie().starts_with("__Host-session=; "));
    assert!(sign_out.set_cookie().ends_with("; Max-Age=0"));
    let me = app.call("GET", "/auth/me", None, Some(cookie)).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn ingat_saya_keeps_the_cookie_for_the_30_day_absolute_lifetime(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.admin(&pool, "admin").await;

    let sign_in = app
        .call(
            "POST",
            "/auth/sign-in",
            Some(&json!({ "username": "admin", "password": PASSWORD, "remember": true })),
            None,
        )
        .await;

    assert!(sign_in.set_cookie().ends_with("; Max-Age=2592000"));
}

#[sqlx::test]
async fn wrong_credentials_get_one_generic_answer(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.admin(&pool, "admin").await;
    // Created, but the Access Link was never used, so there is no password yet.
    pranata::user::create_admin(&pool, "pending").await.unwrap();

    for (username, password) in [
        ("admin", "not the password at all"),
        ("nobody", PASSWORD),
        ("pending", PASSWORD),
        ("pending", ""),
    ] {
        let reply = app
            .call(
                "POST",
                "/auth/sign-in",
                Some(&json!({ "username": username, "password": password, "remember": false })),
                None,
            )
            .await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{username}");
        assert_eq!(reply.body, serde_json::Value::Null, "{username}");
        assert!(!reply.headers.contains_key("set-cookie"), "{username}");
    }
}

#[sqlx::test]
async fn sessions_time_out_when_idle_and_at_their_absolute_limit(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.admin(&pool, "admin").await;
    let shift = |column: &'static str, by: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE sessions SET {column} = {column} - interval '{by}'"
            )))
            .execute(&pool)
            .await
            .unwrap();
        }
    };

    for (remember, idle_less, idle_more, absolute_less, absolute_more) in [
        (
            false,
            "29 minutes",
            "31 minutes",
            "11 hours 59 minutes",
            "2 minutes",
        ),
        (
            true,
            "6 days 23 hours",
            "7 days 1 minute",
            "29 days 23 hours",
            "2 hours",
        ),
    ] {
        sqlx::query("DELETE FROM sessions")
            .execute(&pool)
            .await
            .unwrap();
        let cookie = app.sign_in("admin", remember).await;
        let me = || app.call("GET", "/auth/me", None, Some(&cookie));

        shift("last_seen_at", idle_less).await;
        assert_eq!(me().await.status, StatusCode::OK, "{remember} idle");
        shift("last_seen_at", idle_more).await;
        assert_eq!(
            me().await.status,
            StatusCode::UNAUTHORIZED,
            "{remember} idle"
        );

        sqlx::query("DELETE FROM sessions")
            .execute(&pool)
            .await
            .unwrap();
        let cookie = app.sign_in("admin", remember).await;
        let me = || app.call("GET", "/auth/me", None, Some(&cookie));

        shift("expires_at", absolute_less).await;
        assert_eq!(me().await.status, StatusCode::OK, "{remember} absolute");
        shift("expires_at", absolute_more).await;
        assert_eq!(
            me().await.status,
            StatusCode::UNAUTHORIZED,
            "{remember} absolute"
        );
    }
}

#[sqlx::test]
async fn a_hash_with_old_parameters_is_replaced_on_sign_in(pool: PgPool) {
    use argon2::{Argon2, Params, password_hash::PasswordHasher};

    let app = TestApp::new(pool.clone());
    app.admin(&pool, "admin").await;
    let weak = Argon2::from(Params::new(8, 1, 1, None).unwrap())
        .hash_password(PASSWORD.as_bytes())
        .unwrap()
        .to_string();
    sqlx::query("UPDATE users SET password_hash = $1")
        .bind(&weak)
        .execute(&pool)
        .await
        .unwrap();

    app.sign_in("admin", false).await;

    let stored: String = sqlx::query_scalar("SELECT password_hash FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        stored.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"),
        "{stored}"
    );
    app.sign_in("admin", false).await;
}
