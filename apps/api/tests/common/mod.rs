//! Shared helpers for integration tests.

use std::sync::{Arc, Mutex};

use pranata::{AppState, Config, HashLimiter, auth::session::SessionClock, media::LocalStorage};
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
    // Every state gets its own media directory, so tests never see each
    // other's files.
    let media_dir =
        std::env::temp_dir().join(format!("pranata-test-media-{}", uuid::Uuid::new_v4()));
    let config = Config {
        database_url: String::new(),
        migration_database_url: String::new(),
        public_origin: "http://localhost:3000".parse().expect("valid url"),
        web_internal_url: "http://localhost:3000".parse().expect("valid url"),
        purge_token: SecretString::from("test-purge-token"),
        media_dir: media_dir.clone(),
        listen: "127.0.0.1:0".parse().expect("valid addr"),
    };
    AppState {
        db: pool,
        config: Arc::new(config),
        clock,
        hasher: HashLimiter::new(2).expect("dummy hash"),
        storage: Arc::new(LocalStorage::new(media_dir)),
    }
}
