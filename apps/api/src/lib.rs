mod audit;
mod auth;
mod dashboard;
pub mod db;
mod export;
mod health;
mod imaging;
mod passkey;
mod password;
mod role;
pub mod storage;
mod throttle;
pub mod user;
pub mod village;

pub use auth::purge_expired;

use std::{path::PathBuf, sync::Arc};

use axum::{
    Router,
    extract::{FromRef, Request, State},
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

/// What every handler can reach: the database, the passkey ceremonies in flight, the
/// sign-in throttle and the files volume.
#[derive(Clone)]
pub(crate) struct AppState {
    pool: PgPool,
    passkeys: Arc<passkey::Passkeys>,
    throttle: Arc<throttle::Throttle>,
    storage: Arc<storage::Storage>,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl FromRef<AppState> for Arc<passkey::Passkeys> {
    fn from_ref(state: &AppState) -> Self {
        state.passkeys.clone()
    }
}

impl FromRef<AppState> for Arc<throttle::Throttle> {
    fn from_ref(state: &AppState) -> Self {
        state.throttle.clone()
    }
}

impl FromRef<AppState> for Arc<storage::Storage> {
    fn from_ref(state: &AppState) -> Self {
        state.storage.clone()
    }
}

fn api() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health::healthz))
        .routes(routes!(village::get_village))
        .routes(routes!(village::update_profile))
        .routes(routes!(village::upload_logo))
        .routes(routes!(storage::get_media))
        .routes(routes!(auth::use_access_link))
        .routes(routes!(auth::sign_in))
        .routes(routes!(auth::sign_out))
        .routes(routes!(auth::me))
        .routes(routes!(auth::reauthenticate))
        .routes(routes!(auth::sessions))
        .routes(routes!(auth::sign_out_others))
        .routes(routes!(auth::change_password))
        .routes(routes!(passkey::list, passkey::create))
        .routes(routes!(passkey::start_registration))
        .routes(routes!(passkey::delete))
        .routes(routes!(passkey::start_sign_in))
        .routes(routes!(passkey::finish_sign_in))
        .routes(routes!(audit::list))
        .routes(routes!(audit::export))
        .routes(routes!(dashboard::get))
        .routes(routes!(role::list, role::create))
        .routes(routes!(role::update))
        .routes(routes!(user::list, user::create))
        .routes(routes!(user::update))
        .routes(routes!(user::reset))
        .routes(routes!(user::revoke_sessions))
        .routes(routes!(user::remove_passkeys))
}

/// The HTTP application, ready to serve at `public_origin` (`https://host`, no trailing slash)
/// with files kept under `files_dir`.
///
/// # Errors
/// When `public_origin` is not a URL with a domain, which passkeys are bound to.
pub fn app(pool: PgPool, public_origin: String, files_dir: PathBuf) -> anyhow::Result<Router> {
    let passkeys = Arc::new(passkey::Passkeys::new(&public_origin)?);
    Ok(api()
        .split_for_parts()
        .0
        .layer(middleware::from_fn_with_state(
            public_origin,
            same_origin_only,
        ))
        .with_state(AppState {
            pool,
            passkeys,
            throttle: Arc::default(),
            storage: Arc::new(storage::Storage::new(files_dir)),
        }))
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
