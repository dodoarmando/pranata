mod audit;
mod auth;
mod dashboard;
pub mod db;
mod health;
mod password;
mod role;
pub mod user;
pub mod village;

pub use auth::purge_expired;

use axum::{
    Router,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
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
        .routes(routes!(auth::reauthenticate))
        .routes(routes!(audit::list))
        .routes(routes!(dashboard::get))
        .routes(routes!(role::list, role::create))
        .routes(routes!(role::update))
        .routes(routes!(user::list, user::create))
        .routes(routes!(user::update))
        .routes(routes!(user::reset))
        .routes(routes!(user::revoke_sessions))
}

/// The HTTP application, ready to serve at `public_origin` (`https://host`, no trailing slash).
pub fn app(pool: PgPool, public_origin: String) -> Router {
    api()
        .split_for_parts()
        .0
        .layer(middleware::from_fn_with_state(
            public_origin,
            same_origin_only,
        ))
        .with_state(pool)
}

/// CSRF defence without tokens: an unsafe method needs `Sec-Fetch-Site: same-origin` or,
/// from a browser that sends no Fetch Metadata, an `Origin` equal to the public origin.
async fn same_origin_only(
    State(public_origin): State<String>,
    request: Request,
    next: Next,
) -> Response {
    let headers = request.headers();
    let same_origin = match headers.get("sec-fetch-site") {
        Some(site) => site == "same-origin",
        None => headers
            .get(header::ORIGIN)
            .is_some_and(|origin| origin == public_origin.as_str()),
    };
    if request.method().is_safe() || same_origin {
        next.run(request).await
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
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
