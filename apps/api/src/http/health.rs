use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::AppState;

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

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(healthz))
}
