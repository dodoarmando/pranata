pub mod db;
mod health;

use axum::Router;
use sqlx::PgPool;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(OpenApi)]
#[openapi(info(title = "Pranata API"))]
struct ApiDoc;

fn api() -> OpenApiRouter<PgPool> {
    OpenApiRouter::with_openapi(ApiDoc::openapi()).routes(routes!(health::healthz))
}

/// The HTTP application, ready to serve.
pub fn app(pool: PgPool) -> Router {
    api().split_for_parts().0.with_state(pool)
}

/// The OpenAPI description of every route in [`app`].
#[must_use]
pub fn openapi() -> utoipa::openapi::OpenApi {
    api().split_for_parts().1
}
