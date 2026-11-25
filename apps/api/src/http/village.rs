use std::time::Duration;

use axum::{Json, extract::State};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    ApiError, AppState,
    auth::CurrentUser,
    error::Problem,
    http::json::ApiJson,
    village::{
        self,
        profile::{self, Profile, ProfileUpdate},
    },
};

/// The Village profile shown on the public site.
///
/// # Errors
/// 404 `village_not_initialized` before `pranata village init` has run.
#[utoipa::path(
    get,
    path = "/public/village",
    operation_id = "getPublicVillage",
    tag = "public",
    responses(
        (status = 200, description = "The Village profile", body = village::VillageProfile),
        (status = 404, description = "The Village is not initialised yet (code village_not_initialized)", body = crate::error::Problem, content_type = "application/problem+json"),
    )
)]
pub async fn get_public_village(
    State(state): State<AppState>,
) -> Result<Json<village::VillageProfile>, ApiError> {
    village::get(&state.db)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("village_not_initialized"))
}

/// The editable part of the Village profile.
///
/// # Errors
/// 404 `village_not_initialized` before `pranata village init` has run.
#[utoipa::path(
    get,
    path = "/admin/village/profile",
    operation_id = "getVillageProfile",
    tag = "village",
    security(("session" = [])),
    responses(
        (status = 200, description = "The overview and hero photo", body = Profile),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "The Village is not initialised yet (code village_not_initialized)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn get_village_profile(
    State(state): State<AppState>,
    _user: CurrentUser,
) -> Result<Json<Profile>, ApiError> {
    profile::get(&state.db).await.map(Json)
}

/// Replace the overview and hero photo, then purge the public cache.
///
/// # Errors
/// 409 `stale_write`; 422 `validation_failed` (field `overview`) or
/// `hero_not_ready`; 404 `village_not_initialized`.
#[utoipa::path(
    put,
    path = "/admin/village/profile",
    operation_id = "putVillageProfile",
    tag = "village",
    security(("session" = [])),
    request_body = ProfileUpdate,
    responses(
        (status = 200, description = "The saved profile", body = Profile),
        (status = 400, description = "Body is not valid JSON (code invalid_json)", body = Problem, content_type = "application/problem+json"),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "The Village is not initialised yet (code village_not_initialized)", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The profile changed since it was read (code stale_write)", body = Problem, content_type = "application/problem+json"),
        (status = 415, description = "Request is not marked as JSON (code json_required)", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid overview (code validation_failed, with field errors) or a hero that is not a ready image (code hero_not_ready)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn put_village_profile(
    State(state): State<AppState>,
    user: CurrentUser,
    ApiJson(input): ApiJson<ProfileUpdate>,
) -> Result<Json<Profile>, ApiError> {
    let (saved, purge_job) = profile::update(&state.db, user.id, input).await?;
    purge_now(&state, purge_job).await;
    Ok(Json(saved))
}

/// How long a save waits for the web server to drop its cached pages.
const INLINE_PURGE_TIMEOUT: Duration = Duration::from_secs(2);

/// D-12: the next public request after a save must already be fresh, so the
/// purge runs before the answer instead of waiting for the job worker. When it
/// fails the queued job stays as the retry; when it works that job (and only that
/// one: another save's job may still be waiting for its own purge) is redundant
/// and is marked done.
async fn purge_now(state: &AppState, job_id: i64) {
    match tokio::time::timeout(INLINE_PURGE_TIMEOUT, state.purger.purge_all()).await {
        Ok(Ok(())) => {
            let done = sqlx::query!(
                "UPDATE jobs SET done_at = now()
                 WHERE id = $1 AND done_at IS NULL AND failed_at IS NULL
                   AND locked_until IS NULL",
                job_id,
            )
            .execute(&state.db)
            .await;
            if let Err(error) = done {
                tracing::warn!(%error, "could not mark the purge job done; it will run harmlessly");
            }
        }
        Ok(Err(error)) => {
            tracing::warn!(%error, "inline cache purge failed; the queued job will retry");
        }
        Err(_) => tracing::warn!("inline cache purge timed out; the queued job will retry"),
    }
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_public_village))
        .routes(routes!(get_village_profile, put_village_profile))
}
