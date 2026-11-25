mod common;

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use common::{FixedClock, test_state_with_clock};
use http_body_util::BodyExt;
use pranata::{auth, ids::UserId};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use tower::ServiceExt;

const PASSWORD: &str = "correct horse battery";

struct Harness {
    pool: PgPool,
    clock: Arc<FixedClock>,
    app: Router,
}

struct Reply {
    status: StatusCode,
    body: Value,
    token: Option<String>,
}

struct Signed {
    token: String,
    csrf: String,
}

impl Harness {
    fn new(pool: PgPool) -> Self {
        let clock = FixedClock::new();
        let app = pranata::app(test_state_with_clock(pool.clone(), clock.clone()));
        Self { pool, clock, app }
    }

    async fn seed_user(&self) -> UserId {
        let hash = auth::password::hash_password(PASSWORD).unwrap();
        let role = common::administrator_role(&self.pool).await;
        sqlx::query_scalar!(
            r#"INSERT INTO users (username, display_name, password_hash, role_id)
               VALUES ('budi', 'Budi Santoso', $1, $2) RETURNING id AS "id: UserId""#,
            hash,
            role.0
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn send(
        &self,
        method: Method,
        uri: &str,
        session: Option<(&str, Option<&str>)>,
        body: Option<Value>,
    ) -> Reply {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some((token, csrf)) = session {
            builder = builder.header(header::COOKIE, format!("pranata_session={token}"));
            if let Some(csrf) = csrf {
                builder = builder.header("x-csrf-token", csrf);
            }
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
        let token = parts
            .headers
            .get(header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("pranata_session="))
            .map(|value| value.split(';').next().unwrap().to_owned());
        Reply {
            status: parts.status,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            token,
        }
    }

    async fn sign_in(&self) -> Signed {
        let reply = self
            .send(
                Method::POST,
                "/api/auth/login",
                None,
                Some(json!({"username": "budi", "password": PASSWORD})),
            )
            .await;
        assert_eq!(reply.status, StatusCode::OK);
        Signed {
            token: reply.token.unwrap(),
            csrf: reply.body["csrf_token"].as_str().unwrap().to_owned(),
        }
    }

    async fn sudo_until(&self, signed: &Signed) -> Value {
        let me = self
            .send(
                Method::GET,
                "/api/auth/me",
                Some((&signed.token, None)),
                None,
            )
            .await;
        assert_eq!(me.status, StatusCode::OK);
        me.body["sudo_until"].clone()
    }

    async fn confirm(&self, signed: &Signed, csrf: Option<&str>, password: &str) -> Reply {
        self.send(
            Method::POST,
            "/api/auth/sudo",
            Some((&signed.token, csrf)),
            Some(json!({"password": password})),
        )
        .await
    }

    fn in_ten_minutes(&self) -> Value {
        let until = self.clock.now_for_test() + Duration::minutes(10);
        json!(until.format(&Rfc3339).unwrap())
    }
}

/// Parse so that "Z" and "+00:00" spellings compare equal.
fn same_instant(actual: &Value, expected: &Value) -> bool {
    let parse = |value: &Value| OffsetDateTime::parse(value.as_str().unwrap(), &Rfc3339).unwrap();
    parse(actual) == parse(expected)
}

#[sqlx::test]
async fn sign_in_opens_the_sudo_window(pool: PgPool) {
    let h = Harness::new(pool);
    h.seed_user().await;
    let signed = h.sign_in().await;
    let until = h.sudo_until(&signed).await;
    assert!(same_instant(&until, &h.in_ten_minutes()), "{until}");
}

#[sqlx::test]
async fn session_without_reauthentication_needs_sudo(pool: PgPool) {
    let h = Harness::new(pool);
    let user = h.seed_user().await;
    // Inserted the release 0.1.0 way: no reauthenticated_at.
    let now = h.clock.now_for_test();
    sqlx::query!(
        "INSERT INTO sessions (token_hash, user_id, csrf_token, created_at, last_seen_at, expires_at, remember)
         VALUES ($1, $2, 'csrf', $3, $3, $4, false)",
        Sha256::digest(b"old-session").to_vec(),
        user.0,
        now,
        now + Duration::hours(12),
    )
    .execute(&h.pool)
    .await
    .unwrap();
    let signed = Signed {
        token: "old-session".into(),
        csrf: "csrf".into(),
    };
    // `require_sudo` answers from this same value (unit-tested in auth::sudo).
    assert_eq!(h.sudo_until(&signed).await, Value::Null);
}

#[sqlx::test]
async fn confirming_the_password_reopens_the_window(pool: PgPool) {
    let h = Harness::new(pool);
    let user = h.seed_user().await;
    let signed = h.sign_in().await;
    h.clock.advance(Duration::minutes(11));
    assert_eq!(h.sudo_until(&signed).await, Value::Null);

    let reply = h.confirm(&signed, Some(&signed.csrf), PASSWORD).await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    let until = h.sudo_until(&signed).await;
    assert!(same_instant(&until, &h.in_ten_minutes()), "{until}");

    let entry = sqlx::query!(
        "SELECT actor, object_type, object_id FROM audit_entries WHERE action = 'reauthenticate'"
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(entry.actor, Some(user.0));
    assert_eq!(entry.object_type, "user");
    assert_eq!(entry.object_id, user.0.to_string());
}

#[sqlx::test]
async fn wrong_sudo_password_is_401(pool: PgPool) {
    let h = Harness::new(pool);
    h.seed_user().await;
    let signed = h.sign_in().await;
    h.clock.advance(Duration::minutes(5));
    let before = h.sudo_until(&signed).await;

    let reply = h
        .confirm(&signed, Some(&signed.csrf), "wrong password")
        .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.body["code"], "invalid_credentials");
    assert_eq!(h.sudo_until(&signed).await, before);
    let audited = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM audit_entries WHERE action = 'reauthenticate'"#
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(audited, 0);
}

#[sqlx::test]
async fn sudo_needs_csrf(pool: PgPool) {
    let h = Harness::new(pool);
    h.seed_user().await;
    let signed = h.sign_in().await;
    let reply = h.confirm(&signed, None, PASSWORD).await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.body["code"], "csrf_mismatch");
}

#[sqlx::test]
async fn sudo_needs_a_session(pool: PgPool) {
    let h = Harness::new(pool);
    let reply = h
        .send(
            Method::POST,
            "/api/auth/sudo",
            None,
            Some(json!({"password": PASSWORD})),
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.body["code"], "unauthenticated");
}

/// Every route that needs Sudo Mode. `{id}` stands for any id: Sudo Mode is
/// checked before the target is looked up.
const SUDO_ROUTES: &[(Method, &str)] = &[
    (Method::POST, "/api/admin/users"),
    (Method::PATCH, "/api/admin/users/{id}"),
    (Method::POST, "/api/admin/users/{id}/deactivate"),
    (Method::POST, "/api/admin/users/{id}/reactivate"),
    (Method::POST, "/api/admin/users/{id}/access-links"),
];

#[sqlx::test]
async fn write_routes_need_sudo(pool: PgPool) {
    let h = Harness::new(pool);
    let user = h.seed_user().await;
    let signed = h.sign_in().await;
    h.clock.advance(Duration::minutes(11));

    let routes: Vec<(Method, String)> = SUDO_ROUTES
        .iter()
        .map(|(method, uri)| (method.clone(), uri.replace("{id}", &user.0.to_string())))
        .collect();
    let routes: Vec<(Method, &str)> = routes
        .iter()
        .map(|(method, uri)| (method.clone(), uri.as_str()))
        .collect();
    common::assert_needs_sudo(&h.app, &signed.token, &signed.csrf, &routes).await;
}
