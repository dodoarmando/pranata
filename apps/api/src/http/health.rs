use std::time::Duration;

use axum::{Json, extract::State};
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    AppState,
    error::{ApiError, Problem},
};

#[derive(Serialize, ToSchema)]
pub struct Health {
    pub status: &'static str,
    pub version: &'static str,
}

/// Liveness and version probe.
#[utoipa::path(
    get,
    path = "/healthz",
    operation_id = "healthz",
    tag = "ops",
    responses((status = 200, description = "The API is up", body = Health))
)]
pub async fn healthz() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[derive(Serialize, ToSchema)]
pub struct Ready {
    pub status: &'static str,
}

/// How long the readiness probe waits for the database.
const READY_TIMEOUT: Duration = Duration::from_secs(3);

/// Readiness probe for an external uptime check: the API is up and the
/// database answers. `/healthz` stays a pure liveness probe.
#[utoipa::path(
    get,
    path = "/readyz",
    operation_id = "readyz",
    tag = "ops",
    responses(
        (status = 200, description = "The API is up and the database answers", body = Ready),
        (status = 503, description = "The database does not answer (code database_unavailable)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn readyz(State(state): State<AppState>) -> Result<Json<Ready>, ApiError> {
    let probe = sqlx::query_scalar!(r#"SELECT 1 AS "ok!""#).fetch_one(&state.db);
    match tokio::time::timeout(READY_TIMEOUT, probe).await {
        Ok(Ok(_)) => Ok(Json(Ready { status: "ok" })),
        Ok(Err(error)) => {
            tracing::warn!(error = ?error, "readiness probe: the database query failed");
            Err(ApiError::Unavailable("database_unavailable"))
        }
        Err(_) => {
            tracing::warn!("readiness probe: the database did not answer in time");
            Err(ApiError::Unavailable("database_unavailable"))
        }
    }
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(healthz))
        .routes(routes!(readyz))
}
