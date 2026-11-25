mod common;

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use common::{FixedClock, test_state_with_clock};
use http_body_util::BodyExt;
use pranata::auth;
use serde_json::{Value, json};
use sqlx::PgPool;
use time::Duration;
use tower::ServiceExt;

const PASSWORD: &str = "correct horse battery";

struct Harness {
    pool: PgPool,
    clock: Arc<FixedClock>,
    app: Router,
    next_ip: AtomicU32,
}

struct Reply {
    status: StatusCode,
    body: Value,
    retry_after: Option<String>,
    token: Option<String>,
}

impl Harness {
    fn new(pool: PgPool) -> Self {
        let clock = FixedClock::new();
        let app = pranata::app(test_state_with_clock(pool.clone(), clock.clone()));
        Self {
            pool,
            clock,
            app,
            next_ip: AtomicU32::new(0),
        }
    }

    /// A client address nobody used before in this test, so the per-IP limit
    /// never interferes with the per-account tests.
    fn fresh_ip(&self) -> String {
        let n = self.next_ip.fetch_add(1, Ordering::Relaxed);
        format!("10.{}.{}.{}", (n >> 16) & 255, (n >> 8) & 255, n & 255)
    }

    async fn seed_user(&self, username: &str) -> pranata::ids::UserId {
        let hash = auth::password::hash_password(PASSWORD).unwrap();
        let role = common::administrator_role(&self.pool).await;
        sqlx::query_scalar!(
            r#"INSERT INTO users (username, display_name, password_hash, role_id)
               VALUES ($1, 'Budi Santoso', $2, $3) RETURNING id AS "id: pranata::ids::UserId""#,
            username,
            hash,
            role.0
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn send(
        &self,
        uri: &str,
        ip: Option<&str>,
        session: Option<(&str, &str)>,
        body: Value,
    ) -> Reply {
        let mut builder = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(ip) = ip {
            builder = builder.header("x-forwarded-for", ip);
        }
        if let Some((token, csrf)) = session {
            builder = builder
                .header(header::COOKIE, format!("pranata_session={token}"))
                .header("x-csrf-token", csrf);
        }
        let request = builder.body(Body::from(body.to_string())).unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        Reply {
            status: parts.status,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            retry_after: parts
                .headers
                .get(header::RETRY_AFTER)
                .map(|value| value.to_str().unwrap().to_owned()),
            token: parts
                .headers
                .get(header::SET_COOKIE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("pranata_session="))
                .map(|value| value.split(';').next().unwrap().to_owned()),
        }
    }

    /// A password sign-in from a fresh client address.
    async fn login(&self, username: &str, password: &str) -> Reply {
        let ip = self.fresh_ip();
        self.send(
            "/api/auth/login",
            Some(&ip),
            None,
            json!({"username": username, "password": password}),
        )
        .await
    }

    async fn failures(&self) -> i64 {
        sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM login_failures"#)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
}

#[sqlx::test]
async fn ten_failures_lock_the_account_for_fifteen_minutes(pool: PgPool) {
    let h = Harness::new(pool);
    h.seed_user("budi").await;
    for _ in 0..10 {
        assert_eq!(
            h.login("budi", "wrong password").await.status,
            StatusCode::UNAUTHORIZED
        );
    }
    let locked = h.login("budi", PASSWORD).await;
    assert_eq!(locked.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(locked.body["code"], "too_many_attempts");
    assert_eq!(locked.retry_after.as_deref(), Some("900"));

    h.clock.advance(Duration::minutes(15));
    assert_eq!(h.login("budi", PASSWORD).await.status, StatusCode::OK);
}

#[sqlx::test]
async fn unknown_usernames_are_throttled_too(pool: PgPool) {
    let h = Harness::new(pool);
    h.seed_user("budi").await;
    for _ in 0..10 {
        assert_eq!(
            h.login("tidakada", "wrong password").await.status,
            StatusCode::UNAUTHORIZED
        );
    }
    let locked = h.login("tidakada", "wrong password").await;
    assert_eq!(locked.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(locked.body["code"], "too_many_attempts");
    assert_eq!(locked.retry_after.as_deref(), Some("900"));
}

#[sqlx::test]
async fn success_clears_the_counter(pool: PgPool) {
    let h = Harness::new(pool);
    h.seed_user("budi").await;
    for _ in 0..9 {
        assert_eq!(
            h.login("budi", "wrong password").await.status,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(h.login("budi", PASSWORD).await.status, StatusCode::OK);
    for _ in 0..9 {
        assert_eq!(
            h.login("budi", "wrong password").await.status,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(h.login("budi", PASSWORD).await.status, StatusCode::OK);
}

#[sqlx::test]
async fn wrong_sudo_password_counts_as_a_failure(pool: PgPool) {
    let h = Harness::new(pool);
    h.seed_user("budi").await;
    let signed = h.login("budi", PASSWORD).await;
    assert_eq!(signed.status, StatusCode::OK);
    let token = signed.token.unwrap();
    let csrf = signed.body["csrf_token"].as_str().unwrap().to_owned();
    for _ in 0..10 {
        let reply = h
            .send(
                "/api/auth/sudo",
                None,
                Some((&token, &csrf)),
                json!({"password": "wrong password"}),
            )
            .await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    }
    let locked = h.login("budi", PASSWORD).await;
    assert_eq!(locked.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(locked.body["code"], "too_many_attempts");
    // The locked account cannot confirm Sudo Mode either.
    let sudo = h
        .send(
            "/api/auth/sudo",
            None,
            Some((&token, &csrf)),
            json!({"password": PASSWORD}),
        )
        .await;
    assert_eq!(sudo.status, StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test]
async fn redeeming_an_access_link_clears_the_counter(pool: PgPool) {
    let h = Harness::new(pool);
    let user = h.seed_user("budi").await;
    for _ in 0..10 {
        h.login("budi", "wrong password").await;
    }
    assert_eq!(
        h.login("budi", PASSWORD).await.status,
        StatusCode::TOO_MANY_REQUESTS
    );

    let mut conn = h.pool.acquire().await.unwrap();
    let token = auth::access_link::create(&mut conn, user, None, h.clock.now_for_test())
        .await
        .unwrap();
    drop(conn);
    let ip = h.fresh_ip();
    let redeemed = h
        .send(
            "/api/auth/access-links/redeem",
            Some(&ip),
            None,
            json!({"token": token, "password": "a brand new password"}),
        )
        .await;
    assert_eq!(redeemed.status, StatusCode::NO_CONTENT);
    assert_eq!(
        h.login("budi", "a brand new password").await.status,
        StatusCode::OK
    );
}

#[sqlx::test]
async fn malformed_usernames_are_not_recorded(pool: PgPool) {
    let h = Harness::new(pool);
    let reply = h.login(&"a".repeat(300), "wrong password").await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.body["code"], "invalid_credentials");
    assert_eq!(h.failures().await, 0);
}
