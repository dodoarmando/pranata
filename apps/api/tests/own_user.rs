mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::json;
use sqlx::PgPool;

const FIREFOX: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:150.0) Gecko/20100101 Firefox/150.0";
const ANDROID: &str = "Mozilla/5.0 (Linux; Android 15) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0 Mobile Safari/537.36";

#[sqlx::test]
async fn a_user_sees_their_live_sessions_with_browser_address_and_last_activity(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let laptop = app
        .sign_in_from(
            "admin",
            false,
            &[("user-agent", FIREFOX), ("x-forwarded-for", "203.0.113.7")],
        )
        .await;
    app.sign_in_from(
        "admin",
        true,
        &[("user-agent", ANDROID), ("x-forwarded-for", "2001:db8::1")],
    )
    .await;
    // A forwarded address that is not one is dropped.
    app.sign_in_from("admin", false, &[("x-forwarded-for", "not an address")])
        .await;
    // Another User's session is not listed.
    app.administrator(&pool, "admin2").await;
    app.sign_in("admin2", false).await;

    let reply = app.call("GET", "/auth/sessions", None, Some(&laptop)).await;

    assert_eq!(reply.status, StatusCode::OK);
    let mut sessions = reply.body.as_array().unwrap().clone();
    assert_eq!(sessions.len(), 3);
    for session in &mut sessions {
        assert!(session["lastSeenAt"].is_string(), "{session}");
        session.as_object_mut().unwrap().remove("lastSeenAt");
    }
    for expected in [
        json!({ "userAgent": FIREFOX, "ip": "203.0.113.7", "current": true }),
        json!({ "userAgent": ANDROID, "ip": "2001:db8::1", "current": false }),
        json!({ "userAgent": null, "ip": null, "current": false }),
    ] {
        assert!(sessions.contains(&expected), "{expected} in {sessions:?}");
    }
}

#[sqlx::test]
async fn akun_saya_needs_a_session(pool: PgPool) {
    let app = TestApp::new(pool);
    for (method, path, body) in [("GET", "/auth/sessions", None)] {
        let reply = app.call(method, path, body, None).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{method} {path}");
    }
}
