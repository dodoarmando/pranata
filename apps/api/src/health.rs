use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Ok,
    Unavailable,
}

#[derive(Serialize, ToSchema)]
pub struct Health {
    status: HealthStatus,
}

/// Reports whether the API can reach its database.
#[utoipa::path(
    get,
    path = "/healthz",
    operation_id = "getHealth",
    tag = "system",
    responses(
        (status = OK, description = "The API and its database are up", body = Health),
        (status = SERVICE_UNAVAILABLE, description = "The database is unreachable", body = Health),
    ),
)]
pub async fn healthz(State(pool): State<PgPool>) -> (StatusCode, Json<Health>) {
    match sqlx::query_scalar!(r#"SELECT 1 AS "one!""#)
        .fetch_one(&pool)
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(Health {
                status: HealthStatus::Ok,
            }),
        ),
        Err(error) => {
            tracing::error!(%error, "health check cannot reach the database");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(Health {
                    status: HealthStatus::Unavailable,
                }),
            )
        }
    }
}
