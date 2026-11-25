//! Shared helpers for integration tests.

use std::{
    io::{Read, Write},
    sync::{Arc, Mutex, OnceLock},
};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use pranata::{
    AppState, Config, HashLimiter, auth::session::SessionClock, cache_purge::Purger,
    media::LocalStorage,
};
use secrecy::SecretString;
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use tower::ServiceExt;

/// A clock that only moves when a test says so.
pub struct FixedClock(Mutex<OffsetDateTime>);

impl FixedClock {
    /// Start at a fixed instant (2026-11-24 08:00 UTC).
    #[must_use]
    #[allow(dead_code)]
    pub fn new() -> Arc<Self> {
        let start = OffsetDateTime::from_unix_timestamp(1_795_593_600).expect("valid timestamp");
        Arc::new(Self(Mutex::new(start)))
    }

    #[allow(dead_code)]
    pub fn now_for_test(&self) -> OffsetDateTime {
        *self.0.lock().expect("clock lock")
    }

    #[allow(dead_code)]
    pub fn advance(&self, by: Duration) {
        *self.0.lock().expect("clock lock") += by;
    }
}

impl SessionClock for FixedClock {
    fn now(&self) -> OffsetDateTime {
        *self.0.lock().expect("clock lock")
    }
}

/// URL of a process-wide stand-in web server that answers every request
/// with 204, so jobs that purge the cache succeed in tests that do not care.
fn ok_web_url() -> &'static str {
    static URL: OnceLock<String> = OnceLock::new();
    URL.get_or_init(|| {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind stand-in web");
        let url = format!("http://{}", listener.local_addr().expect("local addr"));
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                std::thread::spawn(move || {
                    let mut stream = stream;
                    let mut buf = [0_u8; 4096];
                    let _ = stream.read(&mut buf);
                    let _ =
                        stream.write_all(b"HTTP/1.1 204 No Content\r\nconnection: close\r\n\r\n");
                });
            }
        });
        url
    })
}

/// Build an `AppState` around the pool handed out by `#[sqlx::test]`, using the
/// system clock.
#[must_use]
#[allow(dead_code)]
pub fn test_state(pool: PgPool) -> AppState {
    test_state_with_clock(pool, Arc::new(pranata::auth::session::SystemClock))
}

/// Like [`test_state`], with an injected clock.
#[must_use]
pub fn test_state_with_clock(pool: PgPool, clock: Arc<dyn SessionClock>) -> AppState {
    test_state_with(pool, clock, ok_web_url())
}

/// Like [`test_state`], with the web server's internal URL (where cache
/// purges go) set to `web_internal_url`.
#[must_use]
#[allow(dead_code)]
pub fn test_state_with_web(pool: PgPool, web_internal_url: &str) -> AppState {
    test_state_with(
        pool,
        Arc::new(pranata::auth::session::SystemClock),
        web_internal_url,
    )
}

fn test_state_with(pool: PgPool, clock: Arc<dyn SessionClock>, web_internal_url: &str) -> AppState {
    // Every state gets its own media directory, so tests never see each
    // other's files.
    let media_dir =
        std::env::temp_dir().join(format!("pranata-test-media-{}", uuid::Uuid::new_v4()));
    let config = Config {
        database_url: String::new(),
        migration_database_url: String::new(),
        public_origin: "http://localhost:3000".parse().expect("valid url"),
        web_internal_url: web_internal_url.parse().expect("valid url"),
        purge_token: SecretString::from("test-purge-token"),
        media_dir: media_dir.clone(),
        listen: "127.0.0.1:0".parse().expect("valid addr"),
    };
    let purger = Purger::new(&config).expect("purger");
    let webauthn = pranata::auth::passkey::relying_party(&config.public_origin).expect("webauthn");
    AppState {
        db: pool,
        config: Arc::new(config),
        purger,
        clock,
        hasher: HashLimiter::new(2).expect("dummy hash"),
        storage: Arc::new(LocalStorage::new(media_dir)),
        webauthn: Arc::new(webauthn),
    }
}

/// The built-in Administrator Role, created by the migrations.
#[allow(dead_code)]
pub async fn administrator_role(pool: &PgPool) -> pranata::ids::RoleId {
    pranata::access::administrator_role(pool)
        .await
        .expect("the Administrator Role exists")
}

/// Assert that every route answers an empty JSON body with 403
/// `sudo_required`. The session behind `cookie` must hold every Permission
/// the routes check and must not be in Sudo Mode.
#[allow(dead_code)]
pub async fn assert_needs_sudo(app: &Router, cookie: &str, csrf: &str, routes: &[(Method, &str)]) {
    for (method, uri) in routes {
        let request = Request::builder()
            .method(method.clone())
            .uri(*uri)
            .header(header::COOKIE, format!("pranata_session={cookie}"))
            .header("x-csrf-token", csrf)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}: {body}");
        assert_eq!(body["code"], "sudo_required", "{method} {uri}: {body}");
    }
}

/// Wait until `count` connections to this test's database wait for a lock.
#[allow(dead_code)]
pub async fn wait_for_lock_waits(pool: &PgPool, count: i64) {
    for _ in 0..500 {
        let waiting = sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM pg_stat_activity
               WHERE datname = current_database() AND wait_event_type = 'Lock'"#
        )
        .fetch_one(pool)
        .await
        .unwrap();
        if waiting >= count {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("fewer than {count} requests waited for a lock");
}

/// Wait until a connection to this test's database waits for a row lock.
#[allow(dead_code)]
pub async fn wait_for_a_lock_wait(pool: &PgPool) {
    wait_for_lock_waits(pool, 1).await;
}

/// Open a transaction that holds `user`'s row `FOR NO KEY UPDATE`, the lock
/// every write to a User takes, until the caller commits or drops it.
#[allow(dead_code)]
pub async fn lock_user(
    pool: &PgPool,
    user: pranata::ids::UserId,
) -> sqlx::Transaction<'static, sqlx::Postgres> {
    let mut holder = pool.begin().await.unwrap();
    sqlx::query!(
        "SELECT id FROM users WHERE id = $1 FOR NO KEY UPDATE",
        user.0
    )
    .fetch_one(&mut *holder)
    .await
    .unwrap();
    holder
}
