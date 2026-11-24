//! Shared helpers for integration tests.

use std::{
    io::{Read, Write},
    sync::{Arc, Mutex, OnceLock},
};

use pranata::{
    AppState, Config, HashLimiter, auth::session::SessionClock, cache_purge::Purger,
    media::LocalStorage,
};
use secrecy::SecretString;
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

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
    AppState {
        db: pool,
        config: Arc::new(config),
        purger,
        clock,
        hasher: HashLimiter::new(2).expect("dummy hash"),
        storage: Arc::new(LocalStorage::new(media_dir)),
    }
}
