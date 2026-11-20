mod album;
mod announcement;
mod apbdes_summary;
mod audit;
mod auth;
mod category;
mod dashboard;
pub mod db;
mod event;
mod export;
mod government;
mod health;
mod home;
mod imaging;
mod important_contact;
mod index_edition;
mod institution;
mod news_post;
mod order;
mod page;
mod partner;
mod passkey;
mod password;
mod region;
mod rich_text;
mod role;
mod search;
mod sitemap;
mod slug;
pub mod storage;
mod throttle;
mod transparency_document;
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
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(OpenApi)]
#[openapi(info(title = "Pranata API"))]
struct ApiDoc;

/// What every handler can reach: the database, the passkey ceremonies in flight, the
/// sign-in throttle, the files volume and the clock.
#[derive(Clone)]
pub(crate) struct AppState {
    pool: PgPool,
    passkeys: Arc<passkey::Passkeys>,
    throttle: Arc<throttle::Throttle>,
    storage: Arc<storage::Storage>,
    clock: Clock,
}

/// What time it is: the system's, or the instant a test pins.
#[derive(Clone, Copy)]
pub(crate) struct Clock(Option<DateTime<Utc>>);

impl Clock {
    pub(crate) fn now(self) -> DateTime<Utc> {
        self.0.unwrap_or_else(Utc::now)
    }
}

impl FromRef<AppState> for Clock {
    fn from_ref(state: &AppState) -> Self {
        state.clock
    }
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

/// The newer modules, which own their routes.
fn modules() -> OpenApiRouter<AppState> {
    important_contact::routes()
        .merge(partner::routes())
        .merge(apbdes_summary::routes())
        .merge(transparency_document::routes())
        .merge(home::routes())
        .merge(search::routes())
        .merge(sitemap::routes())
}

fn api() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health::healthz))
        .routes(routes!(village::get_village))
        .routes(routes!(village::update_profile))
        .routes(routes!(village::upload_logo))
        .routes(routes!(
            village::get_vision_mission,
            village::update_vision_mission
        ))
        .routes(routes!(region::get_tree))
        .routes(routes!(region::update_settings))
        .routes(routes!(region::create_unit))
        .routes(routes!(region::update_unit))
        .routes(routes!(region::order_hamlets))
        .routes(routes!(
            government::list_positions,
            government::create_position
        ))
        .routes(routes!(
            government::rename_position,
            government::delete_position
        ))
        .routes(routes!(government::create_tenure))
        .routes(routes!(government::update_tenure))
        .routes(routes!(government::upload_photo))
        .routes(routes!(government::get_government))
        .routes(routes!(institution::list, institution::create))
        .routes(routes!(institution::update, institution::delete))
        .routes(routes!(index_edition::list, index_edition::create))
        .routes(routes!(index_edition::update, index_edition::delete))
        .routes(routes!(category::list, category::create))
        .routes(routes!(category::update, category::delete))
        .routes(routes!(news_post::list, news_post::create))
        .routes(routes!(
            news_post::get,
            news_post::update,
            news_post::delete
        ))
        .routes(routes!(news_post::publish))
        .routes(routes!(news_post::unpublish))
        .routes(routes!(news_post::upload_cover))
        .routes(routes!(news_post::list_published))
        .routes(routes!(news_post::get_published))
        .routes(routes!(announcement::list, announcement::create))
        .routes(routes!(
            announcement::get,
            announcement::update,
            announcement::delete
        ))
        .routes(routes!(announcement::publish))
        .routes(routes!(announcement::unpublish))
        .routes(routes!(announcement::list_published))
        .routes(routes!(announcement::list_in_effect))
        .routes(routes!(announcement::get_published))
        .routes(routes!(event::list, event::create))
        .routes(routes!(event::get, event::update, event::delete))
        .routes(routes!(event::publish))
        .routes(routes!(event::unpublish))
        .routes(routes!(event::list_upcoming))
        .routes(routes!(event::list_past))
        .routes(routes!(page::list, page::create))
        .routes(routes!(page::get, page::update, page::delete))
        .routes(routes!(page::publish))
        .routes(routes!(page::unpublish))
        .routes(routes!(page::upload_cover))
        .routes(routes!(page::list_published))
        .routes(routes!(page::get_published))
        .routes(routes!(album::list, album::create))
        .routes(routes!(album::get, album::update, album::delete))
        .routes(routes!(album::publish))
        .routes(routes!(album::unpublish))
        .routes(routes!(album::upload_photo))
        .routes(routes!(album::list_published))
        .routes(routes!(album::get_published))
        .merge(modules())
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
        .routes(routes!(user::link_official))
}

/// The HTTP application, ready to serve at `public_origin` (`https://host`, no trailing slash)
/// with files kept under `files_dir`.
///
/// # Errors
/// When `public_origin` is not a URL with a domain, which passkeys are bound to.
pub fn app(pool: PgPool, public_origin: String, files_dir: PathBuf) -> anyhow::Result<Router> {
    app_at(pool, public_origin, files_dir, None)
}

/// [`app`] with its clock stopped at `now`, for tests of date rules; `None` is the system clock.
///
/// # Errors
/// As [`app`].
pub fn app_at(
    pool: PgPool,
    public_origin: String,
    files_dir: PathBuf,
    now: Option<DateTime<Utc>>,
) -> anyhow::Result<Router> {
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
            clock: Clock(now),
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
