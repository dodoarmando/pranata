//! Pranata API: the only component with database access (D-10).

pub mod audit;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod http;
pub mod ids;
pub mod openapi;
pub mod region_codes;
pub mod village;

use std::sync::Arc;

use axum::Router;
use sqlx::PgPool;

pub use crate::auth::password::HashLimiter;
pub use crate::auth::session::SessionClock;
pub use crate::config::Config;
pub use crate::error::{ApiError, FieldError};

/// Shared handler state. Later tasks add fields (storage, cache purger).
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    /// Clock for session expiry; tests inject a fixed one.
    pub clock: Arc<dyn SessionClock>,
    /// Limits concurrent password hashing (memory-hard, so it must be bounded).
    pub hasher: HashLimiter,
}

/// Build the full router, mounted under `/api`.
pub fn app(state: AppState) -> Router {
    http::router(state)
}
