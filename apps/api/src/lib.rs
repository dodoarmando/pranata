pub mod audit;
mod auth;
pub mod db;
mod health;
mod password;
pub mod user;
pub mod village;

pub use auth::purge_expired;

use axum::{
    Router,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sqlx::PgPool;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(OpenApi)]
#[openapi(info(title = "Pranata API"))]
struct ApiDoc;

fn api() -> OpenApiRouter<PgPool> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health::healthz))
        .routes(routes!(village::get_village))
        .routes(routes!(auth::use_access_link))
        .routes(routes!(auth::sign_in))
        .routes(routes!(auth::sign_out))
        .routes(routes!(auth::me))
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

/// Why a request failed; anything unexpected becomes a logged `500` without its details.
pub(crate) enum ApiError {
    Status(StatusCode),
    PasswordRejected(password::PasswordProblem),
    Internal(anyhow::Error),
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(error: E) -> Self {
        Self::Internal(error.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::Status(status) => status.into_response(),
            Self::PasswordRejected(problem) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                axum::Json(auth::PasswordRejected { problem }),
            )
                .into_response(),
            Self::Internal(error) => {
                tracing::error!(error = format!("{error:#}"), "request failed");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}
