mod common;

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use common::{FixedClock, test_state_with_clock};
use http_body_util::BodyExt;
use pranata::{auth, ids::UserId};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use time::Duration;
use tower::ServiceExt;

const PASSWORD: &str = "correct horse battery";

struct Harness {
    pool: PgPool,
    clock: Arc<FixedClock>,
    app: Router,
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}

impl Harness {
    fn new(pool: PgPool) -> Self {
        capture_logs();
        let clock = FixedClock::new();
        let app = pranata::app(test_state_with_clock(pool.clone(), clock.clone()));
        Self { pool, clock, app }
    }

    async fn seed_user(&self, username: &str) -> UserId {
        let hash = auth::password::hash_password(PASSWORD).unwrap();
        sqlx::query_scalar!(
            r#"INSERT INTO users (username, display_name, password_hash)
               VALUES ($1, 'Budi Santoso', $2) RETURNING id AS "id: UserId""#,
            username,
            hash
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn send(
        &self,
        method: Method,
        uri: &str,
        cookie: Option<&str>,
        csrf: Option<&str>,
        body: Option<Value>,
    ) -> Reply {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(cookie) = cookie {
            builder = builder.header(header::COOKIE, format!("pranata_session={cookie}"));
        }
        if let Some(csrf) = csrf {
            builder = builder.header("x-csrf-token", csrf);
        }
        let request = match body {
            Some(body) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => builder.body(Body::empty()),
        }
        .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        Reply {
            status: parts.status,
            headers: parts.headers,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        }
    }

    async fn login(&self, username: &str, password: &str, remember: bool) -> Reply {
        self.send(
            Method::POST,
            "/api/auth/login",
            None,
            None,
            Some(json!({"username": username, "password": password, "remember": remember})),
        )
        .await
    }

    async fn redeem(&self, token: &str, password: &str) -> Reply {
        self.send(
            Method::POST,
            "/api/auth/access-links/redeem",
            None,
            None,
            Some(json!({"token": token, "password": password})),
        )
        .await
    }

    async fn me(&self, token: &str) -> Reply {
        self.send(Method::GET, "/api/auth/me", Some(token), None, None)
            .await
    }
}

fn set_cookie(reply: &Reply) -> String {
    reply.headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_owned()
}

fn token_of(reply: &Reply) -> String {
    set_cookie(reply)
        .strip_prefix("pranata_session=")
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

fn sha256(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

#[sqlx::test]
async fn login_sets_session_cookie_and_returns_csrf(pool: PgPool) {
    let h = Harness::new(pool);
    let id = h.seed_user("budi").await;

    let reply = h.login("budi", PASSWORD, false).await;
    assert_eq!(reply.status, StatusCode::OK);

    let cookie = set_cookie(&reply);
    for flag in ["HttpOnly", "Secure", "SameSite=Lax", "Path=/"] {
        assert!(cookie.contains(flag), "missing {flag} in {cookie}");
    }
    assert!(
        !cookie.contains("Max-Age"),
        "session cookie must not persist"
    );
    let token = token_of(&reply);
    assert_eq!(token.len(), 43, "32 bytes in unpadded base64url");

    let csrf = reply.body["csrf_token"].as_str().unwrap();
    assert_eq!(csrf.len(), 43);
    assert_eq!(reply.body["user"]["username"], "budi");
    assert_eq!(reply.body["user"]["display_name"], "Budi Santoso");
    assert_eq!(reply.body["user"]["id"], json!(id));
    assert!(reply.body["user"].get("password_hash").is_none());

    // Only the SHA-256 of the token is stored.
    let stored = sqlx::query_scalar!("SELECT token_hash FROM sessions")
        .fetch_all(&h.pool)
        .await
        .unwrap();
    assert_eq!(stored, vec![sha256(&token)]);

    let me = h.me(&token).await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.body["csrf_token"], csrf);
    assert_eq!(me.body["user"]["username"], "budi");
}

#[sqlx::test]
async fn me_without_a_session_is_unauthenticated(pool: PgPool) {
    let h = Harness::new(pool);
    let reply = h.send(Method::GET, "/api/auth/me", None, None, None).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.body["code"], "unauthenticated");

    let reply = h.me("not-a-real-token").await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.body["code"], "unauthenticated");
}

#[sqlx::test]
async fn wrong_password_and_unknown_user_look_identical(pool: PgPool) {
    let h = Harness::new(pool);
    h.seed_user("budi").await;

    let wrong = h.login("budi", "not the password", false).await;
    let unknown = h.login("nobody", "not the password", false).await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong.status, unknown.status);
    assert_eq!(wrong.body, unknown.body);
    assert_eq!(wrong.body["code"], "invalid_credentials");
    assert!(wrong.headers.get(header::SET_COOKIE).is_none());
    assert!(unknown.headers.get(header::SET_COOKIE).is_none());
}

#[sqlx::test]
async fn account_without_password_or_inactive_cannot_sign_in(pool: PgPool) {
    let h = Harness::new(pool);
    sqlx::query!("INSERT INTO users (username, display_name) VALUES ('baru', 'Baru')")
        .execute(&h.pool)
        .await
        .unwrap();
    let id = h.seed_user("budi").await;
    sqlx::query!("UPDATE users SET is_active = false WHERE id = $1", id.0)
        .execute(&h.pool)
        .await
        .unwrap();

    for username in ["baru", "budi"] {
        let reply = h.login(username, PASSWORD, false).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{username}");
        assert_eq!(reply.body["code"], "invalid_credentials");
    }
}

#[sqlx::test]
async fn deactivating_a_user_ends_their_sessions(pool: PgPool) {
    let h = Harness::new(pool);
    let id = h.seed_user("budi").await;
    let token = token_of(&h.login("budi", PASSWORD, false).await);
    assert_eq!(h.me(&token).await.status, StatusCode::OK);

    sqlx::query!("UPDATE users SET is_active = false WHERE id = $1", id.0)
        .execute(&h.pool)
        .await
        .unwrap();
    assert_eq!(h.me(&token).await.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn session_expires_after_12h_without_remember(pool: PgPool) {
    let h = Harness::new(pool);
    h.seed_user("budi").await;
    let token = token_of(&h.login("budi", PASSWORD, false).await);

    h.clock.advance(Duration::hours(11) + Duration::minutes(59));
    assert_eq!(h.me(&token).await.status, StatusCode::OK);
    // No sliding window without "remember": activity does not extend it.
    h.clock.advance(Duration::minutes(2));
    let reply = h.me(&token).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.body["code"], "unauthenticated");
}

#[sqlx::test]
async fn remember_extends_to_30_days(pool: PgPool) {
    let h = Harness::new(pool);
    h.seed_user("budi").await;
    let login = h.login("budi", PASSWORD, true).await;
    assert!(set_cookie(&login).contains("Max-Age=2592000"));
    let token = token_of(&login);

    let expiry = || async {
        sqlx::query_scalar!("SELECT expires_at FROM sessions")
            .fetch_one(&h.pool)
            .await
            .unwrap()
    };
    let first = expiry().await;
    assert_eq!(first - h.clock.now_for_test(), Duration::days(30));

    // Within the hour nothing is renewed and no cookie is re-issued.
    h.clock.advance(Duration::minutes(30));
    let reply = h.me(&token).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(reply.headers.get(header::SET_COOKIE).is_none());
    assert_eq!(expiry().await, first);

    // After an hour the expiry slides forward and the cookie is refreshed.
    h.clock.advance(Duration::minutes(31));
    let reply = h.me(&token).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(set_cookie(&reply).contains("Max-Age=2592000"));
    assert_eq!(token_of(&reply), token);
    assert!(expiry().await > first);

    // Keep using it for 40 days in 20 day steps: still signed in.
    h.clock.advance(Duration::days(20));
    assert_eq!(h.me(&token).await.status, StatusCode::OK);
    h.clock.advance(Duration::days(20));
    assert_eq!(h.me(&token).await.status, StatusCode::OK);

    // Idle for more than 30 days: gone.
    h.clock.advance(Duration::days(31));
    assert_eq!(h.me(&token).await.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn admin_post_without_csrf_is_403(pool: PgPool) {
    let h = Harness::new(pool);
    h.seed_user("budi").await;
    let login = h.login("budi", PASSWORD, false).await;
    let token = token_of(&login);
    let csrf = login.body["csrf_token"].as_str().unwrap().to_owned();
    let path = "/api/admin/anything";

    // Not signed in: 401 before CSRF is even considered.
    let reply = h.send(Method::POST, path, None, None, None).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.body["code"], "unauthenticated");

    for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
        let missing = h.send(method.clone(), path, Some(&token), None, None).await;
        assert_eq!(missing.status, StatusCode::FORBIDDEN, "{method}");
        assert_eq!(missing.body["code"], "csrf_mismatch");

        let wrong = h
            .send(method.clone(), path, Some(&token), Some("wrong"), None)
            .await;
        assert_eq!(wrong.status, StatusCode::FORBIDDEN, "{method}");
        assert_eq!(wrong.body["code"], "csrf_mismatch");

        // The right token gets past the guard (the route itself does not exist).
        let ok = h.send(method, path, Some(&token), Some(&csrf), None).await;
        assert_eq!(ok.status, StatusCode::NOT_FOUND);
    }

    // Reads are not state-changing: no token needed.
    let get = h.send(Method::GET, path, Some(&token), None, None).await;
    assert_eq!(get.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn logout_requires_csrf_then_ends_the_session(pool: PgPool) {
    let h = Harness::new(pool);
    let id = h.seed_user("budi").await;
    let login = h.login("budi", PASSWORD, false).await;
    let token = token_of(&login);
    let csrf = login.body["csrf_token"].as_str().unwrap().to_owned();

    let denied = h
        .send(Method::POST, "/api/auth/logout", Some(&token), None, None)
        .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    assert_eq!(denied.body["code"], "csrf_mismatch");
    assert_eq!(h.me(&token).await.status, StatusCode::OK);

    let out = h
        .send(
            Method::POST,
            "/api/auth/logout",
            Some(&token),
            Some(&csrf),
            None,
        )
        .await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    let cleared = set_cookie(&out);
    assert!(cleared.contains("Max-Age=0"), "{cleared}");
    assert!(cleared.contains("HttpOnly"));
    assert_eq!(h.me(&token).await.status, StatusCode::UNAUTHORIZED);

    let sessions = sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM sessions"#)
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert_eq!(sessions, 0);

    let entry = sqlx::query!(
        "SELECT actor, object_type, object_id FROM audit_entries WHERE action = 'sign_out'"
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(entry.actor, Some(id.0));
    assert_eq!(entry.object_type, "user");
    assert_eq!(entry.object_id, id.0.to_string());
}

#[sqlx::test]
async fn sign_in_writes_audit_entry(pool: PgPool) {
    let h = Harness::new(pool);
    let id = h.seed_user("budi").await;

    h.login("budi", "not the password", false).await;
    let none = sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM audit_entries"#)
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert_eq!(none, 0, "failed sign-ins write no entry");

    let reply = h.login("budi", PASSWORD, false).await;
    let token = token_of(&reply);

    let entries = sqlx::query!(
        "SELECT actor, action, object_type, object_id, summary, changes FROM audit_entries"
    )
    .fetch_all(&h.pool)
    .await
    .unwrap();
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.action, "sign_in");
    assert_eq!(entry.actor, Some(id.0));
    assert_eq!(entry.object_type, "user");
    assert_eq!(entry.object_id, id.0.to_string());

    let dump = format!("{entry:?}");
    assert!(!dump.contains(PASSWORD));
    assert!(!dump.contains(&token));
}

#[sqlx::test]
async fn access_link_sets_password_once(pool: PgPool) {
    let h = Harness::new(pool);
    let created =
        auth::create_user_with_access_link(&h.pool, "Siti", "Siti Aminah", h.clock.now_for_test())
            .await
            .unwrap();
    let link_token = created.access_link_token.clone();

    // No password yet: sign-in is refused.
    let early = h.login("siti", PASSWORD, false).await;
    assert_eq!(early.status, StatusCode::UNAUTHORIZED);

    let set = h.redeem(&link_token, PASSWORD).await;
    assert_eq!(set.status, StatusCode::NO_CONTENT);
    assert_eq!(
        h.login("siti", PASSWORD, false).await.status,
        StatusCode::OK
    );

    let again = h.redeem(&link_token, "another long password").await;
    assert_eq!(again.status, StatusCode::GONE);
    assert_eq!(again.body["code"], "access_link_used");
    // The second attempt did not change the password.
    assert_eq!(
        h.login("siti", PASSWORD, false).await.status,
        StatusCode::OK
    );

    let unknown = h.redeem("nope", PASSWORD).await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    assert_eq!(unknown.body["code"], "access_link_not_found");

    // The token is stored hashed and the audit entry carries no secrets.
    let stored = sqlx::query_scalar!("SELECT token_hash FROM access_links")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert_eq!(stored, sha256(&created.access_link_token));
    let audit = sqlx::query!("SELECT summary, changes FROM audit_entries")
        .fetch_all(&h.pool)
        .await
        .unwrap();
    let dump = format!("{audit:?}");
    assert!(!dump.contains(PASSWORD));
    assert!(!dump.contains(&created.access_link_token));
}

#[sqlx::test]
async fn using_an_access_link_signs_out_existing_sessions(pool: PgPool) {
    let h = Harness::new(pool);
    let id = h.seed_user("budi").await;
    let token = token_of(&h.login("budi", PASSWORD, false).await);
    let link = auth::access_link::create(
        &mut h.pool.acquire().await.unwrap(),
        id,
        None,
        h.clock.now_for_test(),
    )
    .await
    .unwrap();

    let link_token = link;
    let set = h.redeem(&link_token, "a brand new password").await;
    assert_eq!(set.status, StatusCode::NO_CONTENT);
    assert_eq!(h.me(&token).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        h.login("budi", "a brand new password", false).await.status,
        StatusCode::OK
    );
}

#[sqlx::test]
async fn expired_access_link_is_410(pool: PgPool) {
    let h = Harness::new(pool);
    let created =
        auth::create_user_with_access_link(&h.pool, "siti", "Siti", h.clock.now_for_test())
            .await
            .unwrap();
    let link_token = created.access_link_token.clone();

    h.clock.advance(Duration::hours(24));
    let reply = h.redeem(&link_token, PASSWORD).await;
    assert_eq!(reply.status, StatusCode::GONE);
    assert_eq!(reply.body["code"], "access_link_expired");
    assert_eq!(
        h.login("siti", PASSWORD, false).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test]
async fn access_link_works_just_before_it_expires(pool: PgPool) {
    let h = Harness::new(pool);
    let created =
        auth::create_user_with_access_link(&h.pool, "siti", "Siti", h.clock.now_for_test())
            .await
            .unwrap();
    let link_token = created.access_link_token.clone();

    h.clock.advance(Duration::hours(23) + Duration::minutes(59));
    let reply = h.redeem(&link_token, PASSWORD).await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
}

#[sqlx::test]
async fn short_password_rejected(pool: PgPool) {
    let h = Harness::new(pool);
    let created =
        auth::create_user_with_access_link(&h.pool, "siti", "Siti", h.clock.now_for_test())
            .await
            .unwrap();
    let link_token = created.access_link_token.clone();

    let reply = h.redeem(&link_token, "too short").await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.body["code"], "validation_failed");
    assert_eq!(
        reply.body["errors"],
        json!([{"field": "password", "code": "password_length"}])
    );

    // A rejected password does not burn the link.
    let ok = h.redeem(&link_token, PASSWORD).await;
    assert_eq!(ok.status, StatusCode::NO_CONTENT);
}

#[sqlx::test]
async fn create_user_normalises_and_rejects_bad_or_duplicate_usernames(pool: PgPool) {
    let h = Harness::new(pool);
    let now = h.clock.now_for_test();
    let created = auth::create_user_with_access_link(&h.pool, " Siti.A ", "Siti", now)
        .await
        .unwrap();
    let username = sqlx::query_scalar!("SELECT username FROM users WHERE id = $1", created.id.0)
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert_eq!(username, "siti.a");

    let duplicate = auth::create_user_with_access_link(&h.pool, "SITI.A", "Other", now).await;
    assert!(matches!(
        duplicate,
        Err(pranata::ApiError::Conflict("username_taken"))
    ));
    let invalid = auth::create_user_with_access_link(&h.pool, "x", "Other", now).await;
    assert!(matches!(invalid, Err(pranata::ApiError::Validation(_))));

    let audit = sqlx::query!("SELECT actor, action, object_id FROM audit_entries")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert_eq!(audit.action, "create");
    assert_eq!(audit.actor, None);
    assert_eq!(audit.object_id, created.id.0.to_string());
}

#[sqlx::test]
async fn audit_entries_survive_user_deactivation(pool: PgPool) {
    let h = Harness::new(pool);
    let id = h.seed_user("budi").await;
    h.login("budi", PASSWORD, false).await;
    sqlx::query!("UPDATE users SET is_active = false WHERE id = $1", id.0)
        .execute(&h.pool)
        .await
        .unwrap();
    let kept = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM audit_entries WHERE actor = $1"#,
        id.0
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(kept, 1);

    // The foreign key refuses an actor that is not a User.
    let ghost = sqlx::query!(
        "INSERT INTO audit_entries (actor, action, object_type, object_id, summary)
         VALUES (gen_random_uuid(), 'update', 'x', 'y', 'z')"
    )
    .execute(&h.pool)
    .await;
    assert!(ghost.is_err());
}

/// Collects tracing output so a test can search it.
#[derive(Clone, Default)]
struct LogBuffer(Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for LogBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Install (once per test binary) a process-wide subscriber that writes into a
/// buffer. A per-thread subscriber is unreliable here: tracing caches callsite
/// interest globally, so parallel tests without one can switch spans off.
fn capture_logs() -> LogBuffer {
    static LOGS: std::sync::OnceLock<LogBuffer> = std::sync::OnceLock::new();
    LOGS.get_or_init(|| {
        let logs = LogBuffer::default();
        let writer = logs.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::set_global_default(subscriber).expect("one global subscriber");
        logs
    })
    .clone()
}

#[sqlx::test]
async fn redeeming_an_access_link_logs_no_token_or_password(pool: PgPool) {
    let h = Harness::new(pool);
    let logs = capture_logs();
    let created =
        auth::create_user_with_access_link(&h.pool, "siti", "Siti", h.clock.now_for_test())
            .await
            .unwrap();
    let token = created.access_link_token;
    let secret = "a very secret password";

    // A failed attempt and a successful one.
    assert_eq!(
        h.redeem(&token, "short").await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        h.redeem(&token, secret).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(h.redeem(&token, secret).await.status, StatusCode::GONE);

    let output = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    assert!(
        output.contains("/api/auth/access-links/redeem"),
        "the request was not captured: {output}"
    );
    assert!(!output.contains(&token), "token leaked into the logs");
    assert!(!output.contains(secret), "password leaked into the logs");
}

async fn status_of(
    app: Router,
    method: Method,
    uri: &str,
    cookie: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, format!("pranata_session={cookie}"));
    }
    let response = app
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[sqlx::test]
async fn extractor_fails_closed_outside_the_guarded_router(pool: PgPool) {
    use axum::routing::{get, post};
    let h = Harness::new(pool);
    h.seed_user("budi").await;
    let token = token_of(&h.login("budi", PASSWORD, false).await);
    let state = test_state_with_clock(h.pool.clone(), h.clock.clone());

    // A route mounted by hand, without the `resolve` middleware: even with a
    // valid cookie the extractor must not authenticate on its own.
    let app = Router::new()
        .route("/x", get(|_user: auth::CurrentUser| async { "ok" }))
        .with_state(state.clone());
    let (status, body) = status_of(app, Method::GET, "/x", Some(&token)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["code"], "internal_error");

    // The CSRF guard without `resolve` rejects protected requests too, rather
    // than letting them through unchecked.
    let app = Router::new()
        .route("/api/admin/x", post(|| async { "ok" }))
        .layer(axum::middleware::from_fn(auth::csrf::guard))
        .with_state(state);
    let (status, body) = status_of(app, Method::POST, "/api/admin/x", Some(&token)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["code"], "internal_error");
}
