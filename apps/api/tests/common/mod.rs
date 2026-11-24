//! Shared helpers for integration tests.

use std::sync::Arc;

use pranata::{AppState, Config};
use secrecy::SecretString;
use sqlx::PgPool;

/// Build an `AppState` around the pool handed out by `#[sqlx::test]`.
#[must_use]
pub fn test_state(pool: PgPool) -> AppState {
    let config = Config {
        database_url: String::new(),
        migration_database_url: String::new(),
        public_origin: "http://localhost:3000".parse().expect("valid url"),
        web_internal_url: "http://localhost:3000".parse().expect("valid url"),
        purge_token: SecretString::from("test-purge-token"),
        media_dir: std::env::temp_dir().join("pranata-test-media"),
        listen: "127.0.0.1:0".parse().expect("valid addr"),
    };
    AppState {
        db: pool,
        config: Arc::new(config),
    }
}
