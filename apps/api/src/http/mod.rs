pub mod health;
pub mod request_id;
pub mod security_headers;
pub mod village;

use axum::{Router, middleware};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;

use crate::{ApiError, AppState, openapi::ApiDoc};

/// Routes plus their `OpenAPI` metadata. Later tasks nest more routers here.
#[must_use]
pub fn api_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/api", health::router().merge(village::router()))
}

pub fn router(state: AppState) -> Router {
    let (router, _) = api_router().split_for_parts();
    let router = router
        .fallback(|| async { ApiError::NotFound("not_found") })
        .with_state(state)
        .layer(middleware::map_response(security_headers::apply));
    request_id::layer(router)
}
